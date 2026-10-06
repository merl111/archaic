use crate::{HistoryInfo, HistoryLoad, Message};
use matrix_sdk::{Room, room::MessagesOptions, ruma::uint};
use std::collections::HashSet;

const PAGE_SIZE: usize = 50;
const WINDOW_LIMIT: usize = 500;
const PAGE_LIMIT: usize = 128;

use crate::timeline::{Entry, aggregate, decode};

#[derive(Default)]
pub(crate) struct History {
    cached_limit: usize,
    cached_more: bool,
    room_id: String,
    entries: Vec<Entry>,
    cursor: Option<String>,
    forward: Option<String>,
    seen_forward: HashSet<String>,
    seen: HashSet<String>,
    initialized: bool,
    limited: bool,
    gap: bool,
}

impl History {
    pub fn activate(&mut self, cache: &mut std::collections::VecDeque<Self>, room: &str) {
        if self.room_id == room {
            return;
        }
        let next = cache
            .iter()
            .position(|h| h.room_id == room)
            .and_then(|i| cache.remove(i))
            .unwrap_or_default();
        let previous = std::mem::replace(self, next);
        if previous.initialized {
            cache.retain(|h| h.room_id != previous.room_id);
            cache.push_back(previous);
        }
        while cache.len() > 32 {
            cache.pop_front();
        }
    }
    pub fn belongs_to(&self, rooms: &std::collections::HashSet<&str>) -> bool {
        rooms.contains(self.room_id.as_str())
    }

    pub fn is_cached(&self) -> bool {
        self.cached_limit > 0
    }
    fn cached(
        &mut self,
        room: &Room,
        kind: HistoryLoad,
        archive: &crate::archive::Archive,
    ) -> Result<Option<(Vec<Message>, HistoryInfo)>, &'static str> {
        if !archive.persistent()
            || (self.cached_limit == 0 && self.forward.is_some() && kind != HistoryLoad::Latest)
        {
            return Ok(None);
        }
        if kind == HistoryLoad::Latest {
            self.cached_limit = 0;
        }
        if kind == HistoryLoad::Newer || (kind == HistoryLoad::Older && self.cached_limit == 0) {
            return Ok(None);
        }
        let limit = if kind == HistoryLoad::Older {
            (self.cached_limit + PAGE_SIZE).min(WINDOW_LIMIT)
        } else {
            self.cached_limit.max(PAGE_SIZE)
        };
        let redacts = room
            .clone_info()
            .room_version_rules_or_default()
            .redaction
            .keep_room_redaction_redacts;
        let page = archive.recent(room.room_id().as_str(), limit, redacts)?;
        if page.values.is_empty() {
            return Ok(None);
        }
        self.entries = deduplicate(
            page.values
                .into_iter()
                .map(|v| decode(v, redacts))
                .collect(),
        );
        self.cached_limit = limit;
        self.cached_more = page.more;
        self.initialized = true;
        self.forward = None;
        Ok(Some(self.snapshot()))
    }

    pub async fn context(
        &mut self,
        room: &Room,
        event_id: &str,
    ) -> Result<(Vec<Message>, HistoryInfo), &'static str> {
        let id = matrix_sdk::ruma::EventId::parse(event_id).map_err(|_| "message-unavailable")?;
        let response = room
            .event_with_context(&id, true, uint!(50), None)
            .await
            .map_err(crate::message_actions::failure)?;
        if response.events_before.len() + response.events_after.len() > 50 {
            return Err("history-invalid-page");
        }
        let target = response.event.ok_or("message-unavailable")?;
        if target
            .raw()
            .get_field::<String>("event_id")
            .ok()
            .flatten()
            .as_deref()
            != Some(event_id)
        {
            return Err("message-unavailable");
        }
        let rules = room.clone_info().room_version_rules_or_default().redaction;
        let entries = response
            .events_before
            .iter()
            .rev()
            .chain(std::iter::once(&target))
            .chain(response.events_after.iter())
            .map(|event| {
                crate::details::value(room, event)
                    .map(|v| decode(v, rules.keep_room_redaction_redacts))
            })
            .collect::<Result<Vec<_>, _>>()?;
        *self = Self {
            room_id: room.room_id().to_string(),
            entries: deduplicate(entries),
            cursor: response.prev_batch_token,
            forward: response.next_batch_token,
            initialized: true,
            ..Default::default()
        };
        Ok(self.snapshot())
    }

    pub fn sync(
        &mut self,
        room: &Room,
        timeline: &matrix_sdk::sync::Timeline,
    ) -> Option<(Vec<Message>, HistoryInfo)> {
        if self.cached_limit > 0
            || self.forward.is_some()
            || !self.initialized
            || self.room_id != room.room_id().as_str()
            || (timeline.events.is_empty() && !timeline.limited)
        {
            return None;
        }
        let rules = room.clone_info().room_version_rules_or_default().redaction;
        let entries = timeline
            .events
            .iter()
            .filter_map(|event| serde_json::from_str(event.raw().json().get()).ok())
            .map(|value| decode(value, rules.keep_room_redaction_redacts))
            .collect::<Vec<_>>();
        let before = self.snapshot();
        self.apply_sync(entries, timeline.limited, timeline.prev_batch.clone());
        let after = self.snapshot();
        (before != after).then_some(after)
    }
    fn apply_sync(&mut self, entries: Vec<Entry>, limited: bool, cursor: Option<String>) {
        if limited {
            self.apply_recent(HistoryLoad::Recent, deduplicate(entries), cursor);
        } else {
            for entry in entries {
                if let Some(old) = self.entries.iter_mut().find(|old| old.id == entry.id) {
                    *old = entry;
                } else {
                    self.entries.push(entry);
                }
            }
        }
        if self.entries.len() > WINDOW_LIMIT {
            self.entries.drain(..self.entries.len() - WINDOW_LIMIT);
            self.limited = true;
        }
    }

    pub async fn load(
        &mut self,
        room: &Room,
        kind: HistoryLoad,
        archive: &crate::archive::Archive,
    ) -> Result<(Vec<Message>, HistoryInfo), &'static str> {
        if self.room_id != room.room_id().as_str() {
            *self = Self {
                room_id: room.room_id().to_string(),
                ..Self::default()
            };
        }
        if kind == HistoryLoad::Older && self.cached_limit > 0 {
            // The user has reached beyond the local window: fetch one page now
            // instead of waiting for background round-robin to reach this room.
            let before = archive.recent(
                room.room_id().as_str(),
                self.cached_limit + PAGE_SIZE,
                false,
            )?;
            let roots = before
                .values
                .iter()
                .filter(|v| v["type"] == "m.room.message" || v["type"] == "m.room.encrypted")
                .count();
            if before.more && roots < self.cached_limit + PAGE_SIZE {
                crate::archive_sync::fetch(archive, room, "history").await?;
            }
            if self.cached_limit >= WINDOW_LIMIT
                && let Some(first) = self.snapshot().0.first()
            {
                return self.context(room, &first.id).await;
            }
        }
        if self.cached_limit == 0 && self.forward.is_some() && kind == HistoryLoad::Recent {
            return Ok(self.snapshot());
        }
        if let Some(snapshot) = self.cached(room, kind, archive)? {
            // Never wait for the network when there is a local viewport. The background
            // archiver fills remaining history and publishes refreshed cache snapshots.
            return Ok(snapshot);
        }
        if kind == HistoryLoad::Older
            && self.entries.len() >= WINDOW_LIMIT
            && self.cursor.is_some()
            && let Some(first) = self.snapshot().0.first()
        {
            return self.context(room, &first.id).await;
        }
        if kind == HistoryLoad::Older && (!self.initialized || !self.info().can_load_more) {
            return Ok(self.snapshot());
        }
        if kind == HistoryLoad::Newer && !self.info().can_load_newer {
            return Ok(self.snapshot());
        }
        let mut options = if kind == HistoryLoad::Newer {
            MessagesOptions::forward()
        } else {
            MessagesOptions::backward()
        };
        options.limit = uint!(50);
        if kind == HistoryLoad::Older {
            options.from = self.cursor.clone();
        }
        if kind == HistoryLoad::Newer {
            options.from = self.forward.clone();
        }
        let response = room
            .messages(options)
            .await
            .map_err(|_| "history-load-failed")?;
        if response.chunk.len() > PAGE_SIZE {
            return Err("history-invalid-page");
        }
        let rules = room.clone_info().room_version_rules_or_default().redaction;
        let mut chunk = response.chunk;
        if kind != HistoryLoad::Newer {
            chunk.reverse();
        }
        let values = chunk
            .iter()
            .map(|event| crate::details::value(room, event))
            .collect::<Result<Vec<_>, _>>()?;
        archive.merge(
            room.room_id().as_str(),
            values.clone(),
            rules.keep_room_redaction_redacts,
        )?;
        let entries = values
            .into_iter()
            .map(|value| decode(value, rules.keep_room_redaction_redacts))
            .collect();
        self.apply_page(kind, entries, response.end)?;
        Ok(self.snapshot())
    }
    fn apply_page(
        &mut self,
        kind: HistoryLoad,
        entries: Vec<Entry>,
        end: Option<String>,
    ) -> Result<(), &'static str> {
        if kind == HistoryLoad::Newer {
            self.apply_newer(entries, end)?;
        } else if kind == HistoryLoad::Older {
            if end
                .as_ref()
                .is_some_and(|end| Some(end) == self.cursor.as_ref() || self.seen.contains(end))
            {
                return Err("history-stalled");
            }
            if let Some(cursor) = self.cursor.take() {
                self.seen.insert(cursor);
            }
            let known: HashSet<_> = self.entries.iter().map(|entry| entry.id.as_str()).collect();
            let mut prefix = deduplicate(entries);
            prefix.retain(|entry| !known.contains(entry.id.as_str()));
            prefix.append(&mut self.entries);
            self.entries = prefix;
            self.cursor = end;
        } else {
            self.apply_recent(kind, deduplicate(entries), end);
        }
        if self.entries.len() > WINDOW_LIMIT {
            self.entries.drain(..self.entries.len() - WINDOW_LIMIT);
            self.limited = true;
        }
        self.initialized = true;
        Ok(())
    }
    fn apply_newer(
        &mut self,
        entries: Vec<Entry>,
        end: Option<String>,
    ) -> Result<(), &'static str> {
        if end.as_ref().is_some_and(|end| {
            Some(end) == self.forward.as_ref() || self.seen_forward.contains(end)
        }) {
            return Err("history-stalled");
        }
        if let Some(cursor) = self.forward.take() {
            self.seen_forward.insert(cursor);
        }
        for entry in deduplicate(entries) {
            if let Some(old) = self.entries.iter_mut().find(|e| e.id == entry.id) {
                *old = entry;
            } else {
                self.entries.push(entry);
            }
        }
        self.forward = end;

        Ok(())
    }
    fn apply_recent(&mut self, kind: HistoryLoad, entries: Vec<Entry>, end: Option<String>) {
        let overlap = entries
            .iter()
            .find_map(|incoming| self.entries.iter().position(|old| old.id == incoming.id));
        if kind == HistoryLoad::Latest || !self.initialized || overlap.is_none() {
            let gap = kind == HistoryLoad::Recent
                && self.initialized
                && !self.entries.is_empty()
                && overlap.is_none();
            self.entries = entries;
            self.cursor = end;
            self.seen.clear();
            self.forward = None;
            self.seen_forward.clear();
            self.limited = false;
            self.gap = gap || (kind == HistoryLoad::Recent && self.gap);
        } else if let Some(overlap) = overlap {
            self.entries.truncate(overlap);
            let incoming: HashSet<_> = entries.iter().map(|entry| entry.id.as_str()).collect();
            self.entries
                .retain(|old| !incoming.contains(old.id.as_str()));
            self.entries.extend(entries);
        }
    }
    pub async fn retry_decryption(&mut self, room: &Room) -> Option<(Vec<Message>, HistoryInfo)> {
        if self.room_id != room.room_id().as_str() {
            return None;
        }
        crate::timeline::retry_decryption(&mut self.entries, room)
            .await
            .then(|| self.snapshot())
    }
    fn info(&self) -> HistoryInfo {
        if self.cached_limit > 0 {
            return HistoryInfo {
                can_load_more: self.cached_more,
                can_load_newer: false,
                at_limit: self.cached_limit >= WINDOW_LIMIT,
                gap: false,
            };
        }
        let at_limit =
            self.limited || self.entries.len() >= WINDOW_LIMIT || self.seen.len() >= PAGE_LIMIT;
        HistoryInfo {
            can_load_newer: self.forward.is_some()
                && !at_limit
                && self.seen_forward.len() < PAGE_LIMIT,
            can_load_more: self.cursor.is_some() && self.seen.len() < PAGE_LIMIT,
            at_limit,
            gap: self.gap,
        }
    }
    pub(crate) fn snapshot(&self) -> (Vec<Message>, HistoryInfo) {
        (aggregate(&self.entries), self.info())
    }
}

fn deduplicate(entries: Vec<Entry>) -> Vec<Entry> {
    let mut seen = HashSet::new();
    entries
        .into_iter()
        .filter(|entry| seen.insert(entry.id.clone()))
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switching_keeps_expanded_windows_and_pagination_cursors() {
        let mut h = History {
            room_id: "!a:x".into(),
            entries: entries(&[1, 2, 3]),
            cursor: Some("older".into()),
            initialized: true,
            cached_limit: 100,
            ..Default::default()
        };
        let mut cache = std::collections::VecDeque::new();
        h.activate(&mut cache, "!b:x");
        assert!(!h.initialized);
        h.room_id = "!b:x".into();
        h.initialized = true;
        h.entries = entries(&[4]);
        h.activate(&mut cache, "!a:x");
        assert_eq!(ids(&h), ["1", "2", "3"]);
        assert_eq!(h.cursor.as_deref(), Some("older"));
        assert_eq!(h.cached_limit, 100);
        h.activate(&mut cache, "!a:x");
        assert_eq!(cache.len(), 1);
        assert_eq!(cache[0].room_id, "!b:x");
        for n in 0..40 {
            h.activate(&mut cache, &format!("!{n}:x"));
            h.room_id = format!("!{n}:x");
            h.initialized = true;
        }
        assert_eq!(cache.len(), 32);
    }

    fn entries(ids: &[usize]) -> Vec<Entry> {
        ids.iter()
            .map(|id| decode(serde_json::json!({"event_id":id.to_string(),"sender":"@a:local","type":"m.room.message","origin_server_ts":id,"content":{"msgtype":"m.text","body":format!("message {id}")}}),false))
            .collect()
    }
    fn ids(history: &History) -> Vec<String> {
        history
            .snapshot()
            .0
            .into_iter()
            .map(|message| message.id)
            .collect()
    }
    fn page(history: &mut History, kind: HistoryLoad, ids: &[usize], end: Option<&str>) {
        history
            .apply_page(kind, entries(ids), end.map(str::to_owned))
            .unwrap();
    }
    #[test]
    fn incremental_sync_retains_history_and_applies_relations_without_http_refresh() {
        let mut h = History::default();
        page(&mut h, HistoryLoad::Recent, &[1, 2], Some("older"));
        h.apply_sync(entries(&[2, 3]), false, Some("ignored".into()));
        assert_eq!(ids(&h), ["1", "2", "3"]);
        assert_eq!(h.cursor.as_deref(), Some("older"));
        let edit = decode(
            serde_json::json!({"type":"m.room.message","sender":"@a:local","event_id":"$edit","origin_server_ts":100,"content":{"msgtype":"m.text","body":"* edited","m.new_content":{"msgtype":"m.text","body":"edited"},"m.relates_to":{"rel_type":"m.replace","event_id":"2"}}}),
            false,
        );
        h.apply_sync(vec![edit], false, None);
        assert_eq!(h.snapshot().0[1].body.as_deref(), Some("edited"));
        h.apply_sync(entries(&[10, 11]), true, Some("gap".into()));
        assert!(h.info().gap);
        assert_eq!(ids(&h), ["10", "11"]);
    }
    #[test]
    fn older_overlap_and_sync_preserve_chronological_history_without_duplicates() {
        let mut h = History::default();
        page(&mut h, HistoryLoad::Recent, &[3, 4], Some("a"));
        page(&mut h, HistoryLoad::Older, &[1, 2, 3, 3], Some("b"));
        page(&mut h, HistoryLoad::Recent, &[4, 5, 6], Some("ignored"));
        assert_eq!(ids(&h), ["1", "2", "3", "4", "5", "6"]);
        assert_eq!(h.cursor.as_deref(), Some("b"));
        assert!(h.info().can_load_more && !h.info().gap);
        page(&mut h, HistoryLoad::Older, &[0, 1], None);
        assert_eq!(ids(&h), ["0", "1", "2", "3", "4", "5", "6"]);
        assert!(!h.info().can_load_more);
    }
    #[test]
    fn repeated_or_cyclic_cursor_fails_atomically_and_advancing_empty_pages_are_valid() {
        let mut h = History::default();
        page(&mut h, HistoryLoad::Recent, &[3], Some("a"));
        assert_eq!(
            h.apply_page(HistoryLoad::Older, entries(&[2]), Some("a".into())),
            Err("history-stalled")
        );
        assert_eq!(ids(&h), ["3"]);
        assert_eq!(h.cursor.as_deref(), Some("a"));
        page(&mut h, HistoryLoad::Older, &[], Some("b"));
        assert!(h.info().can_load_more);
        assert_eq!(
            h.apply_page(HistoryLoad::Older, entries(&[1]), Some("a".into())),
            Err("history-stalled")
        );
        assert_eq!(h.cursor.as_deref(), Some("b"));
        page(&mut h, HistoryLoad::Older, &[1], None);
        assert_eq!(ids(&h), ["1", "3"]);
    }
    #[test]
    fn disjoint_refresh_reports_gap_until_explicit_latest_reset() {
        let mut h = History::default();
        page(&mut h, HistoryLoad::Recent, &[1, 2], Some("a"));
        page(&mut h, HistoryLoad::Recent, &[10, 11], Some("b"));
        assert_eq!(ids(&h), ["10", "11"]);
        assert!(h.info().gap);
        page(&mut h, HistoryLoad::Recent, &[11, 12], Some("c"));
        assert!(h.info().gap);
        page(&mut h, HistoryLoad::Latest, &[11, 12], Some("c"));
        assert!(!h.info().gap);
        assert_eq!(ids(&h), ["11", "12"]);
        assert_eq!(h.cursor.as_deref(), Some("c"));
    }
    #[test]
    fn raw_event_overlap_replaces_redacted_body_and_counts_non_messages() {
        let mut h = History::default();
        page(&mut h, HistoryLoad::Recent, &[1, 2, 3], Some("a"));
        let redacted = serde_json::json!({
            "type":"m.room.message", "sender":"@a:local", "event_id":"$redacted",
            "origin_server_ts":1, "content":{}, "unsigned":{"redacted_because":{
                "type":"m.room.redaction", "sender":"@a:local", "event_id":"$redaction",
                "origin_server_ts":2, "content":{}, "redacts":"$redacted"
            }}
        });
        assert!(decode(redacted, false).message.unwrap().deleted);
        let mut refreshed = entries(&[2, 3, 4]);
        refreshed[0].message = None;
        h.apply_page(HistoryLoad::Recent, refreshed, Some("unused".into()))
            .unwrap();
        assert_eq!(ids(&h), ["1", "3", "4"]);
        assert_eq!(h.entries.len(), 4);
        assert_eq!(h.cursor.as_deref(), Some("a"));
    }
    #[test]
    fn event_window_is_bounded_and_latest_releases_limit() {
        let mut h = History::default();
        page(
            &mut h,
            HistoryLoad::Recent,
            &(0..WINDOW_LIMIT).collect::<Vec<_>>(),
            Some("a"),
        );
        assert!(h.info().at_limit && h.info().can_load_more);
        page(&mut h, HistoryLoad::Recent, &[499, 500, 501], Some("b"));
        assert_eq!(h.entries.len(), WINDOW_LIMIT);
        assert_eq!(h.entries[0].id, "2");
        assert!(h.info().at_limit);
        page(&mut h, HistoryLoad::Latest, &[500, 501], Some("b"));
        assert!(!h.info().at_limit && h.info().can_load_more);
    }
    #[test]
    fn empty_page_cursor_history_is_also_bounded() {
        let mut h = History::default();
        page(&mut h, HistoryLoad::Recent, &[], Some("0"));
        for i in 1..=PAGE_LIMIT {
            page(&mut h, HistoryLoad::Older, &[], Some(&i.to_string()));
        }
        assert!(h.info().at_limit && !h.info().can_load_more);
        assert!(h.entries.is_empty());
        page(&mut h, HistoryLoad::Latest, &[], None);
        assert!(!h.info().at_limit && !h.info().can_load_more);
        assert!(h.seen.is_empty());
    }
}
