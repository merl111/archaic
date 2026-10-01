use crate::model::{Model, Phase};
use archaic_i18n::Translator;
use cui::{Result, Widget, sys};

pub struct View {
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

fn label(parent: &Widget, text: &str, role: i32) -> Result<Widget> {
    let widget = parent.label(text)?;
    widget.set_role(role)?;
    Ok(widget)
}
fn input(parent: &Widget, tr: &Translator, key: &str, secret: bool) -> Result<Widget> {
    label(parent, &tr.text(key), sys::CUI_ROLE_CAPTION)?;
    let widget = if secret {
        parent.password("")?
    } else {
        parent.entry("")?
    };
    widget.accessibility(&tr.text(key), "")?;
    Ok(widget)
}

impl View {
    pub fn new(root: &Widget, tr: &Translator) -> Result<Self> {
        root.box_set_padding(24)?;
        let header = root.box_layout(sys::CUI_HORIZONTAL, 20)?;
        label(&header, "Archaic", sys::CUI_ROLE_TITLE)?;
        let subtitle = label(&header, &tr.text("app-tagline"), sys::CUI_ROLE_CAPTION)?;
        subtitle.expand(true)?;
        let account = label(&header, "", sys::CUI_ROLE_CAPTION)?;
        let sign_out = header.button(&tr.text("sign-out"))?;
        root.separator()?;

        let login = root.box_layout(sys::CUI_VERTICAL, 14)?;
        login.box_set_padding(28)?;
        label(&login, &tr.text("welcome"), sys::CUI_ROLE_TITLE)?;
        label(&login, &tr.text("login-description"), sys::CUI_ROLE_BODY)?;
        let server = input(&login, tr, "homeserver", false)?;
        server.set_placeholder("https://matrix.example.org")?;
        let username = input(&login, tr, "username", false)?;
        username.set_placeholder("@name:example.org")?;
        let password = input(&login, tr, "password", true)?;
        let sign_in = login.button(&tr.text("sign-in"))?;
        sign_in.set_role(sys::CUI_ROLE_PRIMARY)?;
        label(&login, &tr.text("privacy-note"), sys::CUI_ROLE_CAPTION)?;
        label(&login, &tr.text("privacy-detail"), sys::CUI_ROLE_CAPTION)?;

        let workspace = root.split(sys::CUI_HORIZONTAL, 0.26)?;
        workspace.expand(true)?;
        let sidebar = workspace.split_pane(0)?;
        sidebar.box_set_padding(16)?;
        sidebar.set_min_size(240, 260)?;
        sidebar.set_role(sys::CUI_ROLE_PANEL)?;
        label(&sidebar, &tr.text("rooms"), sys::CUI_ROLE_HEADING)?;
        let search = sidebar.search(&tr.text("room-search"))?;
        search.accessibility(&tr.text("room-search"), "")?;
        let rooms = sidebar.list(&[])?;
        rooms.expand(true)?;
        rooms.accessibility(&tr.text("rooms"), "")?;
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
        label(&conversation, &tr.text("composer"), sys::CUI_ROLE_CAPTION)?;
        let composer = conversation.textarea("")?;
        composer.set_min_size(300, 90)?;
        composer.set_placeholder(&tr.text("composer"))?;
        composer.accessibility(&tr.text("composer"), "")?;
        let composer_footer = conversation.box_layout(sys::CUI_HORIZONTAL, 12)?;
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
        label(&footer, &tr.text("preview"), sys::CUI_ROLE_CAPTION)?.expand(true)?;
        let status = label(&footer, "", sys::CUI_ROLE_CAPTION)?;
        Ok(Self {
            login,
            server,
            username,
            password,
            sign_in,
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

    pub fn render(&self, model: &Model, tr: &Translator, fixture: bool) -> Result<()> {
        let connected = matches!(model.phase, Phase::Connected | Phase::SigningOut);
        self.login.set_visible(!connected)?;
        self.workspace.set_visible(connected)?;
        self.sign_out.set_visible(connected)?;
        self.sign_out
            .set_enabled(model.phase == Phase::Connected && !fixture)?;
        self.sign_in.set_enabled(model.phase == Phase::SignedOut)?;
        self.server.set_enabled(model.phase == Phase::SignedOut)?;
        self.username.set_enabled(model.phase == Phase::SignedOut)?;
        self.password.set_enabled(model.phase == Phase::SignedOut)?;
        set_text(&self.account, &model.user)?;
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
        set_text(
            &self.error,
            &model.error.map(|key| tr.text(key)).unwrap_or_default(),
        )?;
        self.error.set_visible(model.error.is_some())?;
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
        self.refresh.set_enabled(connected && !fixture)?;
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
