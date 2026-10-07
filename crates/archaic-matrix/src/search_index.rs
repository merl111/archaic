//! Encrypted persistent event index. Relations are retained so edits/redactions change hits.
use crate::{SearchPage, timeline};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
#[derive(Default, Serialize, Deserialize)]
pub(crate) struct Index {
    pub(crate) rooms: BTreeMap<String, IndexedRoom>,
}
#[derive(Default, Serialize, Deserialize)]
pub(crate) struct IndexedRoom {
    pub(crate) events: BTreeMap<String, Value>,
    #[serde(default)]
    pub(crate) redacted: HashSet<String>,
    next: Option<String>,
    started: bool,
    seen: HashSet<String>,
}
impl Index {
    #[cfg(test)]
    pub fn merge(
        &mut self,
        room: &str,
        values: Vec<Value>,
        redacts_in_content: bool,
    ) -> Result<bool, &'static str> {
        if values.is_empty() {
            return Ok(false);
        }
        let indexed = self.rooms.entry(room.to_owned()).or_default();
        let additions = values
            .iter()
            .filter(|v| {
                v["event_id"]
                    .as_str()
                    .is_some_and(|id| !indexed.events.contains_key(id))
            })
            .count();
        if indexed.events.len() + additions > 100_000 {
            return Err("tool-limit");
        }
        let mut changed = false;
        // Keep tombstones across pagination: older responses cannot resurrect redacted content.
        for value in &values {
            if value["unsigned"].get("redacted_because").is_some()
                && let Some(id) = value["event_id"].as_str()
            {
                changed |= indexed.redacted.insert(id.to_owned());
            }
            if value["type"] == "m.room.redaction"
                && let Some(id) = (if redacts_in_content {
                    &value["content"]["redacts"]
                } else {
                    &value["redacts"]
                })
                .as_str()
            {
                changed |= indexed.redacted.insert(id.to_owned());
            }
        }
        for mut v in values {
            let Some(id) = v["event_id"].as_str().map(str::to_owned) else {
                continue;
            };
            erase_redacted(&mut v, &indexed.redacted);
            if indexed.events.get(&id) != Some(&v) {
                indexed.events.insert(id, v);
                changed = true;
            }
        }
        for value in indexed.events.values_mut() {
            changed |= erase_redacted(value, &indexed.redacted);
        }
        Ok(changed)
    }
}

#[cfg(test)]
fn erase_redacted(value: &mut Value, redacted: &HashSet<String>) -> bool {
    let tombstone = value["event_id"]
        .as_str()
        .is_some_and(|id| redacted.contains(id));
    let edit = value["content"]["m.relates_to"]["rel_type"] == "m.replace"
        && value["content"]["m.relates_to"]["event_id"]
            .as_str()
            .is_some_and(|id| redacted.contains(id));
    if !tombstone && !edit {
        return false;
    }
    let already = value["content"] == serde_json::json!({})
        && value["unsigned"] == serde_json::json!({"redacted_because":{}});
    value["content"] = serde_json::json!({});
    value["unsigned"] = serde_json::json!({"redacted_because":{}});
    !already
}
#[derive(Default)]
pub(crate) struct Filter {
    pub(crate) text: String,
    sender: Option<String>,
    files: bool,
    after: Option<u64>,
    before: Option<u64>,
}
impl Filter {
    pub(crate) fn parse(query: &str) -> Result<Self, &'static str> {
        if query.trim().is_empty() || query.len() > 1024 {
            return Err("search-invalid");
        }
        let mut f = Self::default();
        let mut terms = Vec::new();
        for term in query.split_whitespace() {
            if let Some(user) = term.strip_prefix("from:") {
                matrix_sdk::ruma::UserId::parse(user).map_err(|_| "search-invalid")?;
                f.sender = Some(user.into());
            } else if let Some(value) = term.strip_prefix("after:") {
                f.after = Some(value.parse().map_err(|_| "search-invalid")?);
            } else if let Some(value) = term.strip_prefix("before:") {
                f.before = Some(value.parse().map_err(|_| "search-invalid")?);
            } else if term == "has:file" {
                f.files = true;
            } else {
                terms.push(term);
            }
        }
        f.text = terms.join(" ").to_lowercase();
        if matches!((f.after,f.before),(Some(a),Some(b)) if a>=b) {
            return Err("search-invalid");
        }
        Ok(f)
    }
}
pub(crate) fn project(room: &IndexedRoom, filter: &Filter, redacts_in_content: bool) -> SearchPage {
    let entries = room
        .events
        .values()
        .cloned()
        .map(|v| timeline::decode(v, redacts_in_content))
        .collect::<Vec<_>>();
    let mut messages = timeline::aggregate(&entries)
        .into_iter()
        .filter(|m| {
            let timestamp = room
                .events
                .get(&m.id)
                .and_then(|v| v["origin_server_ts"].as_u64())
                .unwrap_or_default();
            !m.deleted
                && m.body
                    .as_ref()
                    .is_some_and(|b| b.to_lowercase().contains(&filter.text))
                && filter.sender.as_ref().is_none_or(|s| s == &m.sender)
                && (!filter.files || m.attachment.is_some())
                && filter.after.is_none_or(|a| timestamp >= a)
                && filter.before.is_none_or(|b| timestamp < b)
        })
        .collect::<Vec<_>>();
    messages.sort_by_key(|m| {
        std::cmp::Reverse(
            room.events
                .get(&m.id)
                .and_then(|v| v["origin_server_ts"].as_u64())
                .unwrap_or_default(),
        )
    });
    let truncated = messages.len() > 1000;
    messages.truncate(1000);
    SearchPage {
        truncated,
        unavailable: room
            .events
            .values()
            .filter(|v| v["type"] == "m.room.encrypted")
            .count(),
        scanned: room.events.len(),
        more: !room.started || room.next.is_some(),
        messages,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn persisted_tombstones_reject_old_history_and_late_edits() {
        let mut index = Index::default();
        index.merge("!a:local", vec![json!({"type":"m.room.redaction","event_id":"$redaction","sender":"@a:local","content":{"redacts":"$original"}})], true).unwrap();
        let bytes = serde_json::to_vec(&index).unwrap();
        let mut index: Index = serde_json::from_slice(&bytes).unwrap();
        index.merge("!a:local", vec![
            json!({"type":"m.room.message","event_id":"$original","sender":"@a:local","content":{"msgtype":"m.text","body":"private original"}}),
            json!({"type":"m.room.message","event_id":"$edit","sender":"@a:local","content":{"msgtype":"m.text","body":"private replacement","m.relates_to":{"rel_type":"m.replace","event_id":"$original"},"m.new_content":{"msgtype":"m.text","body":"private replacement"}}})
        ], true).unwrap();
        assert!(
            project(
                &index.rooms["!a:local"],
                &Filter::parse("private").unwrap(),
                true
            )
            .messages
            .is_empty()
        );
        assert!(
            !String::from_utf8(serde_json::to_vec(&index).unwrap())
                .unwrap()
                .contains("private")
        );
    }
    #[test]
    fn index_applies_edits_redactions_filters_and_roundtrips() {
        let mut index = Index::default();
        index.merge("!a:local",vec![json!({"type":"m.room.message","event_id":"$a","sender":"@a:local","origin_server_ts":20,"content":{"msgtype":"m.text","body":"original"}}), json!({"type":"m.room.message","event_id":"$e","sender":"@a:local","origin_server_ts":21,"content":{"msgtype":"m.text","body":"* changed","m.relates_to":{"rel_type":"m.replace","event_id":"$a"},"m.new_content":{"msgtype":"m.text","body":"changed"}}})],false).unwrap();
        let bytes = serde_json::to_vec(&index).unwrap();
        let mut index: Index = serde_json::from_slice(&bytes).unwrap();
        let room = &index.rooms["!a:local"];
        assert_eq!(
            project(
                room,
                &Filter::parse("changed from:@a:local after:10 before:30").unwrap(),
                false
            )
            .messages
            .len(),
            1
        );
        assert!(
            project(room, &Filter::parse("original").unwrap(), false)
                .messages
                .is_empty()
        );
        index.merge("!a:local",vec![json!({"type":"m.room.redaction","event_id":"$r","sender":"@a:local","redacts":"$a","content":{}})],false).unwrap();
        assert!(
            project(
                &index.rooms["!a:local"],
                &Filter::parse("changed").unwrap(),
                false
            )
            .messages
            .is_empty()
        );
    }
}
