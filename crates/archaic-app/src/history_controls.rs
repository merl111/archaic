use crate::{
    model::{Model, Phase},
    view::set_text,
    widgets::label,
};
use archaic_i18n::Translator;
use archaic_matrix::Membership;
use cui::{Result, Widget, sys};

pub struct HistoryControls {
    pub newer: Widget,
    root: Widget,
    pub older: Widget,
    pub latest: Widget,
    status: Widget,
    error: Widget,
}
impl HistoryControls {
    pub fn new(parent: &Widget, tr: &Translator) -> Result<Self> {
        let root = parent.box_layout(sys::CUI_VERTICAL, 6)?;
        let row = root.box_layout(sys::CUI_HORIZONTAL, 10)?;
        let older = row.button(&tr.text("history-older"))?;
        let newer = row.button(&tr.text("history-newer"))?;
        let latest = row.button(&tr.text("history-latest"))?;
        let status = label(&row, "", sys::CUI_ROLE_CAPTION)?;
        status.expand(true)?;
        let error = label(&root, "", sys::CUI_ROLE_DANGER)?;
        Ok(Self {
            newer,
            root,
            older,
            latest,
            status,
            error,
        })
    }
    pub fn render(&self, model: &Model, tr: &Translator, fixture: bool) -> Result<()> {
        let joined = model
            .selected_room()
            .is_some_and(|room| room.membership == Membership::Joined);
        self.root.set_visible(joined)?;
        let enabled = model.phase == Phase::Connected
            && !fixture
            && model.history_pending.is_none()
            && !model.creating
            && model.room_pending.is_none()
            && model.room_confirmation.is_none()
            && !model.joining;
        self.older
            .set_enabled(enabled && model.history_info.can_load_more)?;
        self.latest.set_enabled(enabled)?;
        self.newer
            .set_enabled(enabled && model.history_info.can_load_newer)?;
        self.newer.set_visible(model.history_info.can_load_newer)?;
        let status = if model.history_pending.is_some() || model.status == "loading" {
            "loading"
        } else if model.history_error.is_some() && model.messages.is_empty() {
            "history-unavailable"
        } else if model.history_info.gap {
            "history-gap"
        } else if model.history_info.at_limit {
            "history-limit"
        } else if model.history_info.can_load_more {
            "history-available"
        } else {
            "history-start"
        };
        set_text(&self.status, &tr.text(status))?;
        set_text(
            &self.error,
            &model
                .history_error
                .map(|(_, key)| tr.text(key))
                .unwrap_or_default(),
        )?;
        self.error.set_visible(model.history_error.is_some())?;
        Ok(())
    }
}
