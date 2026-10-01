use crate::{
    model::{Model, Phase},
    view::set_text,
    widgets::label,
};
use archaic_i18n::Translator;
use archaic_matrix::NewConversation;
use cui::{Result, Widget, sys};

pub struct NewConversationForm {
    root: Widget,
    pub mode: Widget,
    direct: Widget,
    pub person: Widget,
    group: Widget,
    name: Widget,
    topic: Widget,
    invitees: Widget,
    pub submit: Widget,
    pub cancel: Widget,
    error: Widget,
}
impl NewConversationForm {
    pub fn new(parent: &Widget, tr: &Translator) -> Result<Self> {
        let root = parent.box_layout(sys::CUI_VERTICAL, 12)?;
        root.box_set_padding(0)?;
        root.expand(true)?;
        let mode = root.select(&[&tr.text("create-direct"), &tr.text("create-group")])?;
        mode.set_selected(0)?;
        mode.accessibility(&tr.text("create-kind"), "")?;
        let direct = root.box_layout(sys::CUI_VERTICAL, 8)?;
        let person = field(&direct, tr, "create-person", "@person:example.org")?;
        let group = root.box_layout(sys::CUI_VERTICAL, 8)?;
        let name = field(&group, tr, "create-name", "")?;
        let topic = field(&group, tr, "create-topic", "")?;
        let invitees = field(
            &group,
            tr,
            "create-invitees",
            "@alice:example.org, @bob:example.org",
        )?;
        label(&root, &tr.text("create-encryption"), sys::CUI_ROLE_CAPTION)?;
        label(&root, &tr.text("create-existing"), sys::CUI_ROLE_CAPTION)?;
        let error = label(&root, "", sys::CUI_ROLE_DANGER)?;
        let actions = root.box_layout(sys::CUI_HORIZONTAL, 12)?;
        let cancel = actions.button(&tr.text("cancel"))?;
        let submit = actions.button(&tr.text("create-open"))?;
        submit.set_role(sys::CUI_ROLE_PRIMARY)?;
        Ok(Self {
            root,
            mode,
            direct,
            person,
            group,
            name,
            topic,
            invitees,
            submit,
            cancel,
            error,
        })
    }
    pub fn input(&self) -> Result<NewConversation> {
        Ok(if self.mode.get_selected()? == 1 {
            NewConversation::Group {
                name: self.name.text()?,
                topic: self.topic.text()?,
                invitees: self.invitees.text()?,
            }
        } else {
            NewConversation::Direct(self.person.text()?)
        })
    }
    pub fn clear(&self) -> Result<()> {
        for field in [&self.person, &self.name, &self.topic, &self.invitees] {
            field.set_text("")?;
        }
        self.mode.set_selected(0)
    }
    pub fn render(&self, model: &Model, tr: &Translator, fixture: bool) -> Result<()> {
        self.root
            .set_visible(model.show_new && model.phase == Phase::Connected)?;
        let group = self.mode.get_selected()? == 1;
        self.direct.set_visible(!group)?;
        self.group.set_visible(group)?;
        let enabled = model.can_create() && !fixture;
        for widget in [
            &self.mode,
            &self.person,
            &self.name,
            &self.topic,
            &self.invitees,
            &self.submit,
            &self.cancel,
        ] {
            widget.set_enabled(enabled)?;
        }
        set_text(
            &self.submit,
            &tr.text(if model.creating {
                "create-working"
            } else {
                "create-open"
            }),
        )?;
        set_text(
            &self.error,
            &model
                .create_error
                .map(|key| tr.text(key))
                .unwrap_or_default(),
        )?;
        self.error.set_visible(model.create_error.is_some())?;
        Ok(())
    }
}
fn field(parent: &Widget, tr: &Translator, key: &str, placeholder: &str) -> Result<Widget> {
    label(parent, &tr.text(key), sys::CUI_ROLE_CAPTION)?;
    let field = parent.entry("")?;
    field.set_placeholder(placeholder)?;
    field.accessibility(&tr.text(key), "")?;
    Ok(field)
}
impl Model {
    pub fn can_create(&self) -> bool {
        self.message_draft.is_none()
            && !self.tools_busy()
            && self.phase == Phase::Connected
            && !self.creating
            && !self.joining
            && self.room_pending.is_none()
            && self.room_confirmation.is_none()
            && !self.sending
    }
}
