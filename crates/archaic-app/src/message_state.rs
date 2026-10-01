use crate::model::{Model, Phase};
use archaic_matrix::{Command, Membership, Message, MessageAction, MessageRequest, transaction_id};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    Thread,
    Forward,
    Reply,
    Edit,
    Delete,
    Reaction,
}
impl ActionKind {
    pub fn inline(self) -> bool {
        matches!(self, Self::Reply | Self::Edit)
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Thread => "message-thread",
            Self::Forward => "message-forward",
            Self::Reply => "message-reply",
            Self::Edit => "message-edit",
            Self::Delete => "message-delete",
            Self::Reaction => "message-react",
        }
    }
}
pub struct ActionDraft {
    pub kind: ActionKind,
    pub target: String,
    pub body: String,
    pub pending: bool,
    pub request: Option<MessageRequest>,
    pub error: Option<&'static str>,
}
impl ActionDraft {
    fn reaction_action(&self, target: &Message, user: &str) -> MessageAction {
        // Retain the exact removal set on unchanged retries, including after partial success.
        if let Some(previous) = &self.request
            && matches!(&previous.action, MessageAction::RemoveReactions { key, .. } | MessageAction::React(key) if key == &self.body)
        {
            previous.action.clone()
        } else {
            let ids: Vec<_> = target
                .reactions
                .iter()
                .filter(|r| r.sender == user && r.key == self.body)
                .map(|r| r.id.clone())
                .collect();
            if ids.is_empty() {
                MessageAction::React(self.body.clone())
            } else {
                MessageAction::RemoveReactions {
                    key: self.body.clone(),
                    ids,
                }
            }
        }
    }
}
impl Model {
    pub fn selected_message(&self) -> Option<&Message> {
        self.messages
            .iter()
            .find(|m| self.message_selected.as_ref() == Some(&m.id))
            .or_else(|| {
                self.tools.details.as_ref().and_then(|d| {
                    std::iter::once(&d.message)
                        .chain(d.thread.iter())
                        .find(|m| self.message_selected.as_ref() == Some(&m.id))
                })
            })
    }

    pub fn can_message_action(&self) -> bool {
        !self.tools_busy()
            && self.phase == Phase::Connected
            && !self.sending
            && !self.joining
            && !self.creating
            && self.room_pending.is_none()
            && self.room_confirmation.is_none()
            && self
                .selected_room()
                .is_some_and(|r| r.membership == Membership::Joined)
    }
    pub fn begin_message_action(&mut self, kind: ActionKind) {
        if !self.can_message_action() || self.message_draft.is_some() {
            return;
        }
        let Some(message) = self
            .selected_message()
            .filter(|m| !m.deleted && m.body.is_some())
        else {
            return;
        };
        if matches!(kind, ActionKind::Edit | ActionKind::Delete)
            && !(message.editable && message.sender == self.user)
        {
            return;
        }
        self.message_draft = Some(ActionDraft {
            kind,
            target: message.id.clone(),
            body: if kind == ActionKind::Edit {
                message.body.clone().unwrap_or_default()
            } else {
                String::new()
            },
            pending: false,
            request: None,
            error: None,
        });
    }
    pub fn message_command(&mut self) -> Option<Command> {
        if !self.can_message_action() {
            return None;
        }
        let draft = self.message_draft.as_ref()?;
        if draft.pending {
            return None;
        }
        if draft.kind == ActionKind::Delete
            && let Some(request) = &draft.request
        {
            return Some(Command::QueueMessageAction(request.clone()));
        }
        let target = self
            .selected_message()
            .filter(|m| m.id == draft.target && !m.deleted && m.body.is_some());
        let Some(target) = target else {
            self.message_draft.as_mut()?.error = Some("message-unavailable");
            return None;
        };
        if matches!(draft.kind, ActionKind::Edit | ActionKind::Delete)
            && !(target.editable && target.sender == self.user)
        {
            return None;
        }
        let action = match draft.kind {
            ActionKind::Thread => MessageAction::Thread(draft.body.clone()),
            ActionKind::Forward => MessageAction::Forward(draft.body.clone()),
            ActionKind::Reply => MessageAction::Reply(draft.body.clone()),
            ActionKind::Edit => MessageAction::Edit(draft.body.clone()),
            ActionKind::Delete => MessageAction::Delete,
            ActionKind::Reaction => draft.reaction_action(target, &self.user),
        };
        let room_id = self.selected.clone()?;
        let draft = self.message_draft.as_mut()?;
        let request = match &draft.request {
            Some(previous) if previous.action == action => previous.clone(),
            _ => MessageRequest {
                room_id,
                event_id: draft.target.clone(),
                action,
                transaction: transaction_id(),
            },
        };
        draft.request = Some(request.clone());
        Some(Command::QueueMessageAction(request))
    }
    pub fn message_result(&mut self, request: MessageRequest, result: Result<(), &'static str>) {
        let Some(draft) = &mut self.message_draft else {
            return;
        };
        if !draft.pending || draft.request.as_ref() != Some(&request) {
            return;
        }
        if let Err(key) = result {
            draft.pending = false;
            draft.error = Some(key);
        } else {
            // Keep the active relation snapshot so subsequent LiveDetails can
            // refresh it, including reactions/edits inside the thread pane.
            self.message_draft = None;
            self.status = "message-action-done";
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use archaic_matrix::{Event, EventKind, Reaction, RoomSummary};
    fn model() -> Model {
        Model {
            epoch: 1,
            phase: Phase::Connected,
            user: "@a:local".into(),
            selected: Some("!r:local".into()),
            rooms: vec![RoomSummary {
                info: Default::default(),
                id: "!r:local".into(),
                name: "Room".into(),
                membership: Membership::Joined,
                inviter: None,
            }],
            messages: vec![Message {
                id: "$m".into(),
                sender: "@a:local".into(),
                body: Some("original".into()),
                editable: true,
                ..Message::default()
            }],
            message_selected: Some("$m".into()),
            ..Model::default()
        }
    }
    fn request(model: &mut Model) -> MessageRequest {
        let Command::QueueMessageAction(request) = model.message_command().unwrap() else {
            panic!("message action");
        };
        request
    }
    #[test]
    fn reacting_to_thread_reply_keeps_live_relation_updates_attached() {
        let mut m = model();
        let mut details = crate::visual_fixture::model().tools.details.unwrap();
        m.message_selected = Some(details.thread[0].id.clone());
        m.tools.details = Some(details.clone());
        m.begin_message_action(ActionKind::Reaction);
        m.message_draft.as_mut().unwrap().body = "❤️".into();
        let Some(Command::QueueMessageAction(request)) = m.message_command() else {
            panic!("reaction command");
        };
        assert_eq!(request.event_id, details.thread[0].id);
        m.message_draft.as_mut().unwrap().pending = true;
        m.message_result(request, Ok(()));
        assert!(m.message_draft.is_none());
        assert!(m.tools.details.is_some());
        details.thread[0].body = Some("Updated thread message".into());
        m.apply(Event {
            epoch: m.epoch,
            kind: EventKind::LiveDetails {
                room_id: m.selected.clone().unwrap(),
                details: Box::new(details),
            },
        });
        assert_eq!(
            m.tools.details.unwrap().thread[0].body.as_deref(),
            Some("Updated thread message")
        );
    }

    #[test]
    fn retry_keeps_transaction_but_changing_text_creates_a_new_one_and_preserves_composer() {
        let mut m = model();
        m.drafts.insert("!r:local".into(), "ordinary draft".into());
        m.begin_message_action(ActionKind::Edit);
        assert_eq!(m.message_draft.as_ref().unwrap().body, "original");
        let first = request(&mut m);
        m.message_draft.as_mut().unwrap().pending = true;
        assert!(m.message_command().is_none());
        m.message_result(first.clone(), Err("message-action-failed"));
        assert_eq!(request(&mut m), first);
        m.message_draft.as_mut().unwrap().body = "changed".into();
        let changed = request(&mut m);
        assert_ne!(changed.transaction, first.transaction);
        m.message_draft.as_mut().unwrap().pending = true;
        m.message_result(first, Ok(()));
        assert!(m.message_draft.is_some());
        m.message_result(changed, Ok(()));
        assert!(m.message_draft.is_none());
        assert_eq!(m.draft(), "ordinary draft");
    }
    #[test]
    fn foreign_or_deleted_messages_block_mutations_and_membership_loss_clears_action() {
        let mut m = model();
        m.messages[0].sender = "@b:local".into();
        for kind in [ActionKind::Edit, ActionKind::Delete] {
            m.begin_message_action(kind);
            assert!(m.message_draft.is_none());
        }
        m.begin_message_action(ActionKind::Reply);
        assert!(m.message_draft.is_some());
        m.messages[0].deleted = true;
        assert!(m.message_command().is_none());
        m.update_rooms(vec![]);
        assert!(m.message_draft.is_none());
    }
    #[test]
    fn current_timeline_reactions_win_over_an_older_thread_snapshot() {
        let mut m = model();
        m.tools.details = Some(archaic_matrix::MessageDetails {
            message: m.messages[0].clone(),
            thread: vec![],
            unavailable: 0,
            reply: None,
            reply_unavailable: false,
            revisions: vec![],
            read_by: vec![],
            thread_read_by: vec![],
            more: false,
            loaded: 0,
        });
        m.messages[0].reactions.push(Reaction {
            id: "$new-reaction".into(),
            sender: m.user.clone(),
            key: "❤️".into(),
        });
        m.begin_message_action(ActionKind::Reaction);
        m.message_draft.as_mut().unwrap().body = "❤️".into();
        assert!(
            matches!(request(&mut m).action, MessageAction::RemoveReactions { ids, .. } if ids == ["$new-reaction"])
        );
    }
    #[test]
    fn reaction_removal_retains_full_set_on_partial_retry_and_old_epochs_do_not_complete_it() {
        let mut m = model();
        m.messages[0].reactions = vec![
            Reaction {
                id: "$r1".into(),
                sender: m.user.clone(),
                key: "👍".into(),
            },
            Reaction {
                id: "$r2".into(),
                sender: m.user.clone(),
                key: "👍".into(),
            },
        ];
        m.begin_message_action(ActionKind::Reaction);
        m.message_draft.as_mut().unwrap().body = "👍".into();
        let first = request(&mut m);
        assert!(matches!(&first.action,MessageAction::RemoveReactions{ids,..} if ids.len()==2));
        m.message_draft.as_mut().unwrap().pending = true;
        m.apply(Event {
            epoch: 0,
            kind: EventKind::MessageActionResult {
                request: first.clone(),
                result: Ok(()),
            },
        });
        assert!(m.message_draft.as_ref().unwrap().pending);
        m.message_result(first.clone(), Err("message-action-failed"));
        m.messages[0].reactions.clear();
        assert_eq!(request(&mut m), first);
    }
    #[test]
    fn acknowledgements_lost_after_reaction_or_delete_do_not_change_the_retry() {
        let mut m = model();
        m.begin_message_action(ActionKind::Reaction);
        m.message_draft.as_mut().unwrap().body = "like".into();
        let first = request(&mut m);
        m.messages[0].reactions.push(Reaction {
            id: "$r".into(),
            sender: m.user.clone(),
            key: "like".into(),
        });
        assert_eq!(request(&mut m), first);
        m.message_draft = None;
        m.begin_message_action(ActionKind::Delete);
        let delete = request(&mut m);
        m.messages[0].deleted = true;
        m.messages[0].body = None;
        assert_eq!(request(&mut m), delete);
    }
    #[test]
    fn delete_requires_an_explicit_confirm_and_blocks_conflicting_operations() {
        let mut m = model();
        m.begin_message_action(ActionKind::Delete);
        assert!(!m.message_draft.as_ref().unwrap().pending);
        assert!(!m.can_create());
        assert!(!m.room_action_allowed(&archaic_matrix::RoomAction::Leave));
        assert!(m.send_command().is_none());
        assert_eq!(request(&mut m).action, MessageAction::Delete);
    }
}
