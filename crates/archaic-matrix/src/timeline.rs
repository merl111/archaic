//! Bounded, loaded-window relation projection. Never treats replacements as new messages.
use crate::{Message, Reaction};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Default)]
pub(crate) struct Entry {
    pub id: String,
    pub message: Option<Message>,
    sender: String,
    timestamp: u64,
    msgtype: String,
    room_message: bool,
    relation: Option<Relation>,
}
#[derive(Clone)]
enum Relation {
    Edit {
        target: String,
        body: String,
        msgtype: String,
    },
    Reaction {
        target: String,
        key: String,
    },
    Redaction(String),
}
fn string(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_owned()
}
pub(crate) fn decode(value: Value, redacts_in_content: bool) -> Entry {
    // SDK has already validated/decrypted the event. JSON keeps this projection independent
    // of enum variants; the caller supplies the room version's redaction format.
    let content = &value["content"];
    let relation = &content["m.relates_to"];
    let mut entry = Entry {
        id: string(&value, "event_id"),
        sender: string(&value, "sender"),
        timestamp: value["origin_server_ts"].as_u64().unwrap_or_default(),
        msgtype: string(content, "msgtype"),
        room_message: value["type"] == "m.room.message",
        ..Entry::default()
    };
    if value.get("state_key").is_some() {
        return entry;
    }
    let deleted = value["unsigned"].get("redacted_because").is_some();
    match value["type"].as_str() {
        Some("m.room.redaction") => {
            if let Some(id) = (if redacts_in_content {
                &content["redacts"]
            } else {
                &value["redacts"]
            })
            .as_str()
            {
                entry.relation = Some(Relation::Redaction(id.into()));
            }
        }
        Some("m.reaction") if !deleted && relation["rel_type"] == "m.annotation" => {
            if let (Some(target), Some(key)) =
                (relation["event_id"].as_str(), relation["key"].as_str())
            {
                entry.relation = Some(Relation::Reaction {
                    target: target.into(),
                    key: key.into(),
                });
            }
        }
        Some("m.room.message") if !deleted && relation["rel_type"] == "m.replace" => {
            let new = &content["m.new_content"];
            if let (Some(target), Some(body), Some(msgtype)) = (
                relation["event_id"].as_str(),
                new["body"].as_str(),
                new["msgtype"].as_str(),
            ) {
                entry.relation = Some(Relation::Edit {
                    target: target.into(),
                    body: body.into(),
                    msgtype: msgtype.into(),
                });
            }
        }
        Some("m.room.message" | "m.room.encrypted") => {
            entry.message = Some(Message {
                id: entry.id.clone(),
                sender: entry.sender.clone(),
                body: (!deleted)
                    .then(|| content["body"].as_str().map(str::to_owned))
                    .flatten(),
                editable: !deleted && entry.msgtype == "m.text",
                deleted,
                reply_to: relation["m.in_reply_to"]["event_id"]
                    .as_str()
                    .map(str::to_owned),
                ..Message::default()
            });
        }
        _ => {}
    }
    entry
}

pub(crate) fn aggregate(entries: &[Entry]) -> Vec<Message> {
    // A redaction remains effective even if its redaction event is itself redacted.
    let redacted: HashSet<_> = entries
        .iter()
        .filter_map(|entry| match &entry.relation {
            Some(Relation::Redaction(id)) => Some(id.as_str()),
            _ => None,
        })
        .collect();
    let mut messages: Vec<_> = entries
        .iter()
        .filter_map(|entry| entry.message.clone())
        .collect();
    let positions: HashMap<_, _> = messages
        .iter()
        .enumerate()
        .map(|(i, m)| (m.id.clone(), i))
        .collect();
    let originals: HashMap<_, _> = entries
        .iter()
        .filter(|e| e.message.is_some())
        .map(|e| (e.id.as_str(), e))
        .collect();
    let mut edits: HashMap<&str, (u64, &str)> = HashMap::new();
    for entry in entries {
        if redacted.contains(entry.id.as_str()) {
            continue;
        }
        match &entry.relation {
            Some(Relation::Edit { target, .. }) => {
                if let Some(&index) = positions.get(target) {
                    apply_edit(entry, &originals, &mut edits, &mut messages[index]);
                }
            }
            Some(Relation::Reaction { target, key }) => {
                if let Some(&index) = positions.get(target) {
                    messages[index].reactions.push(Reaction {
                        id: entry.id.clone(),
                        sender: entry.sender.clone(),
                        key: key.clone(),
                    });
                }
            }
            _ => {}
        }
    }
    for message in &mut messages {
        if message.deleted || redacted.contains(message.id.as_str()) {
            message.deleted = true;
            message.body = None;
            message.editable = false;
            message.edited = false;
            message.reply_to = None;
            message.reactions.clear();
        }
    }
    messages
}

fn apply_edit<'a>(
    entry: &'a Entry,
    originals: &HashMap<&str, &Entry>,
    edits: &mut HashMap<&'a str, (u64, &'a str)>,
    message: &mut Message,
) {
    let Some(Relation::Edit {
        target,
        body,
        msgtype,
    }) = &entry.relation
    else {
        return;
    };
    let target = target.as_str();
    let Some(original) = originals.get(target) else {
        return;
    };
    let rank = (entry.timestamp, entry.id.as_str());
    if !original.room_message
        || original.sender != entry.sender
        || edits.get(target).is_some_and(|previous| *previous >= rank)
    {
        return;
    }
    message.body = Some(body.to_owned());
    message.edited = true;
    message.editable = original.msgtype == "m.text" && msgtype == "m.text";
    edits.insert(target, rank);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn event(id: &str, sender: &str, kind: &str, content: Value, time: u64) -> Entry {
        decode(
            json!({"event_id":id,"sender":sender,"type":kind,"content":content,"origin_server_ts":time}),
            true,
        )
    }
    fn text(id: &str, body: &str) -> Entry {
        event(
            id,
            "@a:local",
            "m.room.message",
            json!({"body":body,"msgtype":"m.text"}),
            1,
        )
    }
    fn edit(id: &str, sender: &str, body: &str, time: u64) -> Entry {
        event(
            id,
            sender,
            "m.room.message",
            json!({"body":"* fallback","msgtype":"m.text","m.relates_to":{"rel_type":"m.replace","event_id":"$original"},"m.new_content":{"body":body,"msgtype":"m.text"}}),
            time,
        )
    }
    fn redact(target: &str) -> Entry {
        event(
            "$redact",
            "@a:local",
            "m.room.redaction",
            json!({"redacts":target}),
            50,
        )
    }
    #[test]
    fn edits_use_latest_valid_same_author_content_and_redaction_restores_previous() {
        let mut entries = vec![
            text("$original", "first"),
            edit("$new", "@a:local", "newest", 20),
            edit("$old", "@a:local", "older", 10),
            edit("$forged", "@b:local", "forged", 30),
        ];
        let result = aggregate(&entries);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].body.as_deref(), Some("newest"));
        assert!(result[0].edited);
        entries.push(redact("$new"));
        assert_eq!(aggregate(&entries)[0].body.as_deref(), Some("older"));
        entries.push(redact("$original"));
        let result = aggregate(&entries);
        assert!(result[0].deleted && !result[0].editable && !result[0].edited);
        assert!(result[0].body.is_none());
    }
    #[test]
    fn changed_msgtypes_are_valid_but_orphan_edits_do_not_become_messages() {
        let mut invalid = edit("$bad", "@a:local", "bad", 20);
        invalid.msgtype = "m.image".into();
        let result = aggregate(&[invalid.clone(), text("$original", "original")]);
        assert!(result[0].edited);
        assert!(aggregate(&[invalid]).is_empty());
    }
    #[test]
    fn replies_and_reactions_keep_targets_and_redactions_remove_only_matching_reaction() {
        let entries = vec![
            text("$original", "hello"),
            event(
                "$reply",
                "@b:local",
                "m.room.message",
                json!({"body":"reply","msgtype":"m.text","m.relates_to":{"m.in_reply_to":{"event_id":"$original"}}}),
                2,
            ),
            event(
                "$r1",
                "@a:local",
                "m.reaction",
                json!({"m.relates_to":{"rel_type":"m.annotation","event_id":"$original","key":"👍"}}),
                3,
            ),
            event(
                "$r2",
                "@b:local",
                "m.reaction",
                json!({"m.relates_to":{"rel_type":"m.annotation","event_id":"$original","key":"👍"}}),
                4,
            ),
            redact("$r1"),
        ];
        let result = aggregate(&entries);
        assert_eq!(result.len(), 2);
        assert_eq!(result[1].reply_to.as_deref(), Some("$original"));
        assert_eq!(result[0].reactions.len(), 1);
        assert_eq!(result[0].reactions[0].id, "$r2");
    }
    #[test]
    fn replacement_cannot_supply_content_for_an_undecrypted_original() {
        let encrypted = event(
            "$original",
            "@a:local",
            "m.room.encrypted",
            json!({"algorithm":"m.megolm.v1.aes-sha2", "msgtype":"m.text"}),
            1,
        );
        let messages = aggregate(&[
            encrypted,
            edit("$fake", "@a:local", "unverified plaintext", 2),
        ]);
        assert!(messages[0].body.is_none() && !messages[0].edited);
    }
    #[test]
    fn room_version_selects_the_authoritative_redaction_target() {
        let value = json!({"type":"m.room.redaction","event_id":"$r","sender":"@a:local","redacts":"$old","content":{"redacts":"$new"}});
        assert!(
            matches!(decode(value.clone(), false).relation, Some(Relation::Redaction(id)) if id == "$old")
        );
        assert!(
            matches!(decode(value, true).relation, Some(Relation::Redaction(id)) if id == "$new")
        );
    }
    #[test]
    fn legacy_redaction_and_server_redacted_original_cannot_be_resurrected() {
        let redacted = decode(
            json!({"type":"m.room.message","event_id":"$original","sender":"@a:local","content":{},"unsigned":{"redacted_because":{}}}),
            true,
        );
        assert!(aggregate(&[redacted, edit("$edit", "@a:local", "resurrect", 20)])[0].deleted);
        let legacy = decode(
            json!({"type":"m.room.redaction","event_id":"$redact","sender":"@a:local","content":{},"redacts":"$original"}),
            false,
        );
        assert!(aggregate(&[text("$original", "secret"), legacy])[0].deleted);
    }
}
