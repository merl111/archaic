use crate::login::Login;
use crate::model::{Model, Phase};
use crate::widgets::label;
use archaic_i18n::Translator;
use cui::{Result, Widget, sys};

pub struct View {
    pub notifications: Widget,
    pub deep_link: Widget,
    pub open_link: Widget,
    pub profile: Widget,
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

impl View {
    pub fn new(root: &Widget, tr: &Translator) -> Result<Self> {
        root.box_set_padding(0)?;
        let header_group = root.box_layout(sys::CUI_VERTICAL, 0)?;
        let header = header_group.box_layout(sys::CUI_HORIZONTAL, 20)?;
        header.box_set_padding(12)?;
        label(&header, "Archaic", sys::CUI_ROLE_HEADING)?;
        let subtitle = label(&header, &tr.text("app-tagline"), sys::CUI_ROLE_CAPTION)?;
        subtitle.expand(true)?;
        let profile = header.entry("default")?;
        profile.set_min_size(90, 0)?;
        profile.accessibility(&tr.text("profile-name"), "")?;
        let profiles = header.select(&[])?;
        profiles.accessibility(&tr.text("profile-saved"), "")?;
        let switch_profile = header.button(&tr.text("profile-switch"))?;
        label(&header, &tr.text("appearance"), sys::CUI_ROLE_CAPTION)?;
        let appearance = header.select(&[
            &tr.text("theme-system"),
            &tr.text("theme-light"),
            &tr.text("theme-dark"),
        ])?;
        appearance.accessibility(&tr.text("appearance"), "")?;
        let appearance_error = label(
            &header_group,
            &tr.text("appearance-save-failed"),
            sys::CUI_ROLE_DANGER,
        )?;
        appearance_error.set_visible(false)?;
        header_group.separator()?;
        let login = Login::new(root, tr)?;

        let security = crate::security::Controls::new(root, tr)?;
        let organization = crate::organization::Controls::new(root, tr)?;
        let new_conversation = crate::new_conversation::NewConversationForm::new(root, tr)?;
        let workspace = root.split(sys::CUI_HORIZONTAL, 0.26)?;
        workspace.expand(true)?;
        let sidebar = workspace.split_pane(0)?;
        sidebar.box_set_padding(16)?;
        sidebar.set_min_size(240, 260)?;
        sidebar.set_role(sys::CUI_ROLE_PANEL)?;
        let sidebar_header = sidebar.box_layout(sys::CUI_HORIZONTAL, 12)?;
        label(&sidebar_header, &tr.text("rooms"), sys::CUI_ROLE_HEADING)?.expand(true)?;
        let new_toggle = sidebar_header.button(&tr.text("new-short"))?;
        new_toggle.accessibility(&tr.text("new-conversation"), "")?;
        new_toggle.set_tooltip(&tr.text("new-conversation"))?;
        let join_toggle =
            sidebar_header.symbol_button(sys::CUI_SYMBOL_PLUS, &tr.text("join-room"))?;
        join_toggle.set_tooltip(&tr.text("join-room"))?;
        join_toggle.accessibility(&tr.text("join-room"), "")?;
        let join = crate::join_form::JoinForm::new(&sidebar, tr)?;
        let deep_link = sidebar.entry("")?;
        deep_link.set_placeholder(&tr.text("matrix-link"))?;
        deep_link.accessibility(&tr.text("matrix-link"), "")?;
        let open_link = sidebar.button(&tr.text("open-link"))?;
        let notifications = sidebar.checkbox(&tr.text("desktop-notifications"), false)?;
        let search = sidebar.search(&tr.text("room-search"))?;
        search.accessibility(&tr.text("room-search"), "")?;
        let rooms = sidebar.list(&[])?;
        rooms.expand(true)?;
        rooms.accessibility(&tr.text("rooms"), "")?;
        sidebar.separator()?;
        label(&sidebar, &tr.text("signed-in-as"), sys::CUI_ROLE_CAPTION)?;
        let account = label(&sidebar, "", sys::CUI_ROLE_CAPTION)?;
        let organization_toggle = sidebar.button(&tr.text("organization"))?;
        let security_toggle = sidebar.button(&tr.text("security-title"))?;
        let reauth_group = sidebar.box_layout(sys::CUI_VERTICAL, 6)?;
        let reauth_password = reauth_group.password("")?;
        reauth_password.accessibility(&tr.text("password"), "")?;
        let reauth = reauth_group.button(&tr.text("reauthenticate"))?;
        let reauth_sso = reauth_group.button(&tr.text("sso-login"))?;
        let reauth_oauth = reauth_group.button(&tr.text("oauth-login"))?;
        let reauth_cancel = reauth_group.button(&tr.text("cancel"))?;
        let sign_out = sidebar.button(&tr.text("sign-out"))?;
        let conversation = workspace.split_pane(1)?;
        conversation.box_set_padding(20)?;
        let room_header = conversation.box_layout(sys::CUI_HORIZONTAL, 12)?;
        let title = label(&room_header, &tr.text("choose-room"), sys::CUI_ROLE_HEADING)?;
        title.expand(true)?;
        let tools_toggle = room_header.button(&tr.text("room-tools"))?;
        let refresh = room_header.symbol_button(sys::CUI_SYMBOL_REPEAT, &tr.text("refresh"))?;
        let room_id = label(&conversation, "", sys::CUI_ROLE_CAPTION)?;
        let room_controls = crate::room_controls::RoomControls::new(&conversation, tr)?;
        conversation.separator()?;
        let thread = conversation.box_layout(sys::CUI_VERTICAL, 8)?;
        thread.expand(true)?;
        let tools = crate::tool_controls::ToolControls::new(&conversation, tr)?;
        let history_controls = crate::history_controls::HistoryControls::new(&thread, tr)?;
        let timeline = crate::timeline_view::Timeline::new(&thread, tr)?;
        let outbox = crate::outbox::Controls::new(&thread, tr)?;
        let message_controls = crate::message_controls::MessageControls::new(&conversation, tr)?;
        let compose_group = conversation.box_layout(sys::CUI_VERTICAL, 10)?;
        compose_group.box_set_padding(12)?;
        compose_group.set_role(sys::CUI_ROLE_CARD)?;
        let composer = compose_group.textarea("")?;
        composer.set_min_size(300, 90)?;
        composer.set_placeholder(&tr.text("composer"))?;
        composer.accessibility(&tr.text("composer"), "")?;
        let typing_status = label(&compose_group, "", sys::CUI_ROLE_CAPTION)?;
        let privacy = compose_group.box_layout(sys::CUI_HORIZONTAL, 12)?;
        let typing_opt = privacy.checkbox(&tr.text("activity-share-typing"), false)?;
        let read_opt = privacy.checkbox(&tr.text("activity-private-read"), false)?;
        let composer_footer = compose_group.box_layout(sys::CUI_HORIZONTAL, 12)?;
        let hint = label(
            &composer_footer,
            &tr.text("recent-only"),
            sys::CUI_ROLE_CAPTION,
        )?;
        hint.expand(true)?;
        let send = composer_footer.symbol_button(sys::CUI_SYMBOL_SEND, &tr.text("send"))?;
        send.set_icon_only(false)?;
        send.set_role(sys::CUI_ROLE_PRIMARY)?;
        let error = label(root, "", sys::CUI_ROLE_DANGER)?;
        let footer = root.box_layout(sys::CUI_HORIZONTAL, 12)?;
        footer.box_set_padding(16)?;
        let session_notice = label(&footer, &tr.text("preview"), sys::CUI_ROLE_CAPTION)?;
        session_notice.expand(true)?;
        let status = label(&footer, "", sys::CUI_ROLE_CAPTION)?;
        Ok(Self {
            notifications,
            deep_link,
            open_link,
            profile,
            switch_profile,
            profiles,
            organization,
            organization_toggle,
            typing_opt,
            read_opt,
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
            appearance,
            appearance_error,
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
            sign_out,
            reauth_group,
            reauth_password,
            reauth,
            reauth_sso,
            reauth_oauth,
            reauth_cancel,
            account,
            status,
            error,
        })
    }

    fn render_login(&self, model: &Model, tr: &Translator, connected: bool) -> Result<()> {
        self.footer.set_visible(connected)?;
        self.login.set_visible(!connected)?;
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
        self.workspace.set_visible(
            connected
                && !((model.security.show || model.organization.show)
                    && model.phase == Phase::Connected)
                && !(model.show_new && model.phase == Phase::Connected),
        )?;
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
                && !model.creating
                && model.room_pending.is_none()
                && !fixture,
        )?;
        self.join.render(model, tr, fixture)?;
        set_text(
            &self.error,
            &model.error.map(|key| tr.text(key)).unwrap_or_default(),
        )?;
        self.error.set_visible(connected && model.error.is_some())?;
        self.render_conversation(model, tr, fixture)
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
        self.thread.set_visible(!model.tools.show)?;
        self.tools.render(model, tr, fixture)?;
        self.tools_toggle
            .set_enabled(model.can_tool() && !fixture)?;
        self.message_controls.render(model, tr, fixture)?;
        self.compose_group
            .set_visible(joined && model.message_draft.is_none() && !model.tools.show)?;
        self.timeline.render(model, tr)?;
        set_text(&self.composer, model.draft())?;
        self.composer.set_enabled(
            joined
                && model.phase == Phase::Connected
                && !fixture
                && !model.tools_busy()
                && !model.creating
                && model.room_pending.is_none()
                && model.room_confirmation.is_none(),
        )?;
        self.send.set_enabled(
            joined
                && !model.sending
                && !fixture
                && model.phase == Phase::Connected
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
            message
                .body
                .clone()
                .unwrap_or_else(|| tr.text("unavailable-message"))
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
