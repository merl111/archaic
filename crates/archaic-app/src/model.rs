use archaic_matrix::{Command, Event, EventKind, Message, RoomSummary, transaction_id};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    SignedOut,
    Connecting,
    Restoring,
    RestoreBlocked,
    LogoutCleanup,
    Connected,
    SigningOut,
}

pub struct Outgoing {
    pub room_id: String,
    pub body: String,
    pub transaction: archaic_matrix::TransactionId,
}

#[derive(Default)]
pub struct Model {
    pub epoch: u64,
    pub temporary: bool,
    pub session_saved: bool,
    pub phase: Phase,
    pub user: String,
    pub rooms: Vec<RoomSummary>,
    pub filter: String,
    pub selected: Option<String>,
    pub messages: Vec<Message>,
    pub drafts: HashMap<String, String>,
    pub outgoing: HashMap<String, Outgoing>,
    pub sending: bool,
    pub show_join: bool,
    pub joining: bool,
    pub room_pending: Option<(String, archaic_matrix::RoomAction)>,
    pub room_confirmation: Option<(String, archaic_matrix::RoomAction)>,
    pub room_feedback: Option<(String, &'static str)>,
    pub join_error: Option<&'static str>,
    pub status: &'static str,
    pub error: Option<&'static str>,
}

impl Model {
    pub fn apply(&mut self, event: Event) {
        if event.epoch != self.epoch {
            return;
        }
        match event.kind {
            EventKind::SessionSaved(saved) => {
                self.session_saved = saved;
                if !saved && !self.temporary {
                    self.error = Some("session-not-saved");
                }
            }
            EventKind::RestoreBlocked(key) => {
                self.phase = Phase::RestoreBlocked;
                self.status = "";
                self.error = Some(key);
            }
            EventKind::Connected(user) => {
                self.user = user;
                self.phase = Phase::Connected;
                self.status = "syncing";
            }
            EventKind::Rooms(rooms) => self.update_rooms(rooms),
            EventKind::History { room_id, messages } => {
                if self.selected.as_ref() == Some(&room_id)
                    && self
                        .selected_room()
                        .is_some_and(|room| room.membership == archaic_matrix::Membership::Joined)
                {
                    self.messages = messages;
                    self.status = "ready";
                }
            }
            EventKind::Sent { transaction } => self.confirm_sent(&transaction),
            EventKind::Joined(room) => self.joined(room),
            EventKind::RoomActionResult {
                room_id,
                action,
                result,
            } => self.room_action_result(room_id, action, result),
            EventKind::JoinFailed(key) => {
                self.joining = false;
                self.show_join = true;
                self.join_error = Some(key);
            }
            EventKind::SignedOut => {
                let epoch = self.epoch;
                *self = Self {
                    epoch,
                    temporary: self.temporary,
                    ..Self::default()
                };
            }
            EventKind::Status(status) => self.status = status,
            EventKind::Error(key) => self.apply_error(key),
        }
    }

    fn joined(&mut self, room: RoomSummary) {
        self.selected = Some(room.id.clone());
        if let Some(existing) = self
            .rooms
            .iter_mut()
            .find(|existing| existing.id == room.id)
        {
            *existing = room;
        } else {
            self.rooms.push(room);
        }
        self.messages.clear();
        self.filter.clear();
        self.joining = false;
        self.show_join = false;
        self.join_error = None;
        self.status = "loading";
    }

    fn apply_error(&mut self, key: &'static str) {
        if self.phase == Phase::Connecting {
            self.phase = Phase::SignedOut;
            self.status = "";
        }
        match key {
            "send-failed" => self.sending = false,
            "logout-cleanup-failed" | "session-expired" => {
                self.phase = Phase::LogoutCleanup;
                self.status = "";
            }
            "logout-failed" => self.phase = Phase::Connected,
            _ => {}
        }
        self.error = Some(key);
    }

    fn confirm_sent(&mut self, transaction: &archaic_matrix::TransactionId) {
        let room = self.outgoing.iter().find_map(|(room, pending)| {
            (&pending.transaction == transaction).then(|| room.clone())
        });
        if let Some(pending) = room.and_then(|room| self.outgoing.remove(&room)) {
            if self.drafts.get(&pending.room_id) == Some(&pending.body) {
                self.drafts.remove(&pending.room_id);
            }
            self.sending = false;
            self.error = None;
        }
    }

    pub fn visible_rooms(&self) -> Vec<&RoomSummary> {
        let filter = self.filter.to_lowercase();
        self.rooms
            .iter()
            .filter(|room| {
                room.name.to_lowercase().contains(&filter)
                    || room.id.to_lowercase().contains(&filter)
            })
            .collect()
    }

    pub fn draft(&self) -> &str {
        self.selected
            .as_ref()
            .and_then(|id| self.drafts.get(id))
            .map(String::as_str)
            .unwrap_or("")
    }

    pub fn send_command(&mut self) -> Option<Command> {
        if self.sending || self.phase != Phase::Connected {
            return None;
        }
        let room = self.selected_room()?;
        if room.membership != archaic_matrix::Membership::Joined
            || self.room_pending.is_some()
            || self.room_confirmation.is_some()
        {
            return None;
        }
        let room_id = room.id.clone();
        let body = self.draft().to_string();
        if body.trim().is_empty() {
            return None;
        }
        let pending = self
            .outgoing
            .entry(room_id.clone())
            .or_insert_with(|| Outgoing {
                room_id: room_id.clone(),
                body: body.clone(),
                transaction: transaction_id(),
            });
        if pending.body != body {
            pending.body = body.clone();
            pending.transaction = transaction_id();
        }
        Some(Command::Send {
            room_id,
            body,
            transaction: pending.transaction.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn connected() -> Model {
        Model {
            epoch: 2,
            phase: Phase::Connected,
            rooms: ["!a:local", "!b:local"]
                .into_iter()
                .map(|id| RoomSummary {
                    id: id.into(),
                    name: id.into(),
                    membership: archaic_matrix::Membership::Joined,
                    inviter: None,
                })
                .collect(),
            selected: Some("!a:local".into()),
            drafts: HashMap::from([("!a:local".into(), "hello".into())]),
            ..Model::default()
        }
    }

    #[test]
    fn join_failure_preserves_conversation_and_success_selects_without_losing_drafts() {
        let mut model = connected();
        model.show_join = true;
        model.joining = true;
        model.apply(Event {
            epoch: 2,
            kind: EventKind::JoinFailed("join-forbidden"),
        });
        assert!(!model.joining && model.show_join);
        assert_eq!(model.selected.as_deref(), Some("!a:local"));
        assert_eq!(model.join_error, Some("join-forbidden"));
        model.filter = "old room filter".into();
        model.apply(Event {
            epoch: 2,
            kind: EventKind::Joined(RoomSummary {
                membership: archaic_matrix::Membership::Joined,
                inviter: None,
                id: "!b:local".into(),
                name: "New room".into(),
            }),
        });
        assert_eq!(model.selected.as_deref(), Some("!b:local"));
        assert!(model.filter.is_empty() && !model.show_join && model.join_error.is_none());
        assert_eq!(model.drafts["!a:local"], "hello");
        assert!(
            model
                .visible_rooms()
                .iter()
                .any(|room| room.id == "!b:local")
        );
        model.apply(Event {
            epoch: 2,
            kind: EventKind::SignedOut,
        });
        assert!(!model.show_join && !model.joining && model.join_error.is_none());
    }
    #[test]
    fn stale_sessions_and_other_room_history_cannot_replace_current_view() {
        let mut model = connected();
        model.apply(Event {
            epoch: 1,
            kind: EventKind::SignedOut,
        });
        assert!(model.phase == Phase::Connected);
        model.apply(Event {
            epoch: 2,
            kind: EventKind::History {
                room_id: "!b:local".into(),
                messages: vec![Message {
                    id: "old".into(),
                    sender: "alice".into(),
                    body: None,
                }],
            },
        });
        assert!(model.messages.is_empty());
    }
    #[test]
    fn retry_uses_same_transaction_and_ack_does_not_erase_newer_draft() {
        let mut model = connected();
        let Command::Send {
            transaction: first, ..
        } = model.send_command().unwrap()
        else {
            panic!()
        };
        model.sending = true;
        assert!(model.send_command().is_none());
        model.apply(Event {
            epoch: 2,
            kind: EventKind::Error("send-failed"),
        });
        let Command::Send {
            transaction: retry, ..
        } = model.send_command().unwrap()
        else {
            panic!()
        };
        assert_eq!(first, retry);
        model
            .drafts
            .insert("!a:local".into(), "a newer draft".into());
        model.selected = Some("!b:local".into());
        model.apply(Event {
            epoch: 2,
            kind: EventKind::Sent { transaction: retry },
        });
        assert_eq!(model.drafts["!a:local"], "a newer draft");
        assert!(!model.sending);
    }
    #[test]
    fn failed_transactions_survive_sending_in_another_room() {
        let mut model = connected();
        let Command::Send {
            transaction: first, ..
        } = model.send_command().unwrap()
        else {
            panic!()
        };
        model.apply(Event {
            epoch: 2,
            kind: EventKind::Error("send-failed"),
        });
        model.selected = Some("!b:local".into());
        model.drafts.insert("!b:local".into(), "other room".into());
        let Command::Send {
            transaction: second,
            ..
        } = model.send_command().unwrap()
        else {
            panic!()
        };
        model.apply(Event {
            epoch: 2,
            kind: EventKind::Sent {
                transaction: second,
            },
        });
        model.selected = Some("!a:local".into());
        let Command::Send {
            transaction: retry, ..
        } = model.send_command().unwrap()
        else {
            panic!()
        };
        assert_eq!(first, retry);
    }

    #[test]
    fn sign_out_removes_private_view_state() {
        let mut model = connected();
        model.apply(Event {
            epoch: 2,
            kind: EventKind::SignedOut,
        });
        assert!(model.drafts.is_empty() && model.rooms.is_empty() && model.selected.is_none());
    }
    #[test]
    fn restoration_failure_blocks_login_and_cleanup_failure_blocks_messaging() {
        let mut model = Model {
            phase: Phase::Restoring,
            ..Model::default()
        };
        model.apply(Event {
            epoch: 0,
            kind: EventKind::RestoreBlocked("vault-unavailable"),
        });
        assert!(model.phase == Phase::RestoreBlocked);
        assert_eq!(model.error, Some("vault-unavailable"));
        model.temporary = true;
        model.apply(Event {
            epoch: 0,
            kind: EventKind::SignedOut,
        });
        assert!(model.phase == Phase::SignedOut && model.temporary);
        model = connected();
        model.apply(Event {
            epoch: 2,
            kind: EventKind::Error("logout-cleanup-failed"),
        });
        assert!(model.phase == Phase::LogoutCleanup);
        assert!(model.send_command().is_none());
    }
}
