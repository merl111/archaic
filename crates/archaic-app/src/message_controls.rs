use crate::{Controller, message_state::ActionKind, model::Model, view::set_text, widgets::label};
use archaic_i18n::Translator;
use cui::{Result, Widget, sys};

pub struct MessageControls {
    preview: Widget,
    root: Widget,
    form: Widget,
    title: Widget,
    pub body: Widget,
    pub submit: Widget,
    pub cancel: Widget,
    error: Widget,
}
impl MessageControls {
    pub fn new(parent: &Widget, tr: &Translator) -> Result<Self> {
        let root = parent.box_layout(sys::CUI_VERTICAL, 16)?;
        let preview = label(&root, "", sys::CUI_ROLE_CAPTION)?;
        let form = root.box_layout(sys::CUI_VERTICAL, 6)?;
        let title = label(&form, "", sys::CUI_ROLE_CAPTION)?;
        let body = form.textarea("")?;
        body.set_min_size(200, 64)?;
        body.accessibility(&tr.text("message-action-body"), "")?;
        let error = label(&form, "", sys::CUI_ROLE_DANGER)?;
        let form_actions = form.box_layout(sys::CUI_HORIZONTAL, 8)?;
        let cancel = form_actions.button(&tr.text("cancel"))?;
        let submit = form_actions.button(&tr.text("message-apply"))?;
        Ok(Self {
            preview,
            root,
            form,
            title,
            body,
            submit,
            cancel,
            error,
        })
    }
    pub fn theme(&self, t: &cui::chat::Theme) -> Result<()> {
        crate::daylight::style(&self.cancel, t, t.selected, 12., 10)?;
        self.submit.set_style(Some(&sys::cui_widget_style {
            background: t.foreground,
            foreground: t.surface,
            radius: 12.,
            padding: 10,
            ..Default::default()
        }))?;
        crate::daylight::style(&self.body, t, t.selected, 12., 12)?;
        Ok(())
    }
    pub fn render(&self, model: &Model, tr: &Translator, fixture: bool) -> Result<()> {
        self.root.set_visible(
            !model.messages.is_empty()
                || model.message_draft.is_some()
                || model.tools.details.is_some(),
        )?;
        let available = !fixture && model.can_message_action();
        self.render_preview(model)?;
        self.form.set_visible(model.message_draft.is_some())?;
        if let Some(draft) = &model.message_draft {
            self.render_form(draft, tr, available)?;
        }

        Ok(())
    }
    fn render_preview(&self, model: &Model) -> Result<()> {
        let selected = model.selected_message();
        set_text(
            &self.preview,
            &selected
                .map(|m| {
                    format!(
                        "{}\n{}",
                        m.sender,
                        crate::daylight::text(m.body.as_deref().unwrap_or(""), 180)
                    )
                })
                .unwrap_or_default(),
        )?;
        Ok(())
    }
    fn render_form(
        &self,
        draft: &crate::message_state::ActionDraft,
        tr: &Translator,
        available: bool,
    ) -> Result<()> {
        let hint = match draft.kind {
            ActionKind::Forward => "message-forward-hint",
            ActionKind::Delete => "message-delete-confirm",
            ActionKind::Reaction => "reaction-hint",
            _ => draft.kind.key(),
        };
        set_text(&self.title, &tr.text(hint))?;
        self.title.set_tooltip(&draft.target)?;
        self.body.set_visible(draft.kind != ActionKind::Delete)?;
        self.body.set_enabled(available && !draft.pending)?;
        set_text(&self.body, &draft.body)?;
        set_text(
            &self.submit,
            &tr.text(if draft.pending {
                "sending"
            } else {
                "message-apply"
            }),
        )?;
        self.submit.set_enabled(available && !draft.pending)?;
        self.cancel.set_enabled(!draft.pending)?;
        self.error.set_visible(draft.error.is_some())?;
        set_text(
            &self.error,
            &draft.error.map(|key| tr.text(key)).unwrap_or_default(),
        )?;
        Ok(())
    }
}
impl Controller {
    fn begin_message(&self, kind: ActionKind) -> Result<()> {
        self.remember_draft()?;
        if !self.fixture {
            self.model.borrow_mut().begin_message_action(kind);
        }
        self.view.ui.panel.set(crate::daylight::Panel::None);
        self.render()?;
        if kind.inline() {
            self.view.composer.focus()?;
        }
        Ok(())
    }
    pub fn forward_message(&self) -> Result<()> {
        self.begin_message(ActionKind::Forward)
    }
    pub fn reply_message(&self) -> Result<()> {
        self.begin_message(ActionKind::Reply)
    }
    pub fn edit_message(&self) -> Result<()> {
        self.begin_message(ActionKind::Edit)
    }
    pub fn delete_message(&self) -> Result<()> {
        self.begin_message(ActionKind::Delete)
    }
    pub fn react_message(&self) -> Result<()> {
        self.begin_message(ActionKind::Reaction)
    }
    pub fn cancel_message(&self) -> Result<()> {
        if self
            .model
            .borrow()
            .message_draft
            .as_ref()
            .is_some_and(|d| !d.pending)
        {
            self.model.borrow_mut().message_draft = None;
        }
        self.render()
    }
    pub fn submit_message(&self) -> Result<()> {
        self.remember_draft()?;
        let command = self.model.borrow_mut().message_command();
        if !self.fixture
            && let Some(command) = command
            && self.enqueue(command)
        {
            let mut model = self.model.borrow_mut();
            if let Some(draft) = &mut model.message_draft {
                draft.pending = true;
                draft.error = None;
            }
        }
        self.render()
    }
}
