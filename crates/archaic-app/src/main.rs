#![recursion_limit = "256"]
mod activity;
mod appearance;
#[cfg(target_os = "macos")]
mod apple_links;
mod desktop;
mod history_controls;
mod history_state;
mod instance;
mod join_form;
mod login;
mod message_controls;
mod message_state;
mod model;
mod new_conversation;
mod options;
mod organization;
mod outbox;
mod room_controls;
mod room_state;
mod search_snippet;
mod security;
mod status_item;
mod timeline_view;
mod tool_controller;
mod tool_controls;
mod tool_state;
mod view;
mod widgets;
use archaic_i18n::Translator;
use archaic_matrix::{Command, Event, Membership, RoomAction, Sender};
use cui::{App, Result};
use model::{Model, Phase};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use view::View;

struct Controller {
    view: View,
    window: cui::Window,
    self_weak: RefCell<std::rc::Weak<Controller>>,
    model: RefCell<Model>,
    tr: Translator,
    sender: Sender,
    rendering: Cell<bool>,
    active_profile: RefCell<String>,
    startup_link: Cell<bool>,
    status_item: RefCell<Option<status_item::StatusItem>>,
    status_item_attempted: Cell<bool>,
    desktop_commands: RefCell<Vec<cui::Command>>,
    notification_sender: desktop::Notifications,
    pending_profile: RefCell<Option<String>>,
    room_labels: RefCell<Vec<String>>,
    fixture: bool,
    app: std::rc::Weak<App>,
    preference_path: Option<std::path::PathBuf>,
}

impl Controller {
    fn render(&self) -> Result<()> {
        self.rendering.set(true);
        let result = self.render_inner();
        self.rendering.set(false);
        result
    }
    fn render_inner(&self) -> Result<()> {
        let model = self.model.borrow();
        for command in self.desktop_commands.borrow().iter() {
            command.set_enabled(model.phase == Phase::Connected)?;
        }
        let visible = model.visible_rooms();
        let labels = visible
            .iter()
            .map(|room| {
                if room.membership == Membership::Invited {
                    format!(
                        "{} · {}",
                        self.tr.text("invitation"),
                        room.name.replace('\0', "�")
                    )
                } else {
                    let name = room.name.replace('\0', "�");
                    match model.activity.unread.get(&room.id) {
                        Some(n) if *n > 0 => format!("{name} ({n})"),
                        _ => name,
                    }
                }
            })
            .collect::<Vec<_>>();
        if *self.room_labels.borrow() != labels {
            self.view
                .rooms
                .set_items(&labels.iter().map(String::as_str).collect::<Vec<_>>())?;
            *self.room_labels.borrow_mut() = labels;
        }
        let index = visible
            .iter()
            .position(|room| Some(&room.id) == model.selected.as_ref())
            .map(|index| index as i32)
            .unwrap_or(-1);
        if self.view.rooms.get_selected()? != index {
            self.view.rooms.set_selected(index)?;
        }
        self.view.render(&model, &self.tr, self.fixture)
    }
    fn enqueue(&self, command: Command) -> bool {
        if self.sender.try_send(command).is_ok() {
            true
        } else {
            self.model.borrow_mut().error = Some("queue-full");
            false
        }
    }
    fn session_action(&self, temporary: bool) -> Result<()> {
        let epoch = self.model.borrow().epoch + 1;
        let command = if temporary {
            Command::Temporary { epoch }
        } else {
            Command::Restore { epoch }
        };
        if self.enqueue(command) {
            let mut model = self.model.borrow_mut();
            model.epoch = epoch;
            model.phase = Phase::Restoring;
            model.temporary = temporary;
            model.error = None;
        }
        self.render()
    }
    fn switch_profile(&self) -> Result<()> {
        self.remember_draft()?;
        let name = self.view.profile.text()?;
        if self.fixture {
            return Ok(());
        }
        if archaic_matrix::storage::validate_profile_name(&name).is_err() {
            self.model.borrow_mut().error = Some("profile-invalid");
            return self.render();
        }
        if !self.desktop_ready() {
            return Ok(());
        }
        self.flush_drafts();
        if self.drafts_dirty() {
            *self.pending_profile.borrow_mut() = Some(name);
            self.model.borrow_mut().status = "profile-saving";
            return self.render();
        }
        self.pending_profile.borrow_mut().take();
        let epoch = self.model.borrow().epoch + 1;
        if self.enqueue(Command::SwitchProfile {
            epoch,
            name: name.clone(),
        }) {
            self.notification_sender.invalidate();
            *self.active_profile.borrow_mut() = name;
            self.view.notifications.set_checked(false)?;
            self.view.typing_opt.set_checked(false)?;
            self.view.read_opt.set_checked(false)?;
            *self.model.borrow_mut() = Model {
                epoch,
                phase: Phase::Restoring,
                status: "restoring-session",
                ..Default::default()
            };
            self.view.composer.set_text("")?;
            self.view.password.set_text("")?;
        }
        self.render()
    }
    fn temporary(&self) -> Result<()> {
        self.session_action(true)
    }
    fn oauth_login(&self) -> Result<()> {
        self.start_browser_login(true)
    }
    fn browser_login(&self) -> Result<()> {
        self.start_browser_login(false)
    }
    fn start_browser_login(&self, oauth: bool) -> Result<()> {
        if self.model.borrow().phase != Phase::SignedOut {
            return Ok(());
        }
        let epoch = self.model.borrow().epoch + 1;
        let server = self.view.server.text()?;
        let command = if oauth {
            Command::OAuthLogin { epoch, server }
        } else {
            Command::BrowserLogin { epoch, server }
        };
        if self.enqueue(command) {
            let mut m = self.model.borrow_mut();
            m.epoch = epoch;
            m.phase = Phase::Connecting;
            m.browser_login = true;
            m.error = None;
        }
        self.render()
    }
    fn cancel_login(&self) -> Result<()> {
        self.enqueue(Command::CancelLogin);
        self.model.borrow_mut().browser_login = false;
        self.render()
    }
    fn login(&self) -> Result<()> {
        if self.model.borrow().phase == Phase::RestoreBlocked {
            return self.session_action(false);
        }
        if self.model.borrow().phase != Phase::SignedOut {
            return Ok(());
        }
        let epoch = self.model.borrow().epoch + 1;
        let command = Command::Login {
            epoch,
            server: self.view.server.text()?,
            username: self.view.username.text()?,
            password: self.view.password.text()?,
        };
        if self.enqueue(command) {
            let mut model = self.model.borrow_mut();
            model.epoch = epoch;
            model.phase = Phase::Connecting;
            model.browser_login = false;
            model.status = "signing-in";
            model.error = None;
            drop(model);
            self.view.password.set_text("")?;
        }
        self.render()
    }
    fn remember_draft(&self) -> Result<()> {
        let text = self.view.composer.text()?;
        let action_body = self.view.message_controls.body.text()?;
        let mut model = self.model.borrow_mut();
        if let Some(draft) = &mut model.message_draft
            && !draft.pending
        {
            draft.body = action_body;
        }
        let changed = model
            .selected
            .clone()
            .filter(|id| model.drafts.get(id) != Some(&text));
        if let Some(id) = changed {
            model.drafts.insert(id.clone(), text.clone());
        }
        Ok(())
    }
    fn flush_drafts(&self) {
        if self.fixture {
            return;
        }
        let changes = {
            let m = self.model.borrow();
            if m.phase != Phase::Connected || m.sending {
                return;
            }
            m.drafts
                .iter()
                .filter(|(id, body)| {
                    m.saved_drafts.get(*id).map(String::as_str).unwrap_or("") != body.as_str()
                })
                .map(|(id, body)| (id.clone(), body.clone()))
                .collect::<Vec<_>>()
        };
        for (room_id, body) in changes {
            if !self.enqueue(Command::SaveDraft { room_id, body }) {
                break;
            }
        }
    }
    fn select(&self) -> Result<()> {
        if self.model.borrow().message_draft.is_some() || self.model.borrow().tools_busy() {
            return Ok(());
        }
        self.remember_draft()?;
        let index = self.view.rooms.get_selected()?;
        let id = usize::try_from(index).ok().and_then(|index| {
            self.model
                .borrow()
                .visible_rooms()
                .get(index)
                .map(|room| room.id.clone())
        });
        if let Some(id) = id {
            if self.model.borrow().selected.as_ref() == Some(&id) {
                return Ok(());
            }
            if self.fixture || self.enqueue(Command::Select(id.clone())) {
                let mut model = self.model.borrow_mut();
                model.selected = Some(id);
                model.room_confirmation = None;
                model.room_feedback = None;
                self.view.room_controls.invite_user.set_text("")?;
                model.clear_history();
                model.status = if self.fixture {
                    "preview-fixture"
                } else if model
                    .selected_room()
                    .is_some_and(|room| room.membership == Membership::Invited)
                {
                    "ready"
                } else {
                    "loading"
                };
                model.error = None;
            }
        }
        self.render()
    }
    fn send(&self) -> Result<()> {
        self.remember_draft()?;
        let command = self
            .model
            .borrow_mut()
            .send_command()
            .map(|command| match command {
                Command::Send {
                    room_id,
                    body,
                    transaction,
                } => Command::Enqueue {
                    room_id,
                    body,
                    transaction,
                },
                other => other,
            });
        if let Some(command) = command
            && self.enqueue(command)
        {
            let mut model = self.model.borrow_mut();
            model.sending = true;
            model.error = None;
        }
        self.render()
    }
    fn reauthenticate(&self) -> Result<()> {
        if self.model.borrow().reauthentication {
            let password = self.view.reauth_password.text()?;
            if !password.is_empty()
                && self.enqueue(Command::Reauthenticate {
                    password: archaic_matrix::security::Secret::new(password),
                })
            {
                self.view.reauth_password.set_text("")?;
            }
        }
        Ok(())
    }
    fn logout(&self) -> Result<()> {
        if self.enqueue(Command::Logout) {
            let mut model = self.model.borrow_mut();
            model.phase = Phase::SigningOut;
            model.status = "signing-out";
            model.error = None;
        }
        self.render()
    }
    fn refresh(&self) -> Result<()> {
        self.remember_draft()?;
        if self.enqueue(Command::Refresh) {
            self.model.borrow_mut().error = None;
        }
        self.render()
    }
    fn newer_history(&self) -> Result<()> {
        self.load_history(archaic_matrix::HistoryLoad::Newer)
    }
    fn message_context(&self) -> Result<()> {
        self.remember_draft()?;
        let target = {
            let m = self.model.borrow();
            if self.fixture || !m.can_message_action() || m.history_pending.is_some() {
                return Ok(());
            }
            m.selected_message()
                .map(|message| (m.selected.clone().unwrap(), message.id.clone()))
        };
        if let Some((room_id, event_id)) = target
            && self.enqueue(Command::Context { room_id, event_id })
        {
            let mut m = self.model.borrow_mut();
            m.tools.show = false;
            m.history_pending = Some(archaic_matrix::HistoryLoad::Latest);
        }
        self.render()
    }
    fn older_history(&self) -> Result<()> {
        self.load_history(archaic_matrix::HistoryLoad::Older)
    }
    fn latest_history(&self) -> Result<()> {
        self.load_history(archaic_matrix::HistoryLoad::Latest)
    }
    fn load_history(&self, kind: archaic_matrix::HistoryLoad) -> Result<()> {
        self.remember_draft()?;
        let command = self.model.borrow().history_command(kind);
        if !self.fixture
            && let Some(command) = command
            && self.enqueue(command)
        {
            let mut model = self.model.borrow_mut();
            model.history_pending = Some(kind);
            model.history_error = None;
        }
        self.render()
    }
    fn filter(&self) -> Result<()> {
        self.remember_draft()?;
        self.model.borrow_mut().filter = self.view.search.text()?;
        self.render()
    }
    fn appearance(&self) -> Result<()> {
        let appearance = appearance::Appearance::from_index(self.view.appearance.get_selected()?);
        if let Some(app) = self.app.upgrade() {
            app.theme(appearance.theme());
        }
        let failed = self
            .preference_path
            .as_ref()
            .is_some_and(|path| appearance.save(path).is_err());
        self.view.appearance_error.set_visible(failed)?;
        Ok(())
    }
    fn toggle_new(&self) -> Result<()> {
        self.remember_draft()?;
        let mut model = self.model.borrow_mut();
        if !model.can_create() || self.fixture {
            return Ok(());
        }
        model.show_new = !model.show_new;
        model.show_join = false;
        model.create_error = None;
        drop(model);
        self.render()
    }
    fn create_mode(&self) -> Result<()> {
        self.model.borrow_mut().create_error = None;
        self.render()
    }
    fn create_conversation(&self) -> Result<()> {
        if !self.model.borrow().can_create() || self.fixture {
            return Ok(());
        }
        if self.enqueue(Command::NewConversation(
            self.view.new_conversation.input()?,
        )) {
            let mut model = self.model.borrow_mut();
            model.creating = true;
            model.create_error = None;
        }
        self.render()
    }
    fn toggle_join(&self) -> Result<()> {
        if self.model.borrow().message_draft.is_some() || self.model.borrow().tools_busy() {
            return Ok(());
        }
        let mut model = self.model.borrow_mut();
        if model.phase != Phase::Connected || model.joining || model.creating || self.fixture {
            return Ok(());
        }
        model.show_join = !model.show_join;
        model.join_error = None;
        drop(model);
        self.render()
    }
    fn join(&self) -> Result<()> {
        if self.model.borrow().message_draft.is_some() || self.model.borrow().tools_busy() {
            return Ok(());
        }
        if self.fixture
            || self.model.borrow().room_pending.is_some()
            || self.model.borrow().joining
            || self.model.borrow().creating
            || self.model.borrow().phase != Phase::Connected
        {
            return Ok(());
        }
        self.remember_draft()?;
        let address = self.view.join.join_address.text()?;
        let command = match self.model.borrow().link_join.as_ref() {
            Some((original, via, event)) if original == address.trim() => Command::JoinVia {
                address,
                via: via.clone(),
                event: event.clone(),
            },
            _ => Command::Join(address),
        };
        if self.enqueue(command) {
            let mut model = self.model.borrow_mut();
            model.joining = true;
            model.join_error = None;
        }
        self.render()
    }
    fn accept_invitation(&self) -> Result<()> {
        self.room_action(RoomAction::Accept, false)
    }
    fn decline_invitation(&self) -> Result<()> {
        self.room_action(RoomAction::Decline, true)
    }
    fn leave_room(&self) -> Result<()> {
        self.room_action(RoomAction::Leave, true)
    }
    fn invite_user(&self) -> Result<()> {
        self.room_action(
            RoomAction::Invite(self.view.room_controls.invite_user.text()?),
            false,
        )
    }
    fn cancel_room_action(&self) -> Result<()> {
        if self.model.borrow().room_pending.is_none() {
            self.model.borrow_mut().room_confirmation = None;
        }
        self.render()
    }
    fn confirm_room_action(&self) -> Result<()> {
        let confirmation = self.model.borrow().room_confirmation.clone();
        let selected = self.model.borrow().selected.clone();
        if let Some((id, action)) = confirmation
            && selected.as_ref() == Some(&id)
        {
            return self.room_action(action, false);
        }
        Ok(())
    }
    fn room_action(&self, action: RoomAction, confirm: bool) -> Result<()> {
        if self.fixture || !self.model.borrow().room_action_allowed(&action) {
            return Ok(());
        }
        self.remember_draft()?;
        let Some(room_id) = self.model.borrow().selected.clone() else {
            return Ok(());
        };
        if confirm {
            let mut model = self.model.borrow_mut();
            model.room_confirmation = Some((room_id, action));
            model.room_feedback = None;
        } else if self.enqueue(Command::RoomAction {
            room_id: room_id.clone(),
            action: action.clone(),
        }) {
            let mut model = self.model.borrow_mut();
            model.room_pending = Some((room_id, action));
            model.room_feedback = None;
        }
        self.render()
    }
    fn event(&self, event: Event) -> Result<()> {
        if let archaic_matrix::EventKind::Notification { room_id, name } = &event.kind {
            self.desktop_notification(event.epoch, room_id, name)?;
        }
        if event.epoch == self.model.borrow().epoch
            && self.model.borrow().browser_login
            && self.model.borrow().phase == Phase::Connecting
            && let archaic_matrix::EventKind::BrowserUrl(url) = &event.kind
            && open_browser(url).is_err()
        {
            self.enqueue(Command::CancelLogin);
            self.model.borrow_mut().error = Some("sso-browser-failed");
        }
        self.remember_draft()?;
        let joined = event.epoch == self.model.borrow().epoch
            && matches!(
                event.kind,
                archaic_matrix::EventKind::Joined(_)
                    | archaic_matrix::EventKind::ConversationOpened { .. }
            );
        let clear_create = event.epoch == self.model.borrow().epoch
            && matches!(
                event.kind,
                archaic_matrix::EventKind::ConversationOpened { .. }
                    | archaic_matrix::EventKind::SignedOut
            );
        if event.epoch == self.model.borrow().epoch
            && let archaic_matrix::EventKind::RoomActionResult {
                room_id,
                action: RoomAction::Invite(_),
                result: Ok(()),
            } = &event.kind
            && self.model.borrow().selected.as_ref() == Some(room_id)
        {
            self.view.room_controls.invite_user.set_text("")?;
        }
        let signed_out = event.epoch == self.model.borrow().epoch
            && matches!(event.kind, archaic_matrix::EventKind::SignedOut);
        let rooms_ready = event.epoch == self.model.borrow().epoch
            && matches!(event.kind, archaic_matrix::EventKind::Rooms(_));
        self.model.borrow_mut().apply(event);
        if signed_out {
            self.notification_sender.invalidate();
            self.view.notifications.set_checked(false)?;
            self.view.typing_opt.set_checked(false)?;
            self.view.read_opt.set_checked(false)?;
        }
        if clear_create {
            self.rendering.set(true);
            let result = self
                .view
                .new_conversation
                .clear()
                .and_then(|_| self.view.security.clear());
            self.rendering.set(false);
            result?;
        }
        if joined {
            self.rendering.set(true);
            let cleared = self
                .view
                .search
                .set_text("")
                .and_then(|_| self.view.join.join_address.set_text(""));
            self.rendering.set(false);
            cleared?;
        }
        if rooms_ready && self.startup_link.get() && self.desktop_ready() {
            self.startup_link.set(false);
            self.open_matrix_link()?;
        }
        self.render()
    }
    fn bind(self: &Rc<Self>) -> Result<()> {
        for (widget, action) in [
            (&self.view.password, Self::login as fn(&Self) -> Result<()>),
            (&self.view.join.join_address, Self::join),
            (&self.view.tools.query, Self::run_search),
            (
                &self.view.new_conversation.person,
                Self::create_conversation,
            ),
        ] {
            let controller = self.clone();
            widget.on_key(move |key, _| {
                if key == cui::sys::CUI_KEY_ENTER && !controller.rendering.get() {
                    action(&controller).expect("live CUI controls");
                    true
                } else {
                    false
                }
            })?;
        }
        for (widget, action) in [
            (&self.view.sign_in, Self::login as fn(&Self) -> Result<()>),
            (&self.view.browser_login, Self::browser_login),
            (&self.view.oauth_login, Self::oauth_login),
            (&self.view.cancel_login, Self::cancel_login),
            (&self.view.temporary, Self::temporary),
            (&self.view.rooms, Self::select),
            (&self.view.timeline.rows, Self::choose_timeline_message),
            (&self.view.send, Self::send),
            (&self.view.composer, Self::composer_input),
            (&self.view.outbox.retry, Self::retry_outbox),
            (&self.view.outbox.cancel, Self::cancel_outbox),
            (&self.view.security_toggle, Self::open_security),
            (&self.view.security.close, Self::close_security),
            (&self.view.security.refresh, Self::refresh_security),
            (&self.view.security.mode, Self::security_mode),
            (&self.view.security.devices, Self::security_selection),
            (&self.view.security.verify, Self::verify_device),
            (&self.view.security.accept, Self::accept_verification),
            (&self.view.security.start, Self::start_verification),
            (&self.view.security.confirm, Self::confirm_verification),
            (&self.view.security.mismatch, Self::mismatch_verification),
            (&self.view.security.cancel, Self::cancel_verification),
            (&self.view.security.recover, Self::recover_security),
            (&self.view.security.bootstrap, Self::bootstrap_security),
            (&self.view.security.enable, Self::enable_recovery),
            (&self.view.security.dismiss_key, Self::dismiss_recovery_key),
            (&self.view.security.room_keys, Self::recover_room_keys),
            (&self.view.security.export, Self::export_keys),
            (&self.view.security.import, Self::import_keys),
            (&self.view.tools_toggle, Self::toggle_tools),
            (&self.view.tools.mode, Self::tool_mode),
            (&self.view.tools.choose_file, Self::choose_file),
            (&self.view.tools.upload, Self::upload_file),
            (&self.view.tools.discard, Self::discard_upload),
            (&self.view.tools.search, Self::run_search),
            (&self.view.tools.more_search, Self::more_search),
            (&self.view.tools.search_mode, Self::search_input),
            (&self.view.tools.query, Self::search_input),
            (&self.view.tools.hits, Self::tool_selection),
            (&self.view.tools.open_hit, Self::open_hit),
            (&self.view.tools.details, Self::load_details),
            (&self.view.tools.more_details, Self::more_details),
            (&self.view.tools.original, Self::open_original),
            (&self.view.tools.download, Self::download_file),
            (&self.view.tools.preview, Self::preview_file),
            (&self.view.tools.cancel_transfer, Self::cancel_transfer),
            (&self.view.tools.mark_read, Self::mark_read),
            (&self.view.message_controls.select, Self::choose_message),
            (&self.view.message_controls.reply, Self::reply_message),
            (&self.view.message_controls.thread, Self::thread_message),
            (&self.view.message_controls.forward, Self::forward_message),
            (&self.view.message_controls.edit, Self::edit_message),
            (&self.view.message_controls.delete, Self::delete_message),
            (&self.view.message_controls.react, Self::react_message),
            (&self.view.message_controls.cancel, Self::cancel_message),
            (&self.view.message_controls.submit, Self::submit_message),
            (&self.view.sign_out, Self::logout),
            (&self.view.open_link, Self::open_matrix_link),
            (&self.view.switch_profile, Self::switch_profile),
            (&self.view.organization_toggle, Self::toggle_organization),
            (&self.view.organization.close, Self::toggle_organization),
            (&self.view.organization.apply, Self::organization_run),
            (&self.view.organization.more, Self::organization_more),
            (&self.view.organization.select, Self::organization_select),
            (&self.view.organization.join, Self::organization_join),
            (&self.view.reauth, Self::reauthenticate),
            (&self.view.refresh, Self::refresh),
            (&self.view.history_controls.older, Self::older_history),
            (&self.view.history_controls.newer, Self::newer_history),
            (&self.view.message_controls.context, Self::message_context),
            (&self.view.history_controls.latest, Self::latest_history),
            (&self.view.search, Self::filter),
            (&self.view.new_toggle, Self::toggle_new),
            (&self.view.new_conversation.cancel, Self::toggle_new),
            (
                &self.view.new_conversation.submit,
                Self::create_conversation,
            ),
            (&self.view.new_conversation.mode, Self::create_mode),
            (&self.view.join_toggle, Self::toggle_join),
            (&self.view.join.join_cancel, Self::toggle_join),
            (&self.view.join.join_submit, Self::join),
            (&self.view.appearance, Self::appearance),
            (&self.view.room_controls.accept, Self::accept_invitation),
            (&self.view.room_controls.decline, Self::decline_invitation),
            (&self.view.room_controls.leave, Self::leave_room),
            (&self.view.room_controls.invite, Self::invite_user),
            (&self.view.room_controls.confirm, Self::confirm_room_action),
            (&self.view.room_controls.cancel, Self::cancel_room_action),
        ] {
            let controller = self.clone();
            widget.on_action(move |_| {
                if !controller.rendering.get() {
                    action(&controller).expect("live CUI controls");
                }
            })?;
        }
        Ok(())
    }
}

fn fixture() -> Model {
    use archaic_matrix::{Message, RoomSummary};
    Model {
        phase: Phase::Connected, user: "@alex:example.org".into(),
        rooms: vec![RoomSummary { membership: archaic_matrix::Membership::Joined, inviter: None, id: "!design:example.org".into(), name: "Design studio".into() }, RoomSummary { membership: archaic_matrix::Membership::Joined, inviter: None, id: "!general:example.org".into(), name: "Archaic".into() }],
        selected: Some("!design:example.org".into()),
        messages: vec![Message { id: "1".into(), sender: "@maya:example.org".into(), body: Some("A place for thoughtful conversations.\nNative controls, a calm workspace, and room to focus.".into()), ..Message::default() }, Message { id: "2".into(), sender: "@alex:example.org".into(), body: Some("This is an explicit offline visual-test fixture.\nThe normal application starts at real Matrix sign-in.".into()), ..Message::default() }],
        ..Model::default()
    }
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let Some(options) = options::parse()? else {
        return Ok(());
    };
    let temporary = options.temporary || options.fixture_mode || options.smoke;
    let instance = if temporary {
        None
    } else if let Some(path) = appearance::preference_path() {
        match instance::launch(
            &path.with_file_name("desktop"),
            instance::Open {
                link: options.link.clone(),
                profile: options.explicit_profile.then(|| options.profile.clone()),
            },
        )? {
            instance::Launch::Forwarded => return Ok(()),
            instance::Launch::Owner(owner) => Some(owner),
        }
    } else {
        None
    };
    let preference_path = if temporary {
        None
    } else {
        appearance::preference_path()
    };
    let appearance = options.theme.unwrap_or_else(|| {
        preference_path
            .as_deref()
            .map(appearance::Appearance::load)
            .unwrap_or_default()
    });
    let app = Rc::new(App::new()?);
    app.theme(appearance.theme());
    let window = app.window("Archaic", 1180, 800)?;
    window.scrollable(true)?;
    let root = window.root()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let (sender, commands, events, receiver) = archaic_matrix::channels();
    let receiver = Rc::new(RefCell::new(Some(receiver)));
    let profile_name = options.profile.clone();
    let worker = runtime.spawn(async move {
        if temporary {
            archaic_matrix::run(commands, events).await;
        } else {
            archaic_matrix::run_persistent_named(commands, events, profile_name).await;
        }
    });
    let tr = Translator::new(&options.locale);
    let controller = Rc::new(Controller {
        view: View::new(&root, &tr)?,
        window: window.clone(),
        self_weak: RefCell::new(std::rc::Weak::new()),
        model: RefCell::new(if options.fixture_mode {
            fixture()
        } else {
            Model {
                temporary,
                phase: if temporary {
                    Phase::SignedOut
                } else {
                    Phase::Restoring
                },
                ..Model::default()
            }
        }),
        tr,
        sender,
        rendering: Cell::new(false),
        active_profile: RefCell::new(options.profile.clone()),
        startup_link: Cell::new(options.link.is_some()),
        status_item: Default::default(),
        status_item_attempted: Cell::new(false),
        desktop_commands: Default::default(),
        notification_sender: desktop::Notifications::new(),
        pending_profile: Default::default(),
        room_labels: RefCell::new(Vec::new()),
        fixture: options.fixture_mode,
        app: Rc::downgrade(&app),
        preference_path,
    });
    controller.view.profile.set_text(&options.profile)?;
    if let Some(link) = &options.link {
        controller.view.deep_link.set_text(link)?;
    }
    *controller.self_weak.borrow_mut() = Rc::downgrade(&controller);
    controller
        .view
        .appearance
        .set_selected(appearance.index())?;
    controller.bind()?;
    controller.install_shortcuts(&app)?;
    controller.render()?;
    let instance = Rc::new(RefCell::new(instance));
    let activation = instance.clone();
    #[cfg(target_os = "macos")]
    let (native_link_sender, native_links) = std::sync::mpsc::sync_channel(8);
    #[cfg(target_os = "macos")]
    let _apple_links = apple_links::AppleLinks::new(native_link_sender);
    let updates = controller.clone();
    let event_receiver = receiver.clone();
    // Bounded main-thread polling until CUI exposes a worker-safe dispatcher.
    let _pump = app.every(50, move || {
        updates.status_item_tick().expect("live desktop controls");
        if let Some(instance) = &*activation.borrow() {
            let ready = updates.desktop_ready();
            if ready && let Ok(open) = instance.incoming.try_recv() {
                updates.desktop_open(open).expect("live desktop controls");
            }
        }
        #[cfg(target_os = "macos")]
        if updates.desktop_ready()
            && let Ok(open) = native_links.try_recv()
        {
            updates.desktop_open(open).expect("live desktop controls");
        }
        for _ in 0..8 {
            let Ok(event) = event_receiver
                .borrow_mut()
                .as_mut()
                .expect("running receiver")
                .try_recv()
            else {
                break;
            };
            updates.event(event).expect("live CUI controls");
        }
    })?;
    let activity = controller.clone();
    let _activity = app.every(1000, move || {
        activity.activity_tick().expect("live controls");
    })?;
    let _smoke_timer = if options.smoke {
        Some(app.every(1800, move || {
            root.quit().expect("live root");
        })?)
    } else {
        None
    };
    window.show()?;
    let result = app.run();
    instance.borrow_mut().take();
    controller.status_item.borrow_mut().take();
    controller.notification_sender.invalidate();
    controller.remember_draft()?;
    let shutdown = runtime.block_on(async {
        let mut events = receiver.borrow_mut().take().expect("running receiver");
        tokio::time::timeout(std::time::Duration::from_secs(15), async {
            // Drain acknowledgements for all earlier commands before taking the final draft snapshot.
            // In particular a successful queue insertion must not reappear as a composer draft.
            let (reply, mut barrier) = tokio::sync::oneshot::channel();
            let request = controller.sender.send(Command::Barrier(reply));
            tokio::pin!(request);
            loop {
                tokio::select! {
                    result = &mut request => { result.map_err(|_| "shutdown-failed")?; break; },
                    Some(event) = events.recv() => controller.model.borrow_mut().apply(event),
                }
            }
            loop {
                tokio::select! {
                    result = &mut barrier => { result.map_err(|_| "shutdown-failed")?; break; },
                    Some(event) = events.recv() => controller.model.borrow_mut().apply(event),
                }
            }
            while let Ok(event) = events.try_recv() {
                controller.model.borrow_mut().apply(event);
            }
            let (reply, mut saved) = tokio::sync::oneshot::channel();
            let command = {
                let m = controller.model.borrow();
                Command::Shutdown {
                    epoch: m.epoch,
                    drafts: m.drafts.clone(),
                    result: reply,
                }
            };
            controller
                .sender
                .send(command)
                .await
                .map_err(|_| "shutdown-failed")?;
            loop {
                tokio::select! {
                    result = &mut saved => return result.map_err(|_| "shutdown-failed")?,
                    _ = events.recv() => {},
                }
            }
        })
        .await
        .map_err(|_| "shutdown-timeout")?
    });
    worker.abort();
    if let Err(key) = shutdown {
        eprintln!("{}", controller.tr.text(key));
    }
    runtime.shutdown_timeout(std::time::Duration::from_secs(1));
    result?;
    Ok(())
}

fn open_browser(url: &str) -> std::io::Result<()> {
    let parsed = url::Url::parse(url).map_err(std::io::Error::other)?;
    if !matches!(parsed.scheme(), "https" | "http") {
        return Err(std::io::Error::other("invalid browser URL"));
    }
    #[cfg(target_os = "linux")]
    let mut command = std::process::Command::new("xdg-open");
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut c = std::process::Command::new("rundll32");
        c.arg("url.dll,FileProtocolHandler");
        c
    };
    let mut child = command
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}
