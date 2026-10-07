//! Bounded, loaded-window relation projection. Never treats replacements as new messages.
use crate::{Message, Reaction};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Default)]
pub(crate) struct Entry {
    pub id: String,
    pub message: Option<Message>,
    encrypted: Option<Value>,
    sender: String,
    timestamp: u64,
    msgtype: String,
    room_message: bool,
    bundled: Option<Box<Entry>>,
    relation: Option<Relation>,
}
#[derive(Clone)]
enum Relation {
    Edit {
        target: String,
        body: String,
        formatted_body: Option<String>,
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
                    formatted_body: formatted(new),
                    msgtype: msgtype.into(),
                });
            }
        }
        Some("m.sticker" | "org.matrix.msc3381.poll.start") => {
            let body = content["body"]
                .as_str()
                .or_else(|| content["org.matrix.msc1767.text"].as_str())
                .unwrap_or("Poll")
                .to_owned();
            entry.message = Some(Message {
                id: entry.id.clone(),
                sender: entry.sender.clone(),
                timestamp: entry.timestamp,
                body: (!deleted).then_some(body),
                deleted,
                ..Message::default()
            });
        }
        Some("m.room.message" | "m.room.encrypted") => {
            let undecrypted = !deleted && !entry.room_message;
            entry.encrypted = undecrypted.then(|| value.clone());
            entry.message = Some(Message {
                timestamp: entry.timestamp,
                thread_root: (relation["rel_type"] == "m.thread")
                    .then(|| relation["event_id"].as_str().map(str::to_owned))
                    .flatten(),
                id: entry.id.clone(),
                sender: entry.sender.clone(),
                undecrypted,
                body: (!deleted && !undecrypted)
                    .then(|| content["body"].as_str().map(str::to_owned))
                    .flatten(),
                formatted_body: (!deleted && !undecrypted)
                    .then(|| formatted(content))
                    .flatten(),
                editable: !deleted && !undecrypted && entry.msgtype == "m.text",
                deleted,
                reply_to: relation["m.in_reply_to"]["event_id"]
                    .as_str()
                    .map(str::to_owned),
                attachment: if deleted || undecrypted {
                    None
                } else {
                    attachment(content)
                },
                ..Message::default()
            });
        }
        _ => {}
    }
    if let Some(message) = &mut entry.message {
        message.transaction_id = value["unsigned"]["transaction_id"]
            .as_str()
            .map(str::to_owned);
    }
    entry.bundled = bundled_replacement(&value, redacts_in_content);

    entry
}

/// Retry only ciphertext retained in the bounded loaded window. Never refetch
/// history or change pagination when a key arrives via verification/backup.
pub(crate) async fn retry_decryption(entries: &mut [Entry], room: &matrix_sdk::Room) -> bool {
    let rules = room.clone_info().room_version_rules_or_default().redaction;
    let mut changed = false;
    for entry in entries {
        let Some(value) = &entry.encrypted else {
            continue;
        };
        let Ok(raw) = matrix_sdk::ruma::serde::Raw::new(value) else {
            continue;
        };
        let Ok(event) = room.decrypt_event(&raw.cast_unchecked(), None).await else {
            continue;
        };
        let Ok(value) = crate::details::value(room, &event) else {
            continue;
        };
        if value["type"] == "m.room.encrypted" {
            continue;
        }
        let decoded = decode(value, rules.keep_room_redaction_redacts);
        if decoded.id == entry.id {
            *entry = decoded;
            changed = true;
        }
    }
    changed
}

fn attachment(content: &Value) -> Option<crate::Attachment> {
    matches!(
        content["msgtype"].as_str(),
        Some("m.file" | "m.image" | "m.video" | "m.audio")
    )
    .then(|| crate::Attachment {
        name: content["filename"]
            .as_str()
            .or(content["body"].as_str())
            .unwrap_or("attachment")
            .to_owned(),
        kind: string(content, "msgtype"),
        size: content["info"]["size"].as_u64(),
    })
}
fn bundled_replacement(value: &Value, redacts_in_content: bool) -> Option<Box<Entry>> {
    let mut bundled = value["unsigned"]["m.relations"].get("m.replace")?.clone();
    // Strip nested bundles before decoding to bound recursion on untrusted content.
    bundled.as_object_mut()?.remove("unsigned");
    if bundled["room_id"]
        .as_str()
        .is_some_and(|room| value["room_id"].as_str() != Some(room))
    {
        return None;
    }
    serde_json::from_value::<matrix_sdk::ruma::events::AnySyncTimelineEvent>(bundled.clone())
        .ok()?;
    Some(Box::new(decode(bundled, redacts_in_content)))
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
    receipt_anchors(entries, &positions, &mut messages);
    let originals: HashMap<_, _> = entries
        .iter()
        .filter(|e| e.message.is_some())
        .map(|e| (e.id.as_str(), e))
        .collect();
    let mut edits: HashMap<&str, (u64, &str)> = HashMap::new();
    let bundled: Vec<_> = entries
        .iter()
        .filter_map(|entry| entry.bundled.as_deref())
        .filter(|b| !entries.iter().any(|e| e.id == b.id))
        .collect();
    for entry in entries.iter().chain(bundled) {
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
        if message.reply_to.is_some()
            && let Some(body) = &mut message.body
        {
            *body = matrix_sdk::ruma::events::room::message::sanitize::remove_plain_reply_fallback(
                body,
            )
            .to_owned();
        }
        if message.deleted || redacted.contains(message.id.as_str()) {
            message.deleted = true;
            message.body = None;
            message.formatted_body = None;
            message.editable = false;
            message.edited = false;
            message.reply_to = None;
            message.reactions.clear();
            message.attachment = None;
        }
    }
    messages
}

// A receipt may name a reaction, edit, or state event that has no visible row.
// Place it on the last visible event preceding it, never on the edited target.
fn receipt_anchors(
    entries: &[Entry],
    positions: &HashMap<String, usize>,
    messages: &mut [Message],
) {
    let mut main = None;
    let mut threads = HashMap::<String, usize>::new();
    for entry in entries {
        if let Some(&i) = positions.get(&entry.id) {
            let message = &messages[i];
            if message.thread_root.is_none() {
                main = Some(i);
            }
            threads.insert(
                message.thread_root.as_ref().unwrap_or(&message.id).clone(),
                i,
            );
            continue;
        }
        if let Some(i) = main {
            messages[i].receipt_events.push(entry.id.clone());
        }
        let target = match &entry.relation {
            Some(
                Relation::Edit { target, .. }
                | Relation::Reaction { target, .. }
                | Relation::Redaction(target),
            ) => target,
            None => continue,
        };
        if let Some(&i) = positions.get(target) {
            let root = messages[i].thread_root.as_ref().unwrap_or(&messages[i].id);
            if let Some(&last) = threads.get(root) {
                messages[last].thread_receipt_events.push(entry.id.clone());
            }
        }
    }
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
        formatted_body,
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
    message.formatted_body = formatted_body.clone();
    message.edited = true;
    message.editable = original.msgtype == "m.text" && msgtype == "m.text";
    edits.insert(target, rank);
}

pub(crate) fn revisions(entries: &[Entry], target: &str) -> Vec<String> {
    let Some(original) = entries
        .iter()
        .find(|e| e.id == target && e.message.is_some())
    else {
        return vec![];
    };
    if aggregate(entries)
        .iter()
        .find(|m| m.id == target)
        .is_none_or(|m| m.deleted)
    {
        return vec![];
    }
    let redacted: HashSet<_> = entries
        .iter()
        .filter_map(|e| match &e.relation {
            Some(Relation::Redaction(id)) => Some(id),
            _ => None,
        })
        .collect();
    let mut edits: Vec<_> = entries
        .iter()
        .filter(|e| e.sender == original.sender && !redacted.contains(&e.id))
        .filter_map(|e| match &e.relation {
            Some(Relation::Edit {
                target: t, body, ..
            }) if t == target => Some((e.timestamp, &e.id, body)),
            _ => None,
        })
        .collect();
    edits.sort();
    edits
        .into_iter()
        .map(|(timestamp, id, body)| format!("{timestamp} · {id}\n{body}"))
        .collect()
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
    fn reply_fallback_is_removed_without_removing_real_quotes() {
        let body = "> <@a:local> Original\n> second line\n\n> Actual quote\nMy reply";
        let reply = event(
            "$reply",
            "@b:local",
            "m.room.message",
            json!({
                "msgtype":"m.text", "body":body,
                "m.relates_to":{"m.in_reply_to":{"event_id":"$original"}}
            }),
            2,
        );
        let messages = aggregate(&[text("$original", "Original"), reply, text("$quote", body)]);
        assert_eq!(
            messages[1].body.as_deref(),
            Some("> Actual quote\nMy reply")
        );
        assert_eq!(messages[1].reply_to.as_deref(), Some("$original"));
        assert_eq!(messages[2].body.as_deref(), Some(body));
    }

    #[test]
    fn ciphertext_never_exposes_untrusted_plaintext_fields() {
        let value = json!({"event_id":"$encrypted","type":"m.room.encrypted","sender":"@a:local","origin_server_ts":1,
            "content":{"body":"untrusted","msgtype":"m.image","url":"mxc://local/untrusted"}});
        let entry = decode(value, false);
        assert!(entry.encrypted.is_some());
        let message = entry.message.unwrap();
        assert!(message.undecrypted);
        assert!(message.body.is_none() && message.attachment.is_none() && !message.editable);
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
    #[test]
    fn bundled_edits_are_validated_and_live_redaction_overrides_bundle() {
        let original = json!({"type":"m.room.message","event_id":"$original","room_id":"!room:local","sender":"@a:local","origin_server_ts":1,"content":{"msgtype":"m.text","body":"Original"}});
        let replacement = json!({"type":"m.room.message","event_id":"$edit","sender":"@a:local","origin_server_ts":2,"content":{"msgtype":"m.text","body":"* Edited","m.new_content":{"msgtype":"m.text","body":"Edited"},"m.relates_to":{"rel_type":"m.replace","event_id":"$original"}}});
        let mut raw = original.clone();
        raw["unsigned"] = json!({"m.relations":{"m.replace":replacement}});
        let entry = decode(raw.clone(), true);
        assert_eq!(
            aggregate(std::slice::from_ref(&entry))[0].body.as_deref(),
            Some("Edited")
        );
        assert_eq!(
            aggregate(&[entry, redact("$edit")])[0].body.as_deref(),
            Some("Original")
        );
        raw["unsigned"]["m.relations"]["m.replace"]["sender"] = json!("@foreign:local");
        assert_eq!(
            aggregate(&[decode(raw.clone(), true)])[0].body.as_deref(),
            Some("Original")
        );
        raw["unsigned"]["m.relations"]["m.replace"]["sender"] = json!("@a:local");
        raw["unsigned"]["m.relations"]["m.replace"]["room_id"] = json!("!other:local");
        assert_eq!(
            aggregate(&[decode(raw, true)])[0].body.as_deref(),
            Some("Original")
        );
    }
    #[test]
    fn redaction_removes_attachment_and_revision_history() {
        let attachment = event(
            "$original",
            "@a:local",
            "m.room.message",
            json!({"msgtype":"m.file","body":"File","filename":"report.pdf","url":"mxc://local/file","info":{"size":42}}),
            1,
        );
        assert_eq!(
            aggregate(std::slice::from_ref(&attachment))[0]
                .attachment
                .as_ref()
                .unwrap()
                .name,
            "report.pdf"
        );
        let entries = [
            attachment,
            edit("$edit", "@a:local", "Changed", 2),
            redact("$original"),
        ];
        assert!(aggregate(&entries)[0].attachment.is_none());
        assert!(revisions(&entries, "$original").is_empty());
    }
}

fn formatted(content: &Value) -> Option<String> {
    (content["format"] == "org.matrix.custom.html")
        .then(|| {
            content["formatted_body"]
                .as_str()
                .filter(|s| s.len() <= 65536)
                .map(str::to_owned)
        })
        .flatten()
}
