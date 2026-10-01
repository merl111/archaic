use crate::login::Login;
use crate::model::{Model, Phase};
use crate::widgets::label;
use archaic_i18n::Translator;
use cui::{Result, Widget, sys};

pub struct View {
    sidebar_heading: Widget,
    pub ui: crate::daylight::Shell,
    pub emoji: crate::emoji_picker::Picker,
    pub reaction: crate::emoji_picker::Picker,
    pub thread_pane: crate::thread_pane::ThreadPane,
    dialogs: Dialogs,
    pub notifications: Widget,
    pub deep_link: Widget,
    pub open_link: Widget,
    pub profile: Widget,
    pub(crate) account_avatar: cui::ChatComponent,
    pub switch_profile: Widget,
    pub profiles: Widget,
    pub organization: crate::organization::Controls,
    pub organization_toggle: Widget,
    pub typing_opt: Widget,
    pub read_opt: Widget,
    pub typing_status: Widget,
    pub browser_login: Widget,
    pub oauth_login: Widget,
    pub cancel_login: Widget,
    pub outbox: crate::outbox::Controls,
    pub security_toggle: Widget,
    pub security: crate::security::Controls,
    pub thread: Widget,
    pub tools_toggle: Widget,
    pub tools: crate::tool_controls::ToolControls,
    pub appearance: Widget,
    pub appearance_error: Widget,
    pub new_toggle: Widget,
    pub new_conversation: crate::new_conversation::NewConversationForm,
    pub join_toggle: Widget,
    pub join: crate::join_form::JoinForm,
    pub compose_group: Widget,
    pub room_controls: crate::room_controls::RoomControls,
    pub message_controls: crate::message_controls::MessageControls,
    pub history_controls: crate::history_controls::HistoryControls,
    pub footer: Widget,
    pub session_notice: Widget,
    pub temporary: Widget,
    pub login_error: Widget,
    pub login_status: Widget,
    pub login: Widget,
    pub login_card: Widget,
    pub server: Widget,
    pub username: Widget,
    pub password: Widget,
    pub sign_in: Widget,
    pub workspace: Widget,
    pub rooms: Widget,
    pub search: Widget,
    pub title: Widget,
    pub room_id: Widget,
    pub timeline: crate::timeline_view::Timeline,
    pub composer: Widget,
    pub send: Widget,
    pub refresh: Widget,
    pub sign_out: Widget,
    pub reauth_group: Widget,
    pub reauth_password: Widget,
    pub reauth: Widget,
    pub reauth_sso: Widget,
    pub reauth_oauth: Widget,
    pub reauth_cancel: Widget,
    pub account: Widget,
    pub status: Widget,
    pub error: Widget,
}

struct Dialogs {
    preferences: Widget,
    security_host: Widget,
    organization_host: Widget,
    new_host: Widget,
    join_host: Widget,
    room_host: Widget,
    history_host: Widget,
    tools_host: Widget,
    message_host: Widget,
}

impl View {
    pub fn new(app: &cui::App, root: &Widget, tr: &Translator) -> Result<Self> {
        let ui = crate::daylight::Shell::new(root, tr)?;
        let t = ui.theme.borrow().clone();
        let reaction = crate::emoji_picker::Picker::popup(app, tr, &t)?;
        let emoji = crate::emoji_picker::Picker::new(&ui.layers, tr, &t)?;
        let thread_pane = crate::thread_pane::ThreadPane::new(&ui.body, tr, &t)?;
        let account_fields = AccountFields::new(&ui, tr)?;
        let preferences = account_fields.root.clone();
        let login = Login::new(&ui.login_host, tr)?;
        let security_host = ui.card(&ui.sheet_body, 0., 0)?;
        security_host.set_min_size(1, 440)?;
        let security = crate::security::Controls::new(&security_host, tr)?;
        let organization_host = ui.card(&ui.sheet_body, 0., 0)?;
        organization_host.set_min_size(1, 440)?;
        let organization = crate::organization::Controls::new(&organization_host, tr)?;
        let new_host = ui.card(&ui.sheet_body, 0., 0)?;
        new_host.expand(true)?;
        let new_conversation = crate::new_conversation::NewConversationForm::new(&new_host, tr)?;
        let join_host = ui.card(&ui.sheet_body, 0., 0)?;
        let join = crate::join_form::JoinForm::new(&join_host, tr)?;
        let sidebar_header = ui.sidebar.box_layout(sys::CUI_HORIZONTAL, 8)?;
        let home = label(
            &sidebar_header,
            &tr.text("daylight-home"),
            sys::CUI_ROLE_HEADING,
        )?;
        home.set_font("sans", 16.5, 700)?;
        home.expand(true)?;
        let new_toggle =
            sidebar_header.symbol_button(sys::CUI_SYMBOL_EDIT, &tr.text("new-conversation"))?;
        let join_toggle =
            sidebar_header.symbol_button(sys::CUI_SYMBOL_PLUS, &tr.text("join-room"))?;
        let search = ui.sidebar.search(&tr.text("room-search"))?;
        search.accessibility(&tr.text("room-search"), "")?;
        crate::daylight::style(&search, &t, t.selected, 12., 8)?;
        let organization_toggle = ui
            .top
            .symbol_button(sys::CUI_SYMBOL_PEOPLE, &tr.text("organization"))?;
        let security_toggle = account_fields.security_toggle.clone();
        let tools_toggle = ui
            .top
            .symbol_button(sys::CUI_SYMBOL_SEARCH, &tr.text("room-tools"))?;
        let refresh = ui
            .top
            .symbol_button(sys::CUI_SYMBOL_REPEAT, &tr.text("refresh"))?;
        tools_toggle.set_visible(false)?;
        refresh.set_visible(false)?;
        let room_host = ui.card(&ui.sheet_body, 0., 0)?;
        let title = label(&room_host, "", sys::CUI_ROLE_HEADING)?;
        let room_id = label(&room_host, "", sys::CUI_ROLE_CAPTION)?;
        let room_controls = crate::room_controls::RoomControls::new(&room_host, tr)?;
        let thread = ui.main.box_layout(sys::CUI_VERTICAL, 0)?;
        thread.expand(true)?;
        let timeline = crate::timeline_view::Timeline::new(&thread, tr, &t)?;
        let history_host = ui.card(&ui.sheet_body, 0., 0)?;
        let history_controls = crate::history_controls::HistoryControls::new(&history_host, tr)?;
        let outbox = crate::outbox::Controls::new(&thread, tr)?;
        let tools_host = ui.card(&ui.sheet_body, 0., 0)?;
        tools_host.set_min_size(1, 440)?;
        let tools = crate::tool_controls::ToolControls::new(&tools_host, tr)?;
        let message_host = ui.card(&ui.message_body, 0., 0)?;
        let message_controls = crate::message_controls::MessageControls::new(&message_host, tr)?;
        let compose_group = ui.composer.root.clone();
        let composer = ui.composer.part(0)?;
        composer.set_placeholder(&tr.text("composer"))?;
        composer.accessibility(&tr.text("composer"), "")?;
        let send = ui.composer.part(1)?;
        let mut presentation = ui.composer.presentation()?;
        presentation.composer_tools = (1 << 5) | (1 << 6);
        ui.composer.set_presentation(&presentation)?;
        ui.composer
            .part(5)?
            .accessibility(&tr.text("choose-file"), "")?;
        let typing_status = label(&thread, "", sys::CUI_ROLE_CAPTION)?;
        let error = label(&ui.main, "", sys::CUI_ROLE_DANGER)?;
        let footer = ui.main.box_layout(sys::CUI_HORIZONTAL, 8)?;
        footer.box_set_padding(6)?;
        let session_notice = label(&footer, "", sys::CUI_ROLE_CAPTION)?;
        session_notice.expand(true)?;
        let status = label(&footer, "", sys::CUI_ROLE_CAPTION)?;
        let rooms = ui.nav.root.clone();
        let workspace = ui.content.clone();
        Ok(Self {
            sidebar_heading: home,
            ui,
            thread_pane,
            emoji,
            reaction,
            dialogs: Dialogs {
                preferences,
                security_host,
                organization_host,
                new_host,
                join_host,
                room_host,
                history_host,
                tools_host,
                message_host,
            },
            notifications: account_fields.notifications,
            deep_link: account_fields.deep_link,
            open_link: account_fields.open_link,
            profile: account_fields.profile,
            account_avatar: account_fields.account_avatar,
            switch_profile: account_fields.switch_profile,
            profiles: account_fields.profiles,
            organization,
            organization_toggle,
            typing_opt: account_fields.typing_opt,
            read_opt: account_fields.read_opt,
            typing_status,
            browser_login: login.browser,
            oauth_login: login.oauth,
            cancel_login: login.cancel,
            outbox,
            security_toggle,
            security,
            thread,
            tools_toggle,
            tools,
            appearance: account_fields.appearance,
            appearance_error: account_fields.appearance_error,
            new_toggle,
            new_conversation,
            join_toggle,
            join,
            compose_group,
            room_controls,
            history_controls,
            message_controls,
            footer,
            session_notice,
            temporary: login.temporary,
            login_error: login.error,
            login_status: login.status,
            login: login.background,
            login_card: login.card,
            server: login.server,
            username: login.username,
            password: login.password,
            sign_in: login.submit,
            workspace,
            rooms,
            search,
            title,
            room_id,
            timeline,
            composer,
            send,
            refresh,
            sign_out: account_fields.sign_out,
            reauth_group: account_fields.reauth_group,
            reauth_password: account_fields.reauth_password,
            reauth: account_fields.reauth,
            reauth_sso: account_fields.reauth_sso,
            reauth_oauth: account_fields.reauth_oauth,
            reauth_cancel: account_fields.reauth_cancel,
            account: account_fields.account,
            status,
            error,
        })
    }

    fn render_login(&self, model: &Model, tr: &Translator, connected: bool) -> Result<()> {
        self.footer.set_visible(false)?;
        self.login.set_visible(!connected)?;
        self.ui.login_host.set_visible(!connected)?;
        self.oauth_login
            .set_visible(model.phase == Phase::SignedOut)?;
        self.browser_login
            .set_visible(model.phase == Phase::SignedOut)?;
        self.cancel_login
            .set_visible(model.browser_login && model.phase == Phase::Connecting)?;
        self.sign_in.set_enabled(matches!(
            model.phase,
            Phase::SignedOut | Phase::RestoreBlocked
        ))?;
        set_text(
            &self.sign_in,
            &tr.text(if model.phase == Phase::RestoreBlocked {
                "retry-session"
            } else {
                "sign-in"
            }),
        )?;
        self.temporary.set_visible(
            !model.temporary && matches!(model.phase, Phase::SignedOut | Phase::RestoreBlocked),
        )?;
        self.server.set_enabled(model.phase == Phase::SignedOut)?;
        self.username.set_enabled(model.phase == Phase::SignedOut)?;
        self.password.set_enabled(model.phase == Phase::SignedOut)?;
        set_text(
            &self.login_error,
            &model.error.map(|key| tr.text(key)).unwrap_or_default(),
        )?;
        self.login_error
            .set_visible(!connected && model.error.is_some())?;
        set_text(
            &self.login_status,
            &tr.text(if model.phase == Phase::Restoring {
                "restoring-session"
            } else if model.temporary && model.phase == Phase::SignedOut {
                "temporary-login-notice"
            } else {
                "signing-in"
            }),
        )?;
        self.login_status.set_visible(
            matches!(model.phase, Phase::Connecting | Phase::Restoring) || model.temporary,
        )?;
        Ok(())
    }

    fn render_status(&self, model: &Model, tr: &Translator, fixture: bool) -> Result<()> {
        set_text(
            &self.session_notice,
            &tr.text(if fixture {
                "preview-fixture"
            } else if model.session_saved {
                "session-saved"
            } else {
                "temporary-notice"
            }),
        )?;
        set_text(
            &self.status,
            &if model.status.is_empty() && !fixture {
                String::new()
            } else {
                tr.text(if fixture {
                    "preview-fixture"
                } else if model.status.is_empty() {
                    "preview"
                } else {
                    model.status
                })
            },
        )?;
        Ok(())
    }

    pub fn render(&self, model: &Model, tr: &Translator, fixture: bool) -> Result<()> {
        let heading = if model.direct_only {
            tr.text("direct-messages")
        } else {
            model
                .active_space
                .as_ref()
                .and_then(|id| model.rooms.iter().find(|r| &r.id == id))
                .map(|r| crate::daylight::text(&r.name, 80))
                .unwrap_or_else(|| tr.text("daylight-home"))
        };
        set_text(&self.sidebar_heading, &heading)?;
        self.account_avatar.rooms(&[cui::chat::NavItem {
            id: 1,
            title: model.user.clone(),
            avatar: cui::chat::Avatar {
                image: model.avatars.get("account").cloned().or_else(|| {
                    model
                        .selected
                        .as_ref()
                        .and_then(|r| model.avatars.get(&format!("user:{r}:{}", model.user)))
                        .cloned()
                }),
                ..crate::daylight::avatar(&model.user)
            },
            ..Default::default()
        }])?;
        self.account_avatar.set_theme(&self.ui.theme.borrow())?;
        self.organization.render(model, tr)?;
        self.reauth_group.set_visible(model.reauthentication)?;
        for button in [&self.reauth, &self.reauth_sso, &self.reauth_oauth] {
            button.set_enabled(!model.browser_login)?;
        }
        self.reauth_cancel.set_visible(model.browser_login)?;
        let connected = matches!(
            model.phase,
            Phase::Connected | Phase::SigningOut | Phase::LogoutCleanup
        );
        self.render_login(model, tr, connected)?;
        self.workspace.set_visible(connected)?;
        self.security_toggle.set_visible(connected)?;
        self.organization_toggle.set_visible(connected)?;
        self.security.render(model, tr, fixture)?;
        self.security_toggle.set_enabled(
            model.phase == Phase::Connected
                && !model.tools_busy()
                && model.message_draft.is_none()
                && !model.sending
                && !fixture,
        )?;
        self.new_conversation.render(model, tr, fixture)?;
        self.new_toggle
            .set_enabled(model.can_create() && !fixture)?;
        self.sign_out.set_visible(connected)?;
        self.sign_out.set_enabled(
            matches!(model.phase, Phase::Connected | Phase::LogoutCleanup) && !fixture,
        )?;
        set_text(&self.account, &model.user)?;
        self.render_status(model, tr, fixture)?;
        self.join_toggle.set_enabled(
            model.phase == Phase::Connected
                && !model.joining
                && model.message_draft.is_none()
                && !model.message_draft.as_ref().is_some_and(|d| d.pending)
                && !model.creating
                && model.room_pending.is_none()
                && !fixture,
        )?;
        self.join.render(model, tr, fixture)?;
        let error = model
            .error
            .or_else(|| model.message_draft.as_ref().and_then(|d| d.error));
        set_text(
            &self.error,
            &error.map(|key| tr.text(key)).unwrap_or_default(),
        )?;
        self.error.set_visible(connected && error.is_some())?;
        self.render_conversation(model, tr, fixture)?;
        self.ui.render(model, tr)?;
        self.render_dialogs(model, tr)?;
        self.render_picker(model)
    }

    fn render_picker(&self, model: &Model) -> Result<()> {
        let reaction_open =
            self.reaction.target.borrow().as_ref().is_some_and(|t| {
                t.epoch == model.epoch && model.selected.as_ref() == Some(&t.room)
            });
        self.reaction.root.set_visible(reaction_open)?;
        if !reaction_open {
            self.reaction.target.borrow_mut().take();
            self.reaction
                .window
                .as_ref()
                .expect("reaction popup")
                .close()?;
        }
        let picker_open =
            self.emoji.target.borrow().as_ref().is_some_and(|t| {
                t.epoch == model.epoch && model.selected.as_ref() == Some(&t.room)
            });
        if !picker_open {
            self.emoji.target.borrow_mut().take();
        }
        self.emoji.root.set_visible(picker_open)?;
        if picker_open {
            self.ui.base.set_enabled(false)?;
            self.ui.scrim.set_visible(true)?;
        }
        Ok(())
    }

    fn render_dialogs(&self, m: &Model, tr: &Translator) -> Result<()> {
        use crate::daylight::Panel;
        let security = m.security.show;
        let org = m.organization.show;
        let new = m.show_new;
        let join = m.show_join;
        let tools = m.tools.show;
        if security || org || new || join || tools || m.message_draft.is_some() {
            self.ui.panel.set(Panel::None);
        }
        let panel = self.ui.panel.get();
        if m.message_draft.is_none() {
            self.ui.quick_reaction.set(false);
        }
        let message = m.message_draft.as_ref().is_some_and(|d| {
            !d.kind.inline()
                && !(d.kind == crate::message_state::ActionKind::Reaction
                    && self.ui.quick_reaction.get()
                    && d.error.is_none())
                && !(d.kind == crate::message_state::ActionKind::Thread
                    && self.thread_pane.open.get())
        });
        let dialogs = [
            (security, &self.dialogs.security_host, "security-title"),
            (org, &self.dialogs.organization_host, "organization"),
            (new, &self.dialogs.new_host, "new-conversation"),
            (join, &self.dialogs.join_host, "join-room"),
            (
                tools,
                &self.dialogs.tools_host,
                ["tool-files", "tool-search", "tool-details", "tool-receipts"]
                    [m.tools.mode.clamp(0, 3) as usize],
            ),
            (
                message,
                &self.dialogs.message_host,
                m.message_draft
                    .as_ref()
                    .map(|d| d.kind.key())
                    .unwrap_or("daylight-message-actions"),
            ),
            (
                panel == Panel::Account || m.reauthentication,
                &self.dialogs.preferences,
                "settings-title",
            ),
            (panel == Panel::Room, &self.dialogs.room_host, "room-tools"),
            (
                panel == Panel::History,
                &self.dialogs.history_host,
                "timeline-messages",
            ),
        ];
        // Async updates can leave several model workflows open. Keep one focused
        // dialog visible; its host and title must always describe the same task.
        let active = dialogs.iter().position(|(show, _, _)| *show);
        for (index, (_, widget, _)) in dialogs.iter().enumerate() {
            widget.set_visible(active == Some(index))?;
        }
        self.ui.modal(
            active.is_some(),
            &active.map(|i| tr.text(dialogs[i].2)).unwrap_or_default(),
            active == Some(5),
        )
    }

    fn render_inline_composer(&self, m: &Model, tr: &Translator) -> Result<()> {
        let draft = m.message_draft.as_ref().filter(|d| d.kind.inline());
        self.ui
            .composer
            .busy(m.sending || draft.is_some_and(|d| d.pending))?;
        if let Some(d) = draft {
            let original = m.selected_message();
            self.ui.composer.context(
                1,
                &crate::daylight::text(original.map(|v| v.sender.as_str()).unwrap_or(""), 1024),
                &crate::daylight::text(
                    original.and_then(|v| v.body.as_deref()).unwrap_or(""),
                    4096,
                ),
                d.kind == crate::message_state::ActionKind::Edit,
            )?;
            set_text(&self.composer, &d.body)?;
        } else {
            self.ui.composer.context(0, "", "", false)?;
            set_text(&self.composer, m.draft())?;
        }
        self.ui
            .composer
            .part(4)?
            .accessibility(&tr.text("cancel"), "")?;
        Ok(())
    }

    fn render_conversation(&self, model: &Model, tr: &Translator, fixture: bool) -> Result<()> {
        let selected = model
            .selected
            .as_ref()
            .and_then(|id| model.rooms.iter().find(|room| &room.id == id));
        set_text(
            &self.title,
            &selected
                .map(|room| room.name.clone())
                .unwrap_or_else(|| tr.text("choose-room")),
        )?;
        set_text(
            &self.room_id,
            &selected.map(|room| room.id.clone()).unwrap_or_default(),
        )?;
        let joined =
            selected.is_some_and(|room| room.membership == archaic_matrix::Membership::Joined);
        self.room_controls.render(model, tr, fixture)?;
        self.history_controls.render(model, tr, fixture)?;
        self.outbox.render(model, tr, fixture)?;
        self.thread.set_visible(true)?;
        self.session_notice
            .set_tooltip(&tr.text("preview-fixture"))?;
        self.tools.render(model, tr, fixture)?;
        self.tools_toggle
            .set_enabled(model.can_tool() && !fixture)?;
        self.message_controls.render(model, tr, fixture)?;
        self.compose_group.set_visible(joined)?;
        self.timeline.render(model, tr)?;
        self.thread_pane.render(model, tr, fixture)?;
        self.render_inline_composer(model, tr)?;
        self.composer.set_enabled(
            joined
                && model.phase == Phase::Connected
                && !fixture
                && !model.tools_busy()
                && !model.message_draft.as_ref().is_some_and(|d| d.pending)
                && !model.creating
                && model.room_pending.is_none()
                && model.room_confirmation.is_none(),
        )?;
        self.send.set_enabled(
            joined
                && !model.sending
                && !fixture
                && model.phase == Phase::Connected
                && !model.message_draft.as_ref().is_some_and(|d| d.pending)
                && !model.creating
                && model.room_pending.is_none()
                && model.room_confirmation.is_none(),
        )?;
        set_text(
            &self.send,
            &tr.text(if model.sending { "sending" } else { "send" }),
        )?;
        self.refresh
            .set_enabled(model.phase == Phase::Connected && !fixture)?;
        self.rooms.set_enabled(
            !model.tools_busy()
                && model.message_draft.is_none()
                && model.phase == Phase::Connected
                && !model.joining
                && !model.creating,
        )?;
        self.search.set_enabled(model.phase == Phase::Connected)?;
        Ok(())
    }
}

// Matrix event bodies may contain NULs. CUI uses C strings; replace them at the view boundary.
pub fn set_text(widget: &Widget, text: &str) -> Result<()> {
    let text = text.replace('\0', "�");
    if widget.text()? != text {
        widget.set_text(&text)?;
    }
    Ok(())
}

pub(crate) fn message_text(
    message: &archaic_matrix::Message,
    model: &Model,
    tr: &Translator,
) -> String {
    let mut text = message.sender.clone();
    if message.thread_root.is_some() {
        text.push_str(&format!(" · {}", tr.text("thread-reply")));
    }
    if message.edited {
        text.push_str(&format!(" · {}", tr.text("message-edited")));
    }
    if let Some(target) = &message.reply_to {
        let preview = model
            .messages
            .iter()
            .find(|m| &m.id == target)
            .and_then(|m| m.body.as_deref())
            .map(|body| body.chars().take(80).collect::<String>())
            .unwrap_or_else(|| target.clone());
        text.push_str(&format!("\n↳ {}: {}", tr.text("message-reply"), preview));
    }
    text.push_str(&format!(
        "\n{}",
        if message.deleted {
            tr.text("message-deleted")
        } else {
            message.body.clone().unwrap_or_else(|| {
                tr.text(if message.undecrypted {
                    "encrypted-message"
                } else {
                    "unavailable-message"
                })
            })
        }
    ));
    if let Some(file) = &message.attachment {
        text.push_str(&format!(
            "\n📎 {} · {}",
            file.name,
            file.size.map(|n| format!("{n} B")).unwrap_or_default()
        ));
    }
    let mut reactions = std::collections::BTreeMap::<&str, std::collections::BTreeSet<&str>>::new();
    for reaction in &message.reactions {
        reactions
            .entry(&reaction.key)
            .or_default()
            .insert(&reaction.sender);
    }
    for (key, senders) in reactions {
        text.push_str(&format!(
            "  [{} {}{}]",
            key,
            senders.len(),
            if senders.contains(model.user.as_str()) {
                " ✓"
            } else {
                ""
            }
        ));
    }
    text
}

struct AccountFields {
    account: Widget,
    root: Widget,
    account_avatar: cui::ChatComponent,
    security_toggle: Widget,
    profile: Widget,
    profiles: Widget,
    switch_profile: Widget,
    appearance: Widget,
    appearance_error: Widget,
    notifications: Widget,
    typing_opt: Widget,
    read_opt: Widget,
    deep_link: Widget,
    open_link: Widget,
    sign_out: Widget,
    reauth_group: Widget,
    reauth_password: Widget,
    reauth: Widget,
    reauth_sso: Widget,
    reauth_oauth: Widget,
    reauth_cancel: Widget,
}
impl AccountFields {
    fn new(ui: &crate::daylight::Shell, tr: &Translator) -> Result<Self> {
        let root = ui.card(&ui.sheet_body, 0., 0)?;
        let tabs = root.tabs()?;
        tabs.set_min_size(1, 360)?;
        let preferences = tabs.tab_add(&tr.text("settings-account"))?;
        let appearance_page = tabs.tab_add(&tr.text("appearance"))?;
        let notification_page = tabs.tab_add(&tr.text("settings-notifications"))?;
        let security_page = tabs.tab_add(&tr.text("security-title"))?;
        let advanced = tabs.tab_add(&tr.text("settings-advanced"))?;
        for page in [
            &preferences,
            &appearance_page,
            &notification_page,
            &security_page,
            &advanced,
        ] {
            page.box_set_padding(16)?;
        }
        let security_toggle = security_page.button(&tr.text("security-title"))?;
        label(
            &security_page,
            &tr.text("security-settings-description"),
            sys::CUI_ROLE_BODY,
        )?;
        label(
            &preferences,
            &tr.text("signed-in-as"),
            sys::CUI_ROLE_CAPTION,
        )?;
        let account_avatar =
            cui::ChatComponent::create(&preferences, cui::ChatKind::Avatar, &ui.theme.borrow())?;
        account_avatar.root.expand(false)?;
        account_avatar.part(0)?.set_min_size(80, 100)?;
        let account = label(&preferences, "", sys::CUI_ROLE_BODY)?;
        let profile_row = preferences.box_layout(sys::CUI_HORIZONTAL, 12)?;
        let profile = profile_row.entry("default")?;
        profile.expand(true)?;
        profile.accessibility(&tr.text("profile-name"), "")?;
        let profiles = profile_row.select(&[])?;
        profiles.expand(true)?;
        profiles.accessibility(&tr.text("profile-saved"), "")?;
        let switch_profile = profile_row.button(&tr.text("profile-switch"))?;
        label(
            &appearance_page,
            &tr.text("appearance"),
            sys::CUI_ROLE_CAPTION,
        )?;
        let appearance = appearance_page.select(&[
            &tr.text("theme-system"),
            &tr.text("theme-light"),
            &tr.text("theme-dark"),
        ])?;
        appearance.accessibility(&tr.text("appearance"), "")?;
        let appearance_error = label(
            &appearance_page,
            &tr.text("appearance-save-failed"),
            sys::CUI_ROLE_DANGER,
        )?;
        appearance_error.set_visible(false)?;
        let notifications = notification_page.checkbox(&tr.text("desktop-notifications"), false)?;
        let typing_opt = notification_page.checkbox(&tr.text("activity-share-typing"), false)?;
        let read_opt = notification_page.checkbox(&tr.text("activity-private-read"), false)?;
        let deep_link = advanced.entry("")?;
        deep_link.set_placeholder(&tr.text("matrix-link"))?;
        deep_link.accessibility(&tr.text("matrix-link"), "")?;
        let open_link = advanced.button(&tr.text("open-link"))?;
        let sign_out = preferences.button(&tr.text("sign-out"))?;
        let reauth_group = preferences.box_layout(sys::CUI_VERTICAL, 6)?;
        let reauth_password = reauth_group.password("")?;
        reauth_password.accessibility(&tr.text("password"), "")?;
        let reauth = reauth_group.button(&tr.text("reauthenticate"))?;
        let reauth_sso = reauth_group.button(&tr.text("sso-login"))?;
        let reauth_oauth = reauth_group.button(&tr.text("oauth-login"))?;
        let reauth_cancel = reauth_group.button(&tr.text("cancel"))?;
        Ok(Self {
            root,
            account_avatar,
            security_toggle,
            account,
            profile,
            profiles,
            switch_profile,
            appearance,
            appearance_error,
            notifications,
            typing_opt,
            read_opt,
            deep_link,
            open_link,
            sign_out,
            reauth_group,
            reauth_password,
            reauth,
            reauth_sso,
            reauth_oauth,
            reauth_cancel,
        })
    }
}
