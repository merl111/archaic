use crate::model::{Model, Phase};
use crate::view::set_text;
use crate::widgets::label;
use archaic_i18n::Translator;
use cui::{Result, Widget, sys};

pub struct JoinForm {
    pub join_form: Widget,
    pub join_address: Widget,
    pub join_submit: Widget,
    pub join_cancel: Widget,
    pub join_error: Widget,
}

impl JoinForm {
    pub fn new(sidebar: &Widget, tr: &Translator) -> Result<Self> {
        let join_form = sidebar.box_layout(sys::CUI_VERTICAL, 10)?;
        join_form.set_role(sys::CUI_ROLE_CARD)?;
        join_form.box_set_padding(12)?;
        label(&join_form, &tr.text("join-room"), sys::CUI_ROLE_HEADING)?;
        label(&join_form, &tr.text("join-address"), sys::CUI_ROLE_CAPTION)?;
        let join_address = join_form.entry("")?;
        join_address.set_placeholder("#room:example.org")?;
        join_address.accessibility(&tr.text("join-address"), "")?;
        let join_error = label(&join_form, "", sys::CUI_ROLE_DANGER)?;
        let join_actions = join_form.box_layout(sys::CUI_HORIZONTAL, 8)?;
        let join_cancel = join_actions.button(&tr.text("cancel"))?;
        let join_submit = join_actions.button(&tr.text("join"))?;
        join_submit.expand(true)?;
        join_submit.set_role(sys::CUI_ROLE_PRIMARY)?;
        Ok(Self {
            join_form,
            join_address,
            join_submit,
            join_cancel,
            join_error,
        })
    }
    pub fn render(&self, model: &Model, tr: &Translator, fixture: bool) -> Result<()> {
        let enabled = model.phase == Phase::Connected
            && !model.joining
            && !model.creating
            && model.room_pending.is_none()
            && !fixture;
        self.join_form.set_visible(model.show_join)?;
        self.join_address.set_enabled(enabled)?;
        self.join_submit.set_enabled(enabled)?;
        self.join_cancel.set_enabled(enabled)?;
        set_text(
            &self.join_submit,
            &tr.text(if model.joining { "joining" } else { "join" }),
        )?;
        set_text(
            &self.join_error,
            &model.join_error.map(|key| tr.text(key)).unwrap_or_default(),
        )?;
        self.join_error.set_visible(model.join_error.is_some())?;
        Ok(())
    }
}
