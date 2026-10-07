//! A stable two-column settings layout with persistent category selection.
use crate::{
    daylight::{Shell, style},
    widgets::label,
};
use archaic_i18n::Translator;
use cui::{Result, Widget, chat::Theme, sys};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
pub struct Settings {
    pub root: Widget,
    pub sync_status: Widget,
    pub pages: Vec<Widget>,
    buttons: Vec<Widget>,
    nav: Widget,
    selected: Rc<Cell<usize>>,
    palette: Rc<RefCell<Theme>>,
}
impl Settings {
    pub fn new(ui: &Shell, tr: &Translator) -> Result<Self> {
        let root = ui.card(&ui.sheet_body, 0., 0)?;
        let columns = root.box_layout(sys::CUI_HORIZONTAL, 24)?;
        columns.set_min_size(1, 420)?;
        let nav = columns.box_layout(sys::CUI_VERTICAL, 6)?;
        nav.set_min_size(184, 1)?;
        let body = columns.box_layout(sys::CUI_VERTICAL, 0)?;
        body.expand(true)?;
        let selected = Rc::new(Cell::new(0));
        let palette = Rc::new(RefCell::new(ui.theme.borrow().clone()));
        let mut buttons = Vec::new();
        let mut pages = Vec::new();
        for (title, subtitle, symbol) in [
            (
                "settings-account",
                "settings-account-hint",
                sys::CUI_SYMBOL_PERSON,
            ),
            (
                "appearance",
                "settings-appearance-hint",
                sys::CUI_SYMBOL_SUN,
            ),
            (
                "settings-notifications",
                "settings-notifications-hint",
                sys::CUI_SYMBOL_BELL,
            ),
            (
                "security-title",
                "security-settings-description",
                sys::CUI_SYMBOL_LOCK,
            ),
            (
                "settings-advanced",
                "settings-advanced-hint",
                sys::CUI_SYMBOL_SETTINGS,
            ),
        ] {
            let button = nav.button(&tr.text(title))?;
            button.set_icon(Some(&cui::Icon::symbol(symbol)?))?;
            button.set_icon_size(18)?;
            button.set_tooltip(&tr.text(title))?;
            button.accessibility(&tr.text(title), "")?;
            button.set_min_size(176, 42)?;
            let page = ui.card(&body, 0., 0)?;
            label(&page, &tr.text(title), sys::CUI_ROLE_HEADING)?;
            let _hint = label(&page, &tr.text(subtitle), sys::CUI_ROLE_CAPTION)?;
            page.separator()?;
            pages.push(page);
            buttons.push(button);
        }
        for (index, button) in buttons.iter().enumerate() {
            let pages = pages.clone();
            let buttons = buttons.clone();
            let selected = selected.clone();
            let palette = palette.clone();
            button.on_action(move |_| {
                selected.set(index);
                update(&pages, &buttons, index, &palette.borrow()).expect("live settings");
            })?;
        }
        label(
            &pages[4],
            &tr.text("history-sync-title"),
            sys::CUI_ROLE_HEADING,
        )?;
        let sync_status = label(
            &pages[4],
            &tr.text("history-sync-waiting"),
            sys::CUI_ROLE_BODY,
        )?;
        let result = Self {
            root,
            sync_status,
            pages,
            buttons,
            nav,
            selected,
            palette,
        };
        result.theme(&ui.theme.borrow())?;
        Ok(result)
    }
    pub fn refresh(&self, text_scale: f64) -> Result<()> {
        let width = self.root.allocated_size()?.map_or(810, |s| s.0);
        let compact = f64::from(width) < 600. * text_scale;
        self.nav.set_min_size(if compact { 48 } else { 184 }, 1)?;
        for button in &self.buttons {
            button.set_icon_only(compact)?;
            button.set_min_size(if compact { 44 } else { 176 }, 42)?;
        }
        Ok(())
    }
    pub fn selected(&self) -> usize {
        self.selected.get()
    }
    pub fn security_button(&self) -> Widget {
        self.buttons[3].clone()
    }
    pub fn select(&self, index: usize) -> Result<()> {
        self.selected.set(index);
        update(&self.pages, &self.buttons, index, &self.palette.borrow())
    }
    pub fn theme(&self, t: &Theme) -> Result<()> {
        *self.palette.borrow_mut() = t.clone();
        update(&self.pages, &self.buttons, self.selected.get(), t)
    }
}
fn update(pages: &[Widget], buttons: &[Widget], selected: usize, t: &Theme) -> Result<()> {
    for (i, (page, button)) in pages.iter().zip(buttons).enumerate() {
        page.set_visible(i == selected)?;
        style(
            button,
            t,
            if i == selected { t.selected } else { t.surface },
            10.,
            10,
        )?;
    }
    Ok(())
}
