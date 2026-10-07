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
    pub images: std::collections::VecDeque<crate::image_viewer::CachedImage>,
    pub image_open: Option<(String, String)>,
    pub history_sync: Option<archaic_matrix::HistorySyncStatus>,
    pub profile: Option<archaic_matrix::MemberProfile>,
    pub active_space: Option<String>,
    pub direct_only: bool,
    pub avatars: HashMap<String, cui::Icon>,
    pub avatar_revision: u64,
    pub display_names: HashMap<String, String>,
    pub member_counts: HashMap<String, u64>,
    pub link_join: Option<(String, Vec<String>, Option<String>)>,
    pub blocked: std::collections::HashSet<String>,
    pub organization: crate::organization::State,
    pub activity: crate::activity::State,
    pub browser_login: bool,
    pub reauthentication: bool,
    pub outbox: Vec<archaic_matrix::outbox::Item>,
    pub thread_sends: HashMap<String, crate::thread_pane::PendingSend>,
    pub transfer_progress: HashMap<String, (u64, u64)>,
    pub security: crate::security::State,
    pub tools: crate::tool_state::ToolState,
    pub epoch: u64,
    pub temporary: bool,
    pub session_saved: bool,
    pub phase: Phase,
    pub user: String,
    pub rooms: Vec<RoomSummary>,
    pub filter: String,
    pub selected: Option<String>,
    pub messages: Vec<Message>,
    pub history_cache: std::collections::VecDeque<crate::history_state::CachedHistory>,
    pub message_selected: Option<String>,
    pub message_draft: Option<crate::message_state::ActionDraft>,
    pub history_info: archaic_matrix::HistoryInfo,
    pub history_jump: Option<String>,
    pub history_pending: Option<archaic_matrix::HistoryLoad>,
    pub history_error: Option<(archaic_matrix::HistoryLoad, &'static str)>,
    pub drafts: HashMap<String, String>,
    pub saved_drafts: HashMap<String, String>,
    pub outgoing: HashMap<String, Outgoing>,
    pub sending: bool,
    pub show_new: bool,
    pub creating: bool,
    pub create_error: Option<&'static str>,
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
    pub fn account_name(&self) -> &str {
        self.display_names
            .get("account")
            .filter(|n| !n.trim().is_empty())
            .map(String::as_str)
            .unwrap_or(&self.user)
    }
    pub fn user_name<'a>(&'a self, user: &'a str) -> &'a str {
        self.selected
            .as_ref()
            .and_then(|r| self.display_names.get(&format!("user:{r}:{user}")))
            .filter(|n| !n.trim().is_empty())
            .map(String::as_str)
            .or_else(|| {
                self.profile
                    .as_ref()
                    .filter(|p| p.loaded && p.user == user && !p.name.trim().is_empty())
                    .map(|p| p.name.as_str())
            })
            .unwrap_or_else(|| {
                if user == self.user {
                    self.account_name()
                } else {
                    user
                }
            })
    }

    pub fn apply(&mut self, event: Event) {
        if event.epoch != self.epoch {
            return;
        }
        match event.kind {
            EventKind::DisplayName { key, name } => {
                let name = crate::daylight::text(&name, 1024);
                if self.display_names.get(&key) != Some(&name) {
                    if self.display_names.len() >= 4096 && !self.display_names.contains_key(&key) {
                        self.display_names.retain(|k, _| k == "account");
                    }
                    self.display_names.insert(key, name);
                    self.avatar_revision = self.avatar_revision.wrapping_add(1);
                }
            }
            EventKind::HistorySync(status) => self.history_sync = Some(status),
            EventKind::MemberProfile(profile) => {
                if self.selected.as_ref() == Some(&profile.room)
                    && self
                        .profile
                        .as_ref()
                        .is_some_and(|p| p.room == profile.room && p.user == profile.user)
                {
                    self.profile = Some(profile);
                }
            }
            EventKind::ImagePreview {
                room_id,
                event_id,
                result,
            } => {
                if self.selected.as_ref() == Some(&room_id) {
                    if let Ok(data) = result {
                        self.cache_image(room_id, event_id, data);
                    }
                }
            }
            EventKind::Avatar { key, pixels } => {
                let image = pixels.and_then(|v| cui::Icon::rgba(&v, 128, 128).ok());
                if let Some(image) = image {
                    if self.avatars.len() >= 1024 && !self.avatars.contains_key(&key) {
                        self.avatars.clear();
                    }
                    self.avatars.insert(key, image);
                } else {
                    self.avatars.remove(&key);
                }
                self.avatar_revision = self.avatar_revision.wrapping_add(1);
            }
            EventKind::MemberCount { room_id, count } => {
                self.member_counts.insert(room_id, count);
            }

            EventKind::LiveDetailsFailed { room_id, key } => {
                if self.selected.as_ref() == Some(&room_id) {
                    self.tools.error = Some(key);
                }
            }
            EventKind::Blocked(users) => self.blocked = users,
            EventKind::DraftSaved { room_id, body } => {
                self.saved_drafts.insert(room_id, body);
            }
            EventKind::Notification { .. } => {}
            EventKind::Organization { id, result } => {
                if self.organization.pending.as_ref() == Some(&id) && !self.organization.show {
                    self.error = result.as_ref().err().copied();
                }
                self.organization.apply(id, result);
            }
            EventKind::TransferProgress {
                room_id: _,
                id,
                current,
                total,
            } => {
                if self.transfer_progress.len() >= 256
                    && !self.transfer_progress.contains_key(id.as_str())
                {
                    self.transfer_progress.clear();
                }
                self.transfer_progress
                    .insert(id.to_string(), (current, total));
            }
            EventKind::ReauthenticationRequired => {
                self.reauthentication = true;
                self.browser_login = false;
            }
            EventKind::LiveDetails { room_id, details } => {
                if self.selected.as_ref() == Some(&room_id)
                    && self
                        .tools
                        .details
                        .as_ref()
                        .is_some_and(|d| d.message.id == details.message.id)
                {
                    self.tools.details = Some(*details);
                }
            }
            EventKind::Drafts(drafts) => {
                for (room, body) in drafts {
                    self.saved_drafts.insert(room.clone(), body.clone());
                    self.drafts.entry(room).or_insert(body);
                }
            }
            EventKind::Activity {
                room_id,
                unread,
                latest,
                typing,
            } => {
                if let Some(latest) = latest {
                    if self
                        .activity
                        .viewed
                        .get(&room_id)
                        .is_some_and(|seen| seen.as_ref() != Some(&latest))
                    {
                        self.activity.viewed.remove(&room_id);
                    }
                    self.activity.latest.insert(room_id.clone(), latest);
                }
                let unread = if self.activity.viewed.contains_key(&room_id) {
                    0
                } else {
                    unread
                };
                self.activity.unread.insert(room_id.clone(), unread);
                if let Some(names) = typing {
                    self.activity
                        .typing
                        .insert(room_id, (names, std::time::Instant::now()));
                }
            }
            EventKind::BrowserUrl(_) => {}
            EventKind::SecuritySnapshot(snapshot) => self.security.snapshot = snapshot,
            EventKind::SecurityResult { id, result } => {
                if let Some(error) = self.security.complete(&id, result) {
                    self.apply_error(error);
                }
            }
            EventKind::RoomToolResult { request, result } => {
                if matches!(request.action, archaic_matrix::ToolAction::MarkRead { .. }) {
                    if result.is_err() {
                        self.activity.read.remove(&request.room_id);
                    } else {
                        self.activity.unread.insert(request.room_id.clone(), 0);
                    }
                }
                self.tool_result(request, result)
            }
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
                self.browser_login = false;
                self.reauthentication = false;
                self.user = user;
                self.phase = Phase::Connected;
                self.status = "syncing";
            }
            EventKind::Rooms(rooms) => self.update_rooms(rooms),
            history @ (EventKind::History { .. } | EventKind::HistoryFailed { .. }) => {
                self.apply_history(history)
            }
            EventKind::MessageActionResult { request, result } => {
                self.message_result(request, result)
            }
            EventKind::Outbox(items) => self.update_outbox(items),
            EventKind::ThreadQueued {
                transaction,
                result,
            } => self.thread_queued(&transaction, result),
            EventKind::Delivery {
                room_id,
                transaction,
                event_id,
            } => self.delivery_result(&room_id, &transaction, event_id),
            EventKind::Queued { transaction } => self.confirm_sent(&transaction),
            EventKind::Sent { transaction } => self.confirm_sent(&transaction),
            EventKind::Joined(room) => self.joined(room),
            EventKind::ConversationOpened { room, warning } => {
                self.conversation_opened(room, warning)
            }
            EventKind::CreateFailed(key) => {
                self.creating = false;
                self.create_error = Some(key);
                if key == "session-expired" {
                    self.apply_error(key);
                }
            }
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
        self.hide_blocked();
        if self.image_open.as_ref().is_some_and(|(room, event)| {
            self.selected.as_ref() != Some(room)
                || self.image_message(event).is_none_or(|m| {
                    m.deleted || m.attachment.as_ref().is_none_or(|a| a.kind != "m.image")
                })
        }) {
            self.image_open = None;
            self.tools.preview = None;
        }
    }

    fn hide_blocked(&mut self) {
        let blocked = &self.blocked;
        let selected_blocked = self
            .messages
            .iter()
            .any(|m| self.message_selected.as_ref() == Some(&m.id) && blocked.contains(&m.sender));
        self.messages.retain(|m| !blocked.contains(&m.sender));
        self.tools
            .search
            .messages
            .retain(|m| !blocked.contains(&m.sender));
        for m in self
            .messages
            .iter_mut()
            .chain(self.tools.search.messages.iter_mut())
        {
            m.reactions.retain(|r| !blocked.contains(&r.sender));
        }
        if let Some(details) = &mut self.tools.details {
            if blocked.contains(&details.message.sender) {
                self.tools.details = None;
                self.tools.preview = None;
            } else {
                details.thread.retain(|m| !blocked.contains(&m.sender));
                details
                    .message
                    .reactions
                    .retain(|r| !blocked.contains(&r.sender));
                if details
                    .reply
                    .as_ref()
                    .is_some_and(|m| blocked.contains(&m.sender))
                {
                    details.reply = None;
                    details.reply_unavailable = true;
                }
            }
        }
        if selected_blocked {
            self.message_selected = None;
            self.tools.preview = None;
        }
    }

    fn conversation_opened(&mut self, room: RoomSummary, warning: Option<&'static str>) {
        self.creating = false;
        self.show_new = false;
        self.create_error = None;
        self.room_confirmation = None;
        self.room_feedback = None;
        self.joined(room);
        self.error = warning;
    }
    fn joined(&mut self, room: RoomSummary) {
        let restored = self.switch_history(room.id.clone());
        if let Some(existing) = self
            .rooms
            .iter_mut()
            .find(|existing| existing.id == room.id)
        {
            *existing = room;
        } else {
            self.rooms.push(room);
        }
        self.filter.clear();
        self.joining = false;
        self.show_join = false;
        self.join_error = None;
        self.status = if restored { "ready" } else { "loading" };
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
            if self.saved_drafts.get(&pending.room_id) == Some(&pending.body) {
                self.saved_drafts.remove(&pending.room_id);
            }
            self.sending = false;
            self.error = None;
        }
    }

    pub fn member_count(&self, room: &RoomSummary) -> Option<u64> {
        // SDK membership synchronization can finish without updating its count
        // summary. A joined room's zero summary is not an authoritative count.
        room.info
            .members_known
            .then_some(room.info.members)
            .filter(|count| *count > 0)
            .or_else(|| self.member_counts.get(&room.id).copied())
    }

    pub fn visible_rooms(&self) -> Vec<&RoomSummary> {
        let filter = self.filter.to_lowercase();
        let mut children = std::collections::HashSet::new();
        let mut pending = self.active_space.clone().into_iter().collect::<Vec<_>>();
        while let Some(id) = pending.pop() {
            if children.insert(id.clone())
                && let Some(room) = self.rooms.iter().find(|r| r.id == id)
            {
                pending.extend(room.info.children.iter().cloned());
            }
        }
        self.rooms
            .iter()
            .filter(|room| !self.direct_only || room.info.direct)
            .filter(|room| self.active_space.is_none() || children.contains(&room.id))
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
        if self.tools_busy()
            || self.message_draft.is_some()
            || self.sending
            || self.creating
            || self.phase != Phase::Connected
        {
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
    #[test]
    fn names_are_scoped_and_stale_accounts_cannot_change_them() {
        let mut model = super::Model {
            epoch: 7,
            user: "@me:local".into(),
            selected: Some("!a:local".into()),
            ..Default::default()
        };
        for (key, name) in [
            ("account", "My name"),
            ("user:!a:local:@bob:local", "Bobby"),
        ] {
            model.apply(archaic_matrix::Event {
                epoch: 7,
                kind: archaic_matrix::EventKind::DisplayName {
                    key: key.into(),
                    name: name.into(),
                },
            });
        }
        assert_eq!(model.account_name(), "My name");
        assert_eq!(model.user_name("@me:local"), "My name");
        assert_eq!(model.user_name("@bob:local"), "Bobby");
        model.selected = Some("!other:local".into());
        assert_eq!(model.user_name("@bob:local"), "@bob:local");
        model.apply(archaic_matrix::Event {
            epoch: 6,
            kind: archaic_matrix::EventKind::DisplayName {
                key: "account".into(),
                name: "Other account".into(),
            },
        });
        assert_eq!(model.account_name(), "My name");
        model.apply(archaic_matrix::Event {
            epoch: 7,
            kind: archaic_matrix::EventKind::DisplayName {
                key: "account".into(),
                name: "".into(),
            },
        });
        assert_eq!(model.account_name(), "@me:local");
    }

    use super::*;
    #[test]
    fn switching_rooms_restores_history_and_clears_it_on_logout_or_membership_loss() {
        use archaic_matrix::{HistoryInfo, HistoryLoad};
        let mut m = connected();
        m.messages = vec![Message {
            id: "$old".into(),
            body: Some("Cached history".into()),
            ..Default::default()
        }];
        m.history_info.can_load_more = true;
        m.status = "ready";
        assert!(!m.switch_history("!b:local".into()));
        assert!(m.messages.is_empty());
        m.status = "loading";
        m.apply(Event {
            epoch: 1,
            kind: EventKind::History {
                room_id: "!a:local".into(),
                messages: vec![],
                info: HistoryInfo::default(),
                kind: HistoryLoad::Recent,
            },
        });
        assert!(m.switch_history("!a:local".into()));
        assert_eq!(m.messages[0].id, "$old");
        assert!(m.history_info.can_load_more);
        m.status = "ready";
        m.switch_history("!b:local".into());
        m.status = "loading";
        m.apply(Event {
            epoch: 2,
            kind: EventKind::History {
                room_id: "!a:local".into(),
                messages: vec![Message {
                    id: "$updated".into(),
                    ..Default::default()
                }],
                info: HistoryInfo::default(),
                kind: HistoryLoad::Recent,
            },
        });
        assert!(m.messages.is_empty());
        assert!(m.switch_history("!a:local".into()));
        assert_eq!(m.messages[0].id, "$updated");
        m.switch_history("!b:local".into());
        m.update_rooms(
            m.rooms
                .iter()
                .filter(|r| r.id != "!a:local")
                .cloned()
                .collect(),
        );
        assert!(m.history_cache.is_empty());
        m.status = "ready";
        m.messages = vec![Message::default()];
        m.switch_history("!a:local".into());
        assert!(!m.history_cache.is_empty());
        m.apply(Event {
            epoch: 2,
            kind: EventKind::SignedOut,
        });
        assert!(m.history_cache.is_empty());
    }

    fn connected() -> Model {
        Model {
            epoch: 2,
            phase: Phase::Connected,
            rooms: ["!a:local", "!b:local"]
                .into_iter()
                .map(|id| RoomSummary {
                    info: Default::default(),
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
    fn home_direct_and_nested_spaces_are_independent_filters() {
        let mut model = connected();
        model.rooms[1].info.direct = true;
        let mut outer = model.rooms[0].clone();
        outer.id = "!space:local".into();
        outer.info.space = true;
        outer.info.children = vec!["!nested:local".into()];
        let mut inner = outer.clone();
        inner.id = "!nested:local".into();
        inner.info.children = vec!["!space:local".into(), "!a:local".into()];
        model.rooms.extend([outer, inner]);
        model.direct_only = true;
        assert_eq!(
            model
                .visible_rooms()
                .iter()
                .map(|r| r.id.as_str())
                .collect::<Vec<_>>(),
            ["!b:local"]
        );
        model.direct_only = false;
        model.active_space = Some("!space:local".into());
        let rooms = model.visible_rooms();
        assert!(rooms.iter().any(|r| r.id == "!a:local"));
        assert!(!rooms.iter().any(|r| r.id == "!b:local"));
        model.filter = "missing".into();
        assert!(model.visible_rooms().is_empty());
        model.active_space = None;
        model.filter.clear();
        assert_eq!(model.visible_rooms().len(), 4);
    }

    #[test]
    fn fetched_members_replace_zero_summaries_but_live_summaries_take_precedence() {
        let mut model = connected();
        model.rooms[0].info.members_known = true;
        assert_eq!(model.member_count(&model.rooms[0]), None);
        model.member_counts.insert("!a:local".into(), 3);
        assert_eq!(model.member_count(&model.rooms[0]), Some(3));
        model.rooms[0].info.members = 4;
        assert_eq!(model.member_count(&model.rooms[0]), Some(4));
    }

    #[test]
    fn member_count_events_are_scoped_to_the_current_session() {
        let mut model = connected();
        model.apply(Event {
            epoch: 1,
            kind: EventKind::MemberCount {
                room_id: "!a:local".into(),
                count: 99,
            },
        });
        assert!(model.member_counts.is_empty());
        model.apply(Event {
            epoch: 2,
            kind: EventKind::MemberCount {
                room_id: "!a:local".into(),
                count: 3,
            },
        });
        assert_eq!(model.member_counts["!a:local"], 3);
    }

    #[test]
    fn queue_acknowledgement_clears_saved_draft_without_erasing_later_input() {
        for later in [None, Some("new text")] {
            let mut model = connected();
            model.saved_drafts = model.drafts.clone();
            let Command::Send { transaction, .. } = model.send_command().unwrap() else {
                unreachable!()
            };
            if let Some(body) = later {
                model.drafts.insert("!a:local".into(), body.into());
            }
            model.apply(Event {
                epoch: 2,
                kind: EventKind::Queued { transaction },
            });
            assert_eq!(model.drafts.get("!a:local").map(String::as_str), later);
            assert!(!model.saved_drafts.contains_key("!a:local"));
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
                info: Default::default(),
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
                info: Default::default(),
                kind: archaic_matrix::HistoryLoad::Recent,
                room_id: "!b:local".into(),
                messages: vec![Message {
                    id: "old".into(),
                    sender: "alice".into(),
                    body: None,
                    ..Message::default()
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
    #[test]
    fn pagination_error_survives_sync_but_retry_success_clears_it() {
        use archaic_matrix::{HistoryInfo, HistoryLoad};
        let mut model = connected();
        model.history_info.can_load_more = true;
        assert!(matches!(
            model.history_command(HistoryLoad::Older),
            Some(Command::LoadHistory {
                kind: HistoryLoad::Older,
                ..
            })
        ));
        model.history_pending = Some(HistoryLoad::Older);
        assert!(model.history_command(HistoryLoad::Older).is_none());
        let recent = || Event {
            epoch: 2,
            kind: EventKind::History {
                room_id: "!a:local".into(),
                messages: vec![],
                kind: HistoryLoad::Recent,
                info: HistoryInfo {
                    can_load_newer: false,
                    can_load_more: true,
                    ..Default::default()
                },
            },
        };
        model.apply(recent());
        assert_eq!(model.history_pending, Some(HistoryLoad::Older));
        model.apply(Event {
            epoch: 2,
            kind: EventKind::HistoryFailed {
                room_id: "!a:local".into(),
                kind: HistoryLoad::Older,
                key: "history-load-failed",
            },
        });
        model.apply(recent());
        assert_eq!(
            model.history_error,
            Some((HistoryLoad::Older, "history-load-failed"))
        );
        assert!(model.history_pending.is_none());
        assert!(model.history_command(HistoryLoad::Older).is_some());
        model.apply(Event {
            epoch: 2,
            kind: EventKind::History {
                room_id: "!a:local".into(),
                messages: vec![],
                kind: HistoryLoad::Older,
                info: HistoryInfo::default(),
            },
        });
        assert!(model.history_error.is_none());
        assert!(model.history_command(HistoryLoad::Older).is_none());
        assert!(model.history_command(HistoryLoad::Latest).is_some());
    }
    #[test]
    fn late_history_failures_do_not_cross_room_session_or_membership_boundaries() {
        use archaic_matrix::HistoryLoad;
        let mut model = connected();
        for (epoch, room_id) in [(1, "!a:local"), (2, "!b:local")] {
            model.apply(Event {
                epoch,
                kind: EventKind::HistoryFailed {
                    room_id: room_id.into(),
                    kind: HistoryLoad::Older,
                    key: "history-load-failed",
                },
            });
        }
        assert!(model.history_error.is_none());
        model.rooms[0].membership = archaic_matrix::Membership::Invited;
        model.apply(Event {
            epoch: 2,
            kind: EventKind::HistoryFailed {
                room_id: "!a:local".into(),
                kind: HistoryLoad::Older,
                key: "history-load-failed",
            },
        });
        assert!(model.history_error.is_none());
        assert!(model.history_command(HistoryLoad::Latest).is_none());
        model.rooms[0].membership = archaic_matrix::Membership::Joined;
        model.phase = Phase::SigningOut;
        assert!(model.history_command(HistoryLoad::Latest).is_none());
    }
    #[test]
    fn conversation_creation_preserves_drafts_on_failure_and_selects_success() {
        let mut model = connected();
        model.show_new = true;
        model.creating = true;
        assert!(!model.can_create());
        assert!(model.send_command().is_none());
        model.apply(Event {
            epoch: 2,
            kind: EventKind::CreateFailed("create-forbidden"),
        });
        assert!(model.can_create() && model.show_new);
        assert_eq!(model.selected.as_deref(), Some("!a:local"));
        assert_eq!(model.draft(), "hello");
        assert_eq!(model.create_error, Some("create-forbidden"));
        model.creating = true;
        model.filter = "old filter".into();
        model.apply(Event {
            epoch: 2,
            kind: EventKind::ConversationOpened {
                room: RoomSummary {
                    info: Default::default(),
                    id: "!created:local".into(),
                    name: "New group".into(),
                    membership: archaic_matrix::Membership::Joined,
                    inviter: None,
                },
                warning: Some("create-direct-tag-failed"),
            },
        });
        assert!(!model.creating && !model.show_new && model.create_error.is_none());
        assert_eq!(model.selected.as_deref(), Some("!created:local"));
        assert!(model.filter.is_empty());
        assert_eq!(model.drafts["!a:local"], "hello");
        assert_eq!(model.error, Some("create-direct-tag-failed"));
    }
    #[test]
    fn creation_ignores_old_epochs_and_expired_sessions_block_new_operations() {
        let mut model = connected();
        model.show_new = true;
        model.creating = true;
        model.apply(Event {
            epoch: 1,
            kind: EventKind::CreateFailed("create-forbidden"),
        });
        assert!(model.creating && model.create_error.is_none());
        model.apply(Event {
            epoch: 2,
            kind: EventKind::CreateFailed("session-expired"),
        });
        assert!(!model.creating && !model.can_create());
        assert!(model.phase == Phase::LogoutCleanup);
        assert!(model.send_command().is_none());
        model.apply(Event {
            epoch: 2,
            kind: EventKind::SignedOut,
        });
        assert!(!model.show_new && model.create_error.is_none());
    }
    #[test]
    fn room_tools_reject_stale_results_block_conflicts_and_preserve_composer() {
        use archaic_matrix::{SearchMode, SearchPage, ToolAction, ToolData, ToolRequest};
        let mut model = connected();
        let request = ToolRequest {
            room_id: "!a:local".into(),
            id: transaction_id(),
            action: ToolAction::Search {
                query: "hello".into(),
                mode: SearchMode::Local,
                more: false,
            },
        };
        model.tools.pending = Some(request.clone());
        assert!(!model.can_tool());
        assert!(model.send_command().is_none());
        assert!(!model.can_message_action());
        model.apply(Event {
            epoch: 1,
            kind: EventKind::RoomToolResult {
                request: request.clone(),
                result: Ok(ToolData::Search(SearchPage::default())),
            },
        });
        assert!(model.tools.pending.is_some());
        let mut wrong = request.clone();
        wrong.room_id = "!other:local".into();
        model.apply(Event {
            epoch: 2,
            kind: EventKind::RoomToolResult {
                request: wrong,
                result: Ok(ToolData::Search(SearchPage::default())),
            },
        });
        assert!(model.tools.pending.is_some());
        model.apply(Event {
            epoch: 2,
            kind: EventKind::RoomToolResult {
                request: request.clone(),
                result: Err("search-failed"),
            },
        });
        assert!(model.tools.pending.is_none());
        assert_eq!(model.tools.error, Some("search-failed"));
        assert_eq!(model.draft(), "hello");
        model.tools.pending = Some(request.clone());
        model.apply(Event {
            epoch: 2,
            kind: EventKind::RoomToolResult {
                request,
                result: Ok(ToolData::Search(SearchPage::default())),
            },
        });
        assert!(model.tools.error.is_none());
        assert!(model.can_tool());
        assert_eq!(
            model.tools.search_key,
            Some(("hello".into(), SearchMode::Local))
        );
        model.clear_history();
        assert!(model.tools.search_key.is_none());
        assert!(!model.tools_busy());
    }
}

#[cfg(test)]
mod unread_tests {
    use super::*;
    #[test]
    fn opened_room_suppresses_stale_unread_but_new_activity_restores_it() {
        let mut model = Model::default();
        model
            .activity
            .viewed
            .insert("!r:x".into(), Some("$old".into()));
        for (latest, expected) in [(None, 0), (Some("$old"), 0), (Some("$new"), 1)] {
            model.apply(Event {
                epoch: 0,
                kind: EventKind::Activity {
                    room_id: "!r:x".into(),
                    unread: 1,
                    latest: latest.map(str::to_owned),
                    typing: None,
                },
            });
            assert_eq!(model.activity.unread["!r:x"], expected);
        }
    }
}
