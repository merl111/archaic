use crate::{Controller, message_state::ActionKind, model::Model, view::set_text, widgets::label};
use archaic_i18n::Translator;
use cui::{Result, Widget, sys};
use std::cell::RefCell;

pub struct MessageControls {
    root: Widget,
    pub select: Widget,
    pub reply: Widget,
    pub edit: Widget,
    pub delete: Widget,
    pub react: Widget,
    form: Widget,
    title: Widget,
    pub body: Widget,
    pub submit: Widget,
    pub cancel: Widget,
    error: Widget,
    labels: RefCell<Vec<String>>,
}
impl MessageControls {
    pub fn new(parent: &Widget, tr: &Translator) -> Result<Self> {
        let root = parent.box_layout(sys::CUI_VERTICAL, 8)?;
        let row = root.box_layout(sys::CUI_HORIZONTAL, 8)?;
        let select = row.select(&[])?;
        select.expand(true)?;
        select.accessibility(&tr.text("message-select"), "")?;
        select.set_tooltip(&tr.text("message-select"))?;
        let reply = row.button(&tr.text("message-reply"))?;
        let edit = row.button(&tr.text("message-edit"))?;
        let delete = row.button(&tr.text("message-delete"))?;
        let react = row.button(&tr.text("message-react"))?;
        let form = root.box_layout(sys::CUI_VERTICAL, 6)?;
        let title = label(&form, "", sys::CUI_ROLE_CAPTION)?;
        let body = form.textarea("")?;
        body.set_min_size(200, 64)?;
        body.accessibility(&tr.text("message-action-body"), "")?;
        let error = label(&form, "", sys::CUI_ROLE_DANGER)?;
        let actions = form.box_layout(sys::CUI_HORIZONTAL, 8)?;
        let cancel = actions.button(&tr.text("cancel"))?;
        let submit = actions.button(&tr.text("message-apply"))?;
        Ok(Self {
            root,
            select,
            reply,
            edit,
            delete,
            react,
            form,
            title,
            body,
            submit,
            cancel,
            error,
            labels: RefCell::new(Vec::new()),
        })
    }
    pub fn render(&self, model: &Model, tr: &Translator, fixture: bool) -> Result<()> {
        self.root
            .set_visible(!model.messages.is_empty() || model.message_draft.is_some())?;
        let mut labels: Vec<_> = model
            .messages
            .iter()
            .map(|m| {
                format!(
                    "{} · {}",
                    m.sender,
                    m.body
                        .as_deref()
                        .unwrap_or("")
                        .chars()
                        .take(36)
                        .collect::<String>()
                )
                .replace(['\0', '\n', '\r'], " ")
            })
            .collect();
        labels.insert(0, tr.text("message-select"));
        if *self.labels.borrow() != labels {
            self.select
                .set_items(&labels.iter().map(String::as_str).collect::<Vec<_>>())?;
            *self.labels.borrow_mut() = labels;
        }
        let selected = model
            .messages
            .iter()
            .position(|m| model.message_selected.as_ref() == Some(&m.id))
            .map(|i| i as i32 + 1)
            .unwrap_or(0);
        if self.select.get_selected()? != selected {
            self.select.set_selected(selected)?;
        }
        let available = !fixture && model.can_message_action();
        let active = available && model.message_draft.is_none();
        self.select.set_enabled(active)?;
        let message = model
            .selected_message()
            .filter(|m| !m.deleted && m.body.is_some());
        self.reply.set_enabled(active && message.is_some())?;
        self.react.set_enabled(active && message.is_some())?;
        let own = message.is_some_and(|m| m.editable && m.sender == model.user);
        self.edit.set_enabled(active && own)?;
        self.delete.set_enabled(active && own)?;
        self.form.set_visible(model.message_draft.is_some())?;
        if let Some(draft) = &model.message_draft {
            self.render_form(draft, tr, available)?;
        }

        Ok(())
    }
    fn render_form(
        &self,
        draft: &crate::message_state::ActionDraft,
        tr: &Translator,
        available: bool,
    ) -> Result<()> {
        let hint = match draft.kind {
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
    pub fn choose_message(&self) -> Result<()> {
        self.remember_draft()?;
        let index = self.view.message_controls.select.get_selected()?;
        let mut model = self.model.borrow_mut();
        if model.message_draft.is_none() {
            model.message_selected = usize::try_from(index)
                .ok()
                .and_then(|i| i.checked_sub(1))
                .and_then(|i| model.messages.get(i))
                .map(|m| m.id.clone());
        }
        drop(model);
        self.render()
    }
    fn begin_message(&self, kind: ActionKind) -> Result<()> {
        self.remember_draft()?;
        if !self.fixture {
            self.model.borrow_mut().begin_message_action(kind);
        }
        self.render()
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
