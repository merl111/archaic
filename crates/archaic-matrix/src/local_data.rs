//! Account/device-scoped encrypted application state, separate from SDK crypto state.
use crate::{storage::Profile, token_store::TokenStore};
use std::{collections::HashMap, sync::Arc};
#[derive(Default)]
pub(crate) struct LocalData {
    storage: Option<(Arc<Profile>, String)>,
    pub drafts: HashMap<String, String>,
    pub index_dirty: bool,
    pub index: crate::search_index::Index,
}
impl LocalData {
    pub fn open(tokens: Option<&TokenStore>) -> Result<Self, &'static str> {
        let storage = tokens.map(TokenStore::local_storage).transpose()?;
        let drafts = match &storage {
            Some((profile, id)) => profile.read_data(id, "drafts")?,
            None => HashMap::new(),
        };
        let index = match &storage {
            Some((p, id)) => p.read_data(id, "search-index")?,
            None => Default::default(),
        };
        Ok(Self {
            storage,
            index_dirty: false,
            drafts,
            index,
        })
    }
    pub fn save_index(&mut self) -> Result<(), &'static str> {
        if let Some((p, id)) = &self.storage {
            p.write_data(id, "search-index", &self.index)?;
        }
        self.index_dirty = false;
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
