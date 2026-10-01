use crate::{
    Controller,
    model::{Model, Phase},
    view::set_text,
};
use archaic_i18n::Translator;
use cui::{Result, Widget, sys};
pub struct Controls {
    root: Widget,
    pub items: Widget,
    pub retry: Widget,
    pub cancel: Widget,
    labels: std::cell::RefCell<Vec<String>>,
}
impl Controls {
    pub fn new(parent: &Widget, tr: &Translator) -> Result<Self> {
        let root = parent.box_layout(sys::CUI_HORIZONTAL, 6)?;
        root.label(&tr.text("outbox-title"))?;
        let items = root.select(&[])?;
        items.expand(true)?;
        let retry = root.button(&tr.text("outbox-retry"))?;
        let cancel = root.button(&tr.text("outbox-cancel"))?;
        Ok(Self {
            root,
            items,
            retry,
            cancel,
            labels: Default::default(),
        })
    }
    pub fn render(&self, model: &Model, tr: &Translator, fixture: bool) -> Result<()> {
        let items = model
            .outbox
            .iter()
            .filter(|i| Some(&i.room_id) == model.selected.as_ref())
            .collect::<Vec<_>>();
        self.root.set_visible(!items.is_empty())?;
        let labels = items
            .iter()
            .map(|i| {
                format!(
                    "{} · {}",
                    tr.text(if i.failed {
                        "outbox-paused"
                    } else {
                        "outbox-queued"
                    }),
                    i.body
                        .chars()
                        .take(100)
                        .collect::<String>()
                        .replace('\0', "�")
                )
            })
            .collect::<Vec<_>>();
        if *self.labels.borrow() != labels {
            let old = self.items.get_selected()?;
            self.items
                .set_items(&labels.iter().map(String::as_str).collect::<Vec<_>>())?;
            if !labels.is_empty() {
                self.items
                    .set_selected(old.max(0).min(labels.len() as i32 - 1))?;
            }
            *self.labels.borrow_mut() = labels;
        }
        let enabled = !fixture && model.phase == Phase::Connected && !items.is_empty();
        self.retry.set_enabled(enabled)?;
        self.cancel.set_enabled(enabled)?;
        set_text(&self.retry, &tr.text("outbox-retry"))?;
        Ok(())
    }
}
impl Controller {
    pub fn retry_outbox(&self) -> Result<()> {
        self.outbox_action(true)
    }
    pub fn cancel_outbox(&self) -> Result<()> {
        self.outbox_action(false)
    }
    fn outbox_action(&self, retry: bool) -> Result<()> {
        if self.fixture || self.model.borrow().phase != Phase::Connected {
            return Ok(());
        }
        let index = self.view.outbox.items.get_selected()?;
        if index < 0 {
            return Ok(());
        }
        let item = {
            let m = self.model.borrow();
            m.outbox
                .iter()
                .filter(|i| Some(&i.room_id) == m.selected.as_ref())
                .nth(index as usize)
                .cloned()
        };
        if let Some(item) = item {
            self.enqueue(archaic_matrix::Command::OutboxAction {
                room_id: item.room_id,
                id: item.id,
                retry,
            });
        }
        self.render()
    }
}
