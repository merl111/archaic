//! Native, centered sign-in card. Expanding spacers keep its intrinsic width.
use crate::widgets::label;
use archaic_i18n::Translator;
use cui::{Result, Widget, sys};

pub struct Login {
    pub card: Widget,
    pub form: Widget,
    pub notes: Widget,
    pub browser: Widget,
    pub oauth: Widget,
    pub cancel: Widget,
    pub background: Widget,
    pub server: Widget,
    pub username: Widget,
    pub password: Widget,
    pub submit: Widget,
    pub error: Widget,
    pub temporary: Widget,
    pub status: Widget,
}

fn field(parent: &Widget, tr: &Translator, key: &str, secret: bool) -> Result<Widget> {
    let group = parent.box_layout(sys::CUI_VERTICAL, 6)?;
    label(&group, &tr.text(key), sys::CUI_ROLE_CAPTION)?;
    let input = if secret {
        group.password("")?
    } else {
        group.entry("")?
    };
    input.set_min_size(310, 0)?;
    input.accessibility(&tr.text(key), "")?;
    Ok(input)
}

impl Login {
    pub fn new(root: &Widget, tr: &Translator) -> Result<Self> {
        let background = root.box_layout(sys::CUI_VERTICAL, 0)?;
        let theme = cui::chat::Theme::new(cui::chat::Appearance::Daylight);
        crate::daylight::style(&background, &theme, theme.background, 0., 0)?;
        background.box_set_padding(40)?;
        background.expand(true)?;
        background.box_layout(sys::CUI_VERTICAL, 0)?.expand(true)?;
        let row = background.box_layout(sys::CUI_HORIZONTAL, 0)?;
        row.box_layout(sys::CUI_HORIZONTAL, 0)?.expand(true)?;
        let card = row.box_layout(sys::CUI_VERTICAL, 12)?;
        card.box_set_padding(32)?;
        crate::daylight::style(&card, &theme, theme.surface, 24., 32)?;
        let brand = card.box_layout(sys::CUI_VERTICAL, 6)?;
        let identity = brand.box_layout(sys::CUI_HORIZONTAL, 10)?;
        let logo = identity.icon(Some(&crate::branding::mark()?))?;
        logo.set_icon_size(44)?;
        logo.accessibility("Archaic", "")?;
        let title = identity.label("archaic")?;
        title.set_font("sans", 24., 800)?;
        label(&brand, &tr.text("app-tagline"), sys::CUI_ROLE_CAPTION)?;
        let form = card.box_layout(sys::CUI_VERTICAL, 12)?;
        let welcome = form.label(&tr.text("welcome"))?;
        welcome.set_font("sans", 15., 650)?;
        let server = field(&form, tr, "homeserver", false)?;
        server.set_placeholder("https://matrix.example.org")?;
        let username = field(&form, tr, "username", false)?;
        username.set_placeholder("@name:example.org")?;
        let password = field(&form, tr, "password", true)?;
        let submit = form.button(&tr.text("sign-in"))?;
        submit.set_role(sys::CUI_ROLE_PRIMARY)?;
        let browser = form.button(&tr.text("sso-login"))?;
        let oauth = form.button(&tr.text("oauth-login"))?;
        let cancel = form.button(&tr.text("cancel"))?;
        let temporary = form.button(&tr.text("temporary-session"))?;
        let error = card.label("")?;
        error.set_role(sys::CUI_ROLE_DANGER)?;
        let status = label(&card, "", sys::CUI_ROLE_CAPTION)?;
        let notes = card.box_layout(sys::CUI_VERTICAL, 6)?;
        label(&notes, &tr.text("login-preview"), sys::CUI_ROLE_CAPTION)?;
        label(&notes, &tr.text("login-safety"), sys::CUI_ROLE_CAPTION)?;
        row.box_layout(sys::CUI_HORIZONTAL, 0)?.expand(true)?;
        background.box_layout(sys::CUI_VERTICAL, 0)?.expand(true)?;
        Ok(Self {
            card,
            form,
            notes,
            browser,
            oauth,
            cancel,
            background,
            server,
            username,
            password,
            submit,
            error,
            temporary,
            status,
        })
    }
}
