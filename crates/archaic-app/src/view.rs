use crate::login::Login;
use crate::model::{Model, Phase};
use crate::widgets::label;
use archaic_i18n::Translator;
use cui::{Result, Widget, sys};

pub struct View {
    pub appearance: Widget,
    pub appearance_error: Widget,
    pub join_toggle: Widget,
    pub join: crate::join_form::JoinForm,
    pub compose_group: Widget,
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
    pub timeline: Widget,
    pub composer: Widget,
    pub send: Widget,
    pub refresh: Widget,
    pub sign_out: Widget,
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

        let workspace = root.split(sys::CUI_HORIZONTAL, 0.26)?;
        workspace.expand(true)?;
        let sidebar = workspace.split_pane(0)?;
        sidebar.box_set_padding(16)?;
        sidebar.set_min_size(240, 260)?;
        sidebar.set_role(sys::CUI_ROLE_PANEL)?;
        let sidebar_header = sidebar.box_layout(sys::CUI_HORIZONTAL, 12)?;
        label(&sidebar_header, &tr.text("rooms"), sys::CUI_ROLE_HEADING)?.expand(true)?;
        let join_toggle =
            sidebar_header.symbol_button(sys::CUI_SYMBOL_PLUS, &tr.text("join-room"))?;
        join_toggle.set_tooltip(&tr.text("join-room"))?;
        join_toggle.accessibility(&tr.text("join-room"), "")?;
        let join = crate::join_form::JoinForm::new(&sidebar, tr)?;
        let search = sidebar.search(&tr.text("room-search"))?;
        search.accessibility(&tr.text("room-search"), "")?;
        let rooms = sidebar.list(&[])?;
        rooms.expand(true)?;
        rooms.accessibility(&tr.text("rooms"), "")?;
        sidebar.separator()?;
        label(&sidebar, &tr.text("signed-in-as"), sys::CUI_ROLE_CAPTION)?;
        let account = label(&sidebar, "", sys::CUI_ROLE_CAPTION)?;
        let sign_out = sidebar.button(&tr.text("sign-out"))?;
        let conversation = workspace.split_pane(1)?;
        conversation.box_set_padding(20)?;
        let room_header = conversation.box_layout(sys::CUI_HORIZONTAL, 12)?;
        let title = label(&room_header, &tr.text("choose-room"), sys::CUI_ROLE_HEADING)?;
        title.expand(true)?;
        let refresh = room_header.symbol_button(sys::CUI_SYMBOL_REPEAT, &tr.text("refresh"))?;
        let room_id = label(&conversation, "", sys::CUI_ROLE_CAPTION)?;
        conversation.separator()?;
        let timeline = conversation.textarea(&tr.text("choose-description"))?;
        timeline.expand(true)?;
        timeline.set_read_only(true)?;
        timeline.accessibility(&tr.text("recent-only"), "")?;
        let compose_group = conversation.box_layout(sys::CUI_VERTICAL, 10)?;
        compose_group.box_set_padding(12)?;
        compose_group.set_role(sys::CUI_ROLE_CARD)?;
        let composer = compose_group.textarea("")?;
        composer.set_min_size(300, 90)?;
        composer.set_placeholder(&tr.text("composer"))?;
        composer.accessibility(&tr.text("composer"), "")?;
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
            appearance,
            appearance_error,
            join_toggle,
            join,
            compose_group,
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
            account,
            status,
            error,
        })
    }

    fn render_login(&self, model: &Model, tr: &Translator, connected: bool) -> Result<()> {
        self.footer.set_visible(connected)?;
        self.login.set_visible(!connected)?;
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
        let connected = matches!(
            model.phase,
            Phase::Connected | Phase::SigningOut | Phase::LogoutCleanup
        );
        self.render_login(model, tr, connected)?;
        self.workspace.set_visible(connected)?;
        self.sign_out.set_visible(connected)?;
        self.sign_out.set_enabled(
            matches!(model.phase, Phase::Connected | Phase::LogoutCleanup) && !fixture,
        )?;
        set_text(&self.account, &model.user)?;
        self.render_status(model, tr, fixture)?;
        self.join_toggle
            .set_enabled(model.phase == Phase::Connected && !model.joining && !fixture)?;
        self.join.render(model, tr, fixture)?;
        set_text(
            &self.error,
            &model.error.map(|key| tr.text(key)).unwrap_or_default(),
        )?;
        self.error.set_visible(connected && model.error.is_some())?;
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
        self.compose_group.set_visible(selected.is_some())?;
        let history = history_text(model, tr, selected.is_some());
        set_text(&self.timeline, &history)?;
        set_text(&self.composer, model.draft())?;
        self.composer
            .set_enabled(selected.is_some() && model.phase == Phase::Connected && !fixture)?;
        self.send.set_enabled(
            selected.is_some() && !model.sending && !fixture && model.phase == Phase::Connected,
        )?;
        set_text(
            &self.send,
            &tr.text(if model.sending { "sending" } else { "send" }),
        )?;
        self.refresh
            .set_enabled(model.phase == Phase::Connected && !fixture)?;
        self.rooms
            .set_enabled(model.phase == Phase::Connected && !model.joining)?;
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

fn history_text(model: &Model, tr: &Translator, selected: bool) -> String {
    if !selected {
        tr.text(if model.rooms.is_empty() {
            "no-rooms"
        } else {
            "choose-description"
        })
    } else if model.messages.is_empty() {
        tr.text(if model.status == "loading" {
            "loading"
        } else {
            "empty-history"
        })
    } else {
        model
            .messages
            .iter()
            .map(|message| {
                format!(
                    "{}\n{}",
                    message.sender,
                    message
                        .body
                        .clone()
                        .unwrap_or_else(|| tr.text("unavailable-message"))
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}
