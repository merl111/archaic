use crate::{HistoryInfo, HistoryLoad, Message};
use matrix_sdk::{
    Room,
    room::MessagesOptions,
    ruma::{
        events::{AnySyncMessageLikeEvent, AnySyncTimelineEvent, SyncMessageLikeEvent},
        uint,
    },
};
use std::collections::HashSet;

const PAGE_SIZE: usize = 50;
const WINDOW_LIMIT: usize = 500;
const PAGE_LIMIT: usize = 128;

#[derive(Clone)]
struct Entry {
    id: String,
    message: Option<Message>,
}

#[derive(Default)]
pub(crate) struct History {
    room_id: String,
    entries: Vec<Entry>,
    cursor: Option<String>,
    seen: HashSet<String>,
    initialized: bool,
    limited: bool,
    gap: bool,
}

impl History {
    pub async fn load(
        &mut self,
        room: &Room,
        kind: HistoryLoad,
    ) -> Result<(Vec<Message>, HistoryInfo), &'static str> {
        if self.room_id != room.room_id().as_str() {
            *self = Self {
                room_id: room.room_id().to_string(),
                ..Self::default()
            };
        }
        if kind == HistoryLoad::Older && (!self.initialized || !self.info().can_load_more) {
            return Ok(self.snapshot());
        }
        let mut options = MessagesOptions::backward();
        options.limit = uint!(50);
        if kind == HistoryLoad::Older {
            options.from = self.cursor.clone();
        }
        let response = room
            .messages(options)
            .await
            .map_err(|_| "history-load-failed")?;
        if response.chunk.len() > PAGE_SIZE {
            return Err("history-invalid-page");
        }
        let entries = response
            .chunk
            .into_iter()
            .rev()
            .map(|event| {
                let event = event
                    .raw()
                    .deserialize()
                    .map_err(|_| "history-invalid-page")?;
                Ok(decode(event))
            })
            .collect::<Result<Vec<_>, &'static str>>()?;
        self.apply_page(kind, entries, response.end)?;
        Ok(self.snapshot())
    }
    fn apply_page(
        &mut self,
        kind: HistoryLoad,
        entries: Vec<Entry>,
        end: Option<String>,
    ) -> Result<(), &'static str> {
        if kind == HistoryLoad::Older {
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
    fn info(&self) -> HistoryInfo {
        let at_limit =
            self.limited || self.entries.len() >= WINDOW_LIMIT || self.seen.len() >= PAGE_LIMIT;
        HistoryInfo {
            can_load_more: self.cursor.is_some() && !at_limit,
            at_limit,
            gap: self.gap,
        }
    }
    fn snapshot(&self) -> (Vec<Message>, HistoryInfo) {
        (
            self.entries
                .iter()
                .filter_map(|entry| entry.message.clone())
                .collect(),
            self.info(),
        )
    }
}

fn deduplicate(entries: Vec<Entry>) -> Vec<Entry> {
    let mut seen = HashSet::new();
    entries
        .into_iter()
        .filter(|entry| seen.insert(entry.id.clone()))
        .collect()
}
fn decode(event: AnySyncTimelineEvent) -> Entry {
    let id = event.event_id().to_string();
    let message = match event {
        AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::RoomMessage(
            SyncMessageLikeEvent::Original(message),
        )) => Some(Message {
            id: id.clone(),
            sender: message.sender.to_string(),
            body: Some(message.content.body().to_owned()),
        }),
        AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::RoomEncrypted(message)) => {
            Some(Message {
                id: id.clone(),
                sender: message.sender().to_string(),
                body: None,
            })
        }
        _ => None,
    };
    Entry { id, message }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(ids: &[usize]) -> Vec<Entry> {
        ids.iter()
            .map(|id| Entry {
                id: id.to_string(),
                message: Some(Message {
                    id: id.to_string(),
                    sender: "@a:local".into(),
                    body: Some(format!("message {id}")),
                }),
            })
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
        let redacted: AnySyncTimelineEvent = serde_json::from_value(serde_json::json!({
            "type":"m.room.message", "sender":"@a:local", "event_id":"$redacted",
            "origin_server_ts":1, "content":{}, "unsigned":{"redacted_because":{
                "type":"m.room.redaction", "sender":"@a:local", "event_id":"$redaction",
                "origin_server_ts":2, "content":{}, "redacts":"$redacted"
            }}
        }))
        .unwrap();
        assert!(decode(redacted).message.is_none());
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
        assert!(h.info().at_limit && !h.info().can_load_more);
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
