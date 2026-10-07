//! Account/device-scoped encrypted application state, separate from SDK crypto state.
use crate::{storage::Profile, token_store::TokenStore};
use std::{collections::HashMap, sync::Arc};
#[derive(Default)]
pub(crate) struct LocalData {
    storage: Option<(Arc<Profile>, String)>,
    pub drafts: HashMap<String, String>,
    pub recent: HashMap<String, u64>,
    pub previews: HashMap<String, crate::RoomPreview>,
    pub index: crate::archive::Archive,
}
impl LocalData {
    pub fn open(tokens: Option<&TokenStore>) -> Result<Self, &'static str> {
        let storage = tokens.map(TokenStore::local_storage).transpose()?;
        let drafts = match &storage {
            Some((profile, id)) => profile.read_data(id, "drafts")?,
            None => HashMap::new(),
        };
        let index = crate::archive::Archive::open(storage.as_ref())?;
        let recent = match &storage {
            Some((p, id)) => p.read_data(id, "room-recency")?,
            None => HashMap::new(),
        };
        let previews = match &storage {
            Some((p, id)) => p.read_data(id, "room-previews")?,
            None => HashMap::new(),
        };
        Ok(Self {
            previews,
            recent,
            storage,
            drafts,
            index,
        })
    }
    pub fn update_recent(
        &mut self,
        response: &matrix_sdk::sync::SyncResponse,
    ) -> Result<(), &'static str> {
        let mut changed = false;
        for (id, room) in &response.rooms.joined {
            for event in &room.timeline.events {
                let raw = event.raw();
                let kind = raw
                    .get_field::<String>("type")
                    .ok()
                    .flatten()
                    .unwrap_or_default();
                let value: serde_json::Value =
                    serde_json::from_str(raw.json().get()).unwrap_or_default();
                let content = &value["content"];
                let preview = self.previews.get_mut(id.as_str());
                let redacts = value["redacts"].as_str().or(content["redacts"].as_str());
                if kind == "m.room.redaction"
                    && preview
                        .as_ref()
                        .is_some_and(|p| redacts == Some(p.event.as_str()))
                {
                    self.previews.remove(id.as_str());
                    changed = true;
                    continue;
                }
                if content["m.relates_to"]["rel_type"] == "m.replace" {
                    if let Some(p) = preview
                        && content["m.relates_to"]["event_id"].as_str() == Some(p.event.as_str())
                        && value["sender"].as_str() == Some(p.sender.as_str())
                    {
                        p.body = content["m.new_content"]["body"]
                            .as_str()
                            .unwrap_or("")
                            .chars()
                            .take(180)
                            .collect();
                        changed = true;
                    }
                    continue;
                }
                if !matches!(
                    kind.as_str(),
                    "m.room.message"
                        | "m.room.encrypted"
                        | "m.sticker"
                        | "org.matrix.msc3381.poll.start"
                ) {
                    continue;
                }
                if let Some(ts) = raw.get_field::<u64>("origin_server_ts").ok().flatten() {
                    let recent = self.recent.entry(id.to_string()).or_default();
                    if ts >= *recent {
                        let body = content["body"]
                            .as_str()
                            .unwrap_or("")
                            .chars()
                            .take(180)
                            .collect();
                        self.previews.insert(
                            id.to_string(),
                            crate::RoomPreview {
                                event: value["event_id"].as_str().unwrap_or("").into(),
                                sender: value["sender"].as_str().unwrap_or("").into(),
                                body,
                                encrypted: kind == "m.room.encrypted",
                            },
                        );
                        *recent = ts;
                        changed = true;
                    }
                }
            }
        }
        if changed && let Some((p, id)) = &self.storage {
            p.write_data(id, "room-recency", &self.recent)?;
            p.write_data(id, "room-previews", &self.previews)?;
        }
        Ok(())
    }
    pub fn save_draft(&mut self, room: String, body: String) -> Result<(), &'static str> {
        self.save_drafts(HashMap::from([(room, body)]))
    }
    pub fn save_drafts(&mut self, drafts: HashMap<String, String>) -> Result<(), &'static str> {
        let mut staged = self.drafts.clone();
        for (room, body) in drafts {
            matrix_sdk::ruma::RoomId::parse(&room).map_err(|_| "draft-save-failed")?;
            if body.len() > 32_000 {
                return Err("draft-save-failed");
            }
            if body.is_empty() {
                staged.remove(&room);
            } else {
                staged.insert(room, body);
            }
        }
        if staged.len() > 2048 {
            return Err("draft-save-failed");
        }
        if let Some((profile, id)) = &self.storage {
            profile
                .write_data(id, "drafts", &staged)
                .map_err(|_| "draft-save-failed")?;
        }
        self.drafts = staged;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recency_tracks_messages_without_regressing_or_counting_state_updates() {
        use matrix_sdk::{
            deserialized_responses::TimelineEvent, ruma::serde::Raw, sync::SyncResponse,
        };
        let mut local = LocalData::default();
        let mut response = SyncResponse::default();
        let id = matrix_sdk::ruma::room_id!("!a:local").to_owned();
        for (kind, timestamp, expected) in [
            ("m.room.message", 10, 10),
            ("m.room.message", 5, 10),
            ("m.room.member", 99, 10),
            ("m.room.encrypted", 20, 20),
        ] {
            response
                .rooms
                .joined
                .entry(id.clone())
                .or_default()
                .timeline
                .events = vec![TimelineEvent::from_plaintext(
                Raw::from_json_string(
                    serde_json::json!({
                        "type": kind, "origin_server_ts": timestamp, "content": {}
                    })
                    .to_string(),
                )
                .unwrap(),
            )];
            local.update_recent(&response).unwrap();
            assert_eq!(local.recent[id.as_str()], expected);
        }
    }
    #[test]
    fn draft_batch_rejects_invalid_entry_without_partial_changes() {
        let mut local = LocalData::default();
        local
            .save_draft("!a:local".into(), "keep me".into())
            .unwrap();
        let drafts = HashMap::from([
            ("!a:local".into(), "replacement".into()),
            ("invalid".into(), "bad".into()),
        ]);
        assert_eq!(local.save_drafts(drafts), Err("draft-save-failed"));
        assert_eq!(local.drafts["!a:local"], "keep me");
        local.save_draft("!a:local".into(), String::new()).unwrap();
        assert!(local.drafts.is_empty());
    }
}
