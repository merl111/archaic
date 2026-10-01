//! CUI tables recycle native rows; unchanged rows are not rewritten on sync.
use crate::{
    Controller,
    model::Model,
    view::{message_text, set_text},
};
use archaic_i18n::Translator;
use cui::{Result, Widget, sys};
use std::cell::RefCell;
pub struct Timeline {
    pub rows: Widget,
    preview: Widget,
    cache: RefCell<Vec<(String, String)>>,
}
impl Timeline {
    pub fn new(parent: &Widget, tr: &Translator) -> Result<Self> {
        let rows = parent.table(&[&tr.text("timeline-messages")])?;
        rows.expand(true)?;
        rows.set_min_size(200, 130)?;
        let preview = parent.label("")?;
        preview.set_role(sys::CUI_ROLE_CAPTION)?;
        Ok(Self {
            rows,
            preview,
            cache: Default::default(),
        })
    }
    pub fn render(&self, model: &Model, tr: &Translator) -> Result<()> {
        let entries = model
            .messages
            .iter()
            .map(|m| (m.id.clone(), message_text(m, model, tr).replace('\0', "�")))
            .collect::<Vec<_>>();
        let old = self.cache.borrow();
        if old
            .iter()
            .map(|(id, _)| id)
            .ne(entries.iter().map(|(id, _)| id))
        {
            let rows = entries
                .iter()
                .map(|(_, text)| [text.as_str()])
                .collect::<Vec<_>>();
            self.rows
                .table_set_rows(&rows.iter().map(|r| r.as_slice()).collect::<Vec<_>>())?;
        } else {
            for (index, ((_, before), (_, after))) in old.iter().zip(&entries).enumerate() {
                if before != after {
                    self.rows.table_set_cell(index, 0, after)?;
                }
            }
        }
        drop(old);
        *self.cache.borrow_mut() = entries;
        let selected = model
            .messages
            .iter()
            .position(|m| Some(&m.id) == model.message_selected.as_ref())
            .map(|i| i as i32)
            .unwrap_or(-1);
        if self.rows.get_selected()? != selected {
            self.rows.set_selected(selected)?;
        }
        self.preview.set_visible(model.messages.is_empty())?;
        let text = tr.text(if model.status == "loading" {
            "loading"
        } else {
            "empty-history"
        });
        set_text(&self.preview, &text)?;
        Ok(())
    }
}
impl Controller {
    pub fn choose_timeline_message(&self) -> Result<()> {
        if self.model.borrow().message_draft.is_some() {
            return Ok(());
        }
        let index = self.view.timeline.rows.get_selected()?;
        let mut m = self.model.borrow_mut();
        m.message_selected = usize::try_from(index)
            .ok()
            .and_then(|i| m.messages.get(i))
            .map(|m| m.id.clone());
        drop(m);
        self.render()
    }
}
