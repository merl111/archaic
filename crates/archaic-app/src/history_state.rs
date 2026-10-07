use crate::model::{Model, Phase};
use archaic_matrix::{EventKind, HistoryLoad, Membership};
pub struct CachedHistory {
    pub room: String,
    pub messages: Vec<archaic_matrix::Message>,
    pub info: archaic_matrix::HistoryInfo,
}
impl Model {
    pub fn switch_history(&mut self, room: String) -> bool {
        if let Some(previous) = self.selected.clone()
            && (!self.messages.is_empty() || self.status == "ready")
            && self
                .selected_room()
                .is_some_and(|r| r.membership == Membership::Joined)
        {
            self.history_cache.retain(|h| h.room != previous);
            self.history_cache.push_back(CachedHistory {
                room: previous,
                messages: std::mem::take(&mut self.messages),
                info: self.history_info,
            });
        }
        self.clear_history();
        self.selected = Some(room.clone());
        let cached = self
            .history_cache
            .iter()
            .position(|h| h.room == room)
            .and_then(|i| self.history_cache.remove(i));
        let restored = cached.is_some();
        if let Some(cached) = cached {
            self.messages = cached.messages;
            self.messages.retain(|m| !self.blocked.contains(&m.sender));
            self.history_info = cached.info;
        }
        while self.history_cache.len() > 32 {
            self.history_cache.pop_front();
        }
        restored
    }

    pub fn history_command(&self, kind: HistoryLoad) -> Option<archaic_matrix::Command> {
        if self.tools_busy()
            || self.phase != Phase::Connected
            || self.history_pending.is_some()
            || self.room_pending.is_some()
            || self.room_confirmation.is_some()
            || self.joining
            || self.creating
            || (kind == HistoryLoad::Newer && !self.history_info.can_load_newer)
            || (kind == HistoryLoad::Older && !self.history_info.can_load_more)
        {
            return None;
        }
        let room = self.selected_room()?;
        (room.membership == Membership::Joined).then(|| archaic_matrix::Command::LoadHistory {
            room_id: room.id.clone(),
            kind,
        })
    }
    pub fn clear_history(&mut self) {
        self.image_open = None;
        self.tools = Default::default();
        self.messages.clear();
        self.message_selected = None;
        self.message_draft = None;
        self.history_info = Default::default();
        self.history_pending = None;
        self.history_jump = None;
        self.history_error = None;
    }
    pub fn apply_history(&mut self, event: EventKind) {
        let (room_id, kind) = match &event {
            EventKind::History { room_id, kind, .. }
            | EventKind::HistoryFailed { room_id, kind, .. } => (room_id, *kind),
            _ => unreachable!("history events only"),
        };
        if self.selected.as_ref() != Some(room_id) {
            if let EventKind::History {
                room_id,
                messages,
                info,
                ..
            } = event
                && let Some(cached) = self.history_cache.iter_mut().find(|h| h.room == room_id)
            {
                cached.messages = messages;
                cached.info = info;
            }
            return;
        }
        if !self
            .selected_room()
            .is_some_and(|room| room.membership == Membership::Joined)
        {
            return;
        }
        if self.history_pending == Some(kind) {
            self.history_pending = None;
        }
        match event {
            EventKind::History { messages, info, .. } => {
                if self.activity.opened.as_ref() == self.selected.as_ref()
                    && matches!(kind, HistoryLoad::Latest | HistoryLoad::Recent)
                    && let Some(room) = &self.selected
                {
                    self.activity.unread.insert(room.clone(), 0);
                    self.activity
                        .viewed
                        .insert(room.clone(), messages.last().map(|m| m.id.clone()));
                }
                self.activity.opened = None;
                self.messages = messages;
                self.history_info = info;
                if kind != HistoryLoad::Recent
                    || self.history_error.is_some_and(|(failed, _)| failed == kind)
                {
                    self.history_error = None;
                }
                self.status = "ready";
            }
            EventKind::HistoryFailed { key, .. } => {
                self.history_jump = None;
                if kind != HistoryLoad::Recent || self.history_error.is_none() {
                    self.history_error = Some((kind, key));
                }
                if self.status == "loading" {
                    self.status = "";
                }
            }
            _ => unreachable!("history events only"),
        }
    }
}
