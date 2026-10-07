use crate::{
    MessageDetails,
    message_actions::failure,
    paging::Cursor,
    timeline::{self, Entry},
};
use matrix_sdk::{
    Room,
    deserialized_responses::TimelineEvent,
    room::RelationsOptions,
    ruma::{
        EventId,
        events::receipt::{ReceiptThread, ReceiptType},
        uint,
    },
};
use serde_json::Value;
#[derive(Clone, Default)]
pub(crate) struct Details {
    key: Option<(String, String)>,
    entries: Vec<Entry>,
    cursor: Cursor,
    latest: Option<MessageDetails>,
}
impl Details {
    pub async fn retry_decryption(&mut self, room: &Room) -> Option<MessageDetails> {
        let (room_id, root) = self.key.as_ref()?;
        if room_id != room.room_id().as_str() {
            return None;
        }
        let mut changed = timeline::retry_decryption(&mut self.entries, room).await;
        let mut latest = self.latest.clone()?;
        if latest.reply.as_ref().is_some_and(|reply| reply.undecrypted)
            && let Some(target) = &latest.message.reply_to
            && let Ok(reply) = fetch_message(room, target).await
            && !reply.undecrypted
        {
            latest.reply = Some(reply);
            latest.reply_unavailable = false;
            changed = true;
        }
        if !changed {
            return None;
        }
        let messages = timeline::aggregate(&self.entries);
        latest.message = messages.iter().find(|m| &m.id == root)?.clone();
        latest.thread = messages
            .into_iter()
            .filter(|m| m.thread_root.as_ref() == Some(root))
            .collect();
        latest.revisions = timeline::revisions(&self.entries, root);
        latest.unavailable = self
            .entries
            .iter()
            .filter(|e| {
                e.message
                    .as_ref()
                    .is_some_and(|m| !m.deleted && m.body.is_none())
            })
            .count();
        if let Some(target) = &latest.message.reply_to {
            match fetch_message(room, target).await {
                Ok(reply) => {
                    latest.reply = Some(reply);
                    latest.reply_unavailable = false;
                }
                Err(_) => {
                    latest.reply = None;
                    latest.reply_unavailable = true;
                }
            }
        }
        crate::receipts::populate_details(room, &mut latest).await;
        self.latest = Some(latest.clone());
        Some(latest)
    }

    pub async fn sync(
        &mut self,
        room: &Room,
        update: &matrix_sdk::sync::JoinedRoomUpdate,
    ) -> Result<Option<MessageDetails>, &'static str> {
        let Some((room_id, root)) = &self.key else {
            return Ok(None);
        };
        if room_id != room.room_id().as_str() {
            return Ok(None);
        }
        let root = root.clone();
        if update.timeline.limited {
            return self.load(room, &root, false).await.map(Some);
        }
        let Some(mut latest) = self.latest.clone() else {
            return Ok(None);
        };
        let rules = room.clone_info().room_version_rules_or_default().redaction;
        let mut changed = false;
        for event in &update.timeline.events {
            let raw = value(room, event)?;
            let target = raw["content"]["m.relates_to"]["event_id"].as_str();
            let redacts = raw["content"]["redacts"]
                .as_str()
                .or(raw["redacts"].as_str());
            let entry = timeline::decode(raw.clone(), rules.keep_room_redaction_redacts);
            if entry.id == root
                || target == Some(root.as_str())
                || self.entries.iter().any(|e| {
                    e.id == entry.id
                        || Some(e.id.as_str()) == target
                        || Some(e.id.as_str()) == redacts
                })
            {
                self.entries.retain(|e| e.id != entry.id);
                self.entries.push(entry);
                changed = true;
            }
        }
        let receipts_changed = update
            .ephemeral
            .iter()
            .any(|e| e.get_field::<String>("type").ok().flatten().as_deref() == Some("m.receipt"));
        if !changed && !receipts_changed {
            return Ok(None);
        }
        if self.entries.len() > 10_001 {
            return Err("tool-limit");
        }
        let messages = timeline::aggregate(&self.entries);
        latest.message = messages
            .iter()
            .find(|m| m.id == root)
            .cloned()
            .ok_or("message-unavailable")?;
        latest.thread = messages
            .into_iter()
            .filter(|m| m.thread_root.as_deref() == Some(&root))
            .collect();
        latest.revisions = timeline::revisions(&self.entries, &root);
        latest.loaded = self.entries.len().saturating_sub(1);
        let id = EventId::parse(&root).map_err(|_| "message-unavailable")?;
        latest.read_by = room
            .load_event_receipts(ReceiptType::Read, &ReceiptThread::Unthreaded, &id)
            .await
            .map_err(failure)?
            .into_iter()
            .map(|(u, _)| u.to_string())
            .collect();
        latest.thread_read_by = thread_readers(room, &latest.message, &id).await?;
        crate::receipts::populate_details(room, &mut latest).await;
        self.latest = Some(latest.clone());
        Ok(Some(latest))
    }
    pub async fn load(
        &mut self,
        room: &Room,
        event_id: &str,
        more: bool,
    ) -> Result<MessageDetails, &'static str> {
        let id = EventId::parse(event_id).map_err(|_| "message-unavailable")?;
        let key = (room.room_id().to_string(), event_id.to_owned());
        if more && self.key.as_ref() != Some(&key) {
            return Err("search-restart");
        }
        let mut staged = if more {
            self.clone()
        } else {
            Self {
                key: Some(key),
                ..Self::default()
            }
        };
        let rules = room.clone_info().room_version_rules_or_default().redaction;
        // Always refresh the original to detect redaction and current bundled replacement.
        let original = room.event(&id, None).await.map_err(failure)?;
        let raw = value(room, &original)?;
        if raw["event_id"].as_str() != Some(event_id) {
            return Err("message-unavailable");
        }
        let original = timeline::decode(raw, rules.keep_room_redaction_redacts);
        if original.message.is_none() {
            return Err("message-unavailable");
        }
        staged.entries.retain(|entry| entry.id != event_id);
        staged.entries.insert(0, original);
        if !more || staged.cursor.more() {
            let options = RelationsOptions {
                from: staged.cursor.next.clone(),
                limit: Some(uint!(100)),
                ..Default::default()
            };
            let response = room.relations(id.clone(), options).await.map_err(failure)?;
            if response.chunk.len() > 100 {
                return Err("history-invalid-page");
            }
            for event in response.chunk {
                let entry =
                    timeline::decode(value(room, &event)?, rules.keep_room_redaction_redacts);
                if entry.id == event_id {
                    continue;
                }
                staged.entries.retain(|old| old.id != entry.id);
                staged.entries.push(entry);
            }
            if staged.entries.len() > 10_001 {
                return Err("tool-limit");
            }
            staged.cursor.advance(response.next_batch_token)?;
        }
        let message = timeline::aggregate(&staged.entries)
            .into_iter()
            .find(|m| m.id == event_id)
            .ok_or("message-unavailable")?;
        let mut reply = None;
        let mut reply_unavailable = false;
        if let Some(target) = &message.reply_to {
            match fetch_message(room, target).await {
                Ok(m) => reply = Some(m),
                Err("session-expired") => return Err("session-expired"),
                Err(_) => reply_unavailable = true,
            }
        }
        let read_by = room
            .load_event_receipts(ReceiptType::Read, &ReceiptThread::Unthreaded, &id)
            .await
            .map_err(failure)?
            .into_iter()
            .map(|(user, _)| user.to_string())
            .collect();
        let mut result = MessageDetails {
            thread: timeline::aggregate(&staged.entries)
                .into_iter()
                .filter(|m| m.thread_root.as_deref() == Some(event_id))
                .collect(),
            unavailable: staged
                .entries
                .iter()
                .filter(|e| {
                    e.message
                        .as_ref()
                        .is_some_and(|m| !m.deleted && m.body.is_none())
                })
                .count(),
            message: message.clone(),
            reply,
            reply_unavailable,
            revisions: timeline::revisions(&staged.entries, event_id),
            thread_read_by: thread_readers(room, &message, &id).await?,
            read_by,
            more: staged.cursor.more(),
            loaded: staged.entries.len() - 1,
        };
        crate::receipts::populate_details(room, &mut result).await;
        staged.latest = Some(result.clone());
        *self = staged;
        Ok(result)
    }
}
async fn fetch_message(room: &Room, id: &str) -> Result<crate::Message, &'static str> {
    let id = EventId::parse(id).map_err(|_| "message-unavailable")?;
    let event = room.event(&id, None).await.map_err(failure)?;
    let raw = value(room, &event)?;
    if raw["event_id"].as_str() != Some(id.as_str()) {
        return Err("message-unavailable");
    }
    let rules = room.clone_info().room_version_rules_or_default().redaction;
    timeline::aggregate(&[timeline::decode(raw, rules.keep_room_redaction_redacts)])
        .into_iter()
        .next()
        .ok_or("message-unavailable")
}
pub(crate) fn value(room: &Room, event: &TimelineEvent) -> Result<Value, &'static str> {
    event
        .raw()
        .deserialize()
        .map_err(|_| "history-invalid-page")?;
    let value: Value =
        serde_json::from_str(event.raw().json().get()).map_err(|_| "history-invalid-page")?;
    if value["room_id"]
        .as_str()
        .is_some_and(|id| id != room.room_id().as_str())
    {
        return Err("history-invalid-page");
    }
    Ok(value)
}

async fn thread_readers(
    room: &Room,
    message: &crate::Message,
    id: &matrix_sdk::ruma::EventId,
) -> Result<Vec<String>, &'static str> {
    let root = EventId::parse(message.thread_root.as_deref().unwrap_or(&message.id))
        .map_err(|_| "message-unavailable")?;
    let mut users = room
        .load_event_receipts(ReceiptType::Read, &ReceiptThread::Thread(root), id)
        .await
        .map_err(failure)?
        .into_iter()
        .map(|(user, _)| user.to_string())
        .collect::<Vec<_>>();
    users.sort();
    Ok(users)
}
