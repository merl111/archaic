use crate::model::{Model, Phase};
use archaic_matrix::{Membership, RoomAction, RoomSummary};

impl Model {
    pub fn selected_room(&self) -> Option<&RoomSummary> {
        self.selected
            .as_ref()
            .and_then(|id| self.rooms.iter().find(|room| &room.id == id))
    }
    pub fn room_action_allowed(&self, action: &RoomAction) -> bool {
        if self.tools_busy()
            || self.message_draft.is_some()
            || self.phase != Phase::Connected
            || self.room_pending.is_some()
            || self.joining
            || self.creating
            || self.sending
        {
            return false;
        }
        self.selected_room().is_some_and(|room| match action {
            RoomAction::Accept | RoomAction::Decline => room.membership == Membership::Invited,
            RoomAction::Leave | RoomAction::Invite(_) => room.membership == Membership::Joined,
        })
    }
    pub fn update_rooms(&mut self, rooms: Vec<RoomSummary>) {
        let previous = self.selected_room().map(|room| room.membership);
        self.rooms = rooms;
        self.history_cache.retain(|h| {
            self.rooms
                .iter()
                .any(|r| r.id == h.room && r.membership == Membership::Joined)
        });
        if self
            .active_space
            .as_ref()
            .is_some_and(|id| !self.rooms.iter().any(|r| &r.id == id && r.info.space))
        {
            self.active_space = None;
        }
        let current = self.selected_room().map(|room| room.membership);
        if previous != current {
            self.clear_history();
            self.room_confirmation = None;
        }
        if self.selected_room().is_none() {
            self.selected = None;
        }
        // Membership loss must not leave sendable drafts or old transaction state behind.
        self.drafts.retain(|id, _| {
            self.rooms
                .iter()
                .any(|room| &room.id == id && room.membership == Membership::Joined)
        });
        self.outgoing.retain(|id, _| {
            self.rooms
                .iter()
                .any(|room| &room.id == id && room.membership == Membership::Joined)
        });
        self.thread_sends.retain(|_, pending| {
            self.rooms
                .iter()
                .any(|r| r.id == pending.room && r.membership == Membership::Joined)
        });
        if self.outgoing.is_empty() {
            self.sending = false;
        }
    }
    pub fn room_action_result(
        &mut self,
        room_id: String,
        action: RoomAction,
        result: Result<(), &'static str>,
    ) {
        if self.room_pending.as_ref() != Some(&(room_id.clone(), action.clone())) {
            return;
        }
        self.room_pending = None;
        if let Err(key) = result {
            self.room_feedback = Some((room_id, key));
            return;
        }
        self.room_confirmation = None;
        self.room_feedback = None;
        match action {
            RoomAction::Accept => {
                if let Some(room) = self.rooms.iter_mut().find(|room| room.id == room_id) {
                    room.membership = Membership::Joined;
                    room.inviter = None;
                }
                self.status = "loading";
            }
            RoomAction::Decline | RoomAction::Leave => {
                self.rooms.retain(|room| room.id != room_id);
                self.history_cache.retain(|h| h.room != room_id);
                self.drafts.remove(&room_id);
                self.outgoing.remove(&room_id);
                if self.selected.as_ref() == Some(&room_id) {
                    self.selected = None;
                    self.clear_history();
                }
                self.status = "room-left";
            }
            RoomAction::Invite(_) => self.room_feedback = Some((room_id, "invite-sent")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use archaic_matrix::{Event, EventKind, Message};

    fn room(id: &str, membership: Membership) -> RoomSummary {
        RoomSummary {
            info: Default::default(),
            id: id.into(),
            name: id.into(),
            membership,
            inviter: None,
        }
    }
    fn model() -> Model {
        Model {
            epoch: 1,
            phase: Phase::Connected,
            selected: Some("!invite:local".into()),
            rooms: vec![
                room("!invite:local", Membership::Invited),
                room("!other:local", Membership::Joined),
            ],
            ..Model::default()
        }
    }
    #[test]
    fn invited_room_blocks_send_and_old_history_until_acceptance() {
        let mut model = model();
        model.drafts.insert("!invite:local".into(), "draft".into());
        assert!(model.send_command().is_none());
        model.apply(Event {
            epoch: 1,
            kind: EventKind::History {
                info: Default::default(),
                kind: archaic_matrix::HistoryLoad::Recent,
                room_id: "!invite:local".into(),
                messages: vec![Message {
                    id: "old".into(),
                    sender: "alice".into(),
                    body: Some("old history".into()),
                    ..Message::default()
                }],
            },
        });
        assert!(model.messages.is_empty());
        assert!(model.room_action_allowed(&RoomAction::Accept));
        model.room_pending = Some(("!invite:local".into(), RoomAction::Accept));
        assert!(!model.room_action_allowed(&RoomAction::Decline));
        model.room_action_result(
            "!invite:local".into(),
            RoomAction::Accept,
            Err("room-action-forbidden"),
        );
        assert!(model.room_pending.is_none());
        assert_eq!(
            model.selected_room().unwrap().membership,
            Membership::Invited
        );
        model.room_pending = Some(("!invite:local".into(), RoomAction::Accept));
        model.room_action_result("!invite:local".into(), RoomAction::Accept, Ok(()));
        assert_eq!(
            model.selected_room().unwrap().membership,
            Membership::Joined
        );
        assert!(model.send_command().is_some());
    }
    #[test]
    fn completion_is_scoped_and_membership_loss_clears_private_room_state() {
        let mut model = model();
        model.room_pending = Some(("!invite:local".into(), RoomAction::Decline));
        model.selected = Some("!other:local".into());
        model
            .drafts
            .insert("!other:local".into(), "keep this".into());
        let _ = model.send_command(); // Pending operations block sending.
        model.room_action_result("!unrelated:local".into(), RoomAction::Decline, Ok(()));
        assert!(model.room_pending.is_some());
        model.apply(Event {
            epoch: 0,
            kind: EventKind::RoomActionResult {
                room_id: "!invite:local".into(),
                action: RoomAction::Decline,
                result: Ok(()),
            },
        });
        assert!(model.room_pending.is_some());
        model.room_action_result("!invite:local".into(), RoomAction::Decline, Ok(()));
        assert_eq!(model.selected.as_deref(), Some("!other:local"));
        assert_eq!(model.drafts["!other:local"], "keep this");
        model.send_command().unwrap();
        model.sending = true;
        model.room_confirmation = Some(("!other:local".into(), RoomAction::Leave));
        assert!(model.send_command().is_none());
        model.update_rooms(vec![]);
        assert!(model.selected.is_none() && model.drafts.is_empty() && model.outgoing.is_empty());
        assert!(model.room_confirmation.is_none());
        assert!(!model.sending);
    }
    #[test]
    fn failed_leave_preserves_draft_and_confirmation_for_retry() {
        let mut model = model();
        model.selected = Some("!other:local".into());
        model
            .drafts
            .insert("!other:local".into(), "keep until confirmed".into());
        let action = ("!other:local".into(), RoomAction::Leave);
        model.room_confirmation = Some(action.clone());
        model.room_pending = Some(action.clone());
        model.room_action_result(
            action.0.clone(),
            action.1.clone(),
            Err("room-action-failed"),
        );
        assert_eq!(model.draft(), "keep until confirmed");
        assert_eq!(model.room_confirmation, Some(action.clone()));
        model.room_pending = Some(action.clone());
        model.room_action_result(action.0, action.1, Ok(()));
        assert!(
            model.selected.is_none()
                && model.drafts.is_empty()
                && model.room_confirmation.is_none()
        );
    }
}
