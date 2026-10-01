use crate::model::{Model, Phase};
use archaic_matrix::{EventKind, HistoryLoad, Membership};
impl Model {
    pub fn history_command(&self, kind: HistoryLoad) -> Option<archaic_matrix::Command> {
        if self.tools_busy()
            || self.phase != Phase::Connected
            || self.history_pending.is_some()
            || self.room_pending.is_some()
            || self.room_confirmation.is_some()
            || self.joining
            || self.creating
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
        self.tools = Default::default();
        self.messages.clear();
        self.message_selected = None;
        self.message_draft = None;
        self.history_info = Default::default();
        self.history_pending = None;
        self.history_error = None;
    }
    pub fn apply_history(&mut self, event: EventKind) {
        let (room_id, kind) = match &event {
            EventKind::History { room_id, kind, .. }
            | EventKind::HistoryFailed { room_id, kind, .. } => (room_id, *kind),
            _ => unreachable!("history events only"),
        };
        if self.selected.as_ref() != Some(room_id)
            || !self
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
