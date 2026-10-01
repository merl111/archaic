//! A real relation-backed thread next to the main Daylight timeline.
use crate::{
    Controller,
    daylight::style,
    message_state::ActionKind,
    model::{Model, Phase},
    timeline_view::Timeline,
    view::set_text,
};
use archaic_i18n::Translator;
use cui::{ChatComponent, ChatKind, Result, Widget, chat::Theme, sys};
use std::cell::{Cell, RefCell};
#[derive(Clone, PartialEq, Eq)]
pub struct Target {
    pub epoch: u64,
    pub room: String,
    pub event: String,
}
pub struct ThreadPane {
    pub root: Widget,
    pub close: Widget,
    pub more: Widget,
    pub timeline: Timeline,
    pub composer: ChatComponent,
    status: Widget,
    pub target: RefCell<Option<Target>>,
    pub open: Cell<bool>,
    was_pending: Cell<bool>,
}
impl ThreadPane {
    pub fn new(parent: &Widget, tr: &Translator, theme: &Theme) -> Result<Self> {
        let root = parent.box_layout(sys::CUI_VERTICAL, 0)?;
        root.set_min_size(340, 1)?;
        style(&root, theme, theme.surface, 20., 0)?;
        let head = root.box_layout(sys::CUI_HORIZONTAL, 12)?;
        head.box_set_padding(18)?;
        let title = head.label(&tr.text("daylight-thread"))?;
        title.set_font("sans", 12., 700)?;
        title.expand(true)?;
        let close = head.symbol_button(sys::CUI_SYMBOL_CLOSE, &tr.text("cancel"))?;
        let timeline = Timeline::new(&root, tr, theme)?;
        let foot = root.box_layout(sys::CUI_VERTICAL, 8)?;
        foot.box_set_padding(14)?;
        let status = foot.label("")?;
        status.set_role(sys::CUI_ROLE_CAPTION)?;
        let more = foot.button(&tr.text("details-more"))?;
        let composer = ChatComponent::create(&root, ChatKind::Composer, theme)?;
        let mut presentation = composer.presentation()?;
        presentation.composer_tools = 1 << 6;
        presentation.composer_padding = 14.;
        composer.set_presentation(&presentation)?;
        composer
            .part(0)?
            .set_placeholder(&tr.text("daylight-thread-reply"))?;
        composer.part(1)?.set_text(&tr.text("send"))?;
        root.set_visible(false)?;
        Ok(Self {
            root,
            close,
            more,
            timeline,
            composer,
            status,
            target: Default::default(),
            open: Cell::new(false),
            was_pending: Cell::new(false),
        })
    }
    pub fn theme(&self, t: &Theme) -> Result<()> {
        style(&self.root, t, t.surface, 20., 0)?;
        style(&self.close, t, t.selected, 10., 6)?;
        style(&self.more, t, t.selected, 12., 8)?;
        self.timeline.set_theme(t)?;
        self.composer.set_theme(t)
    }
    pub fn render(&self, m: &Model, tr: &Translator, fixture: bool) -> Result<()> {
        let valid = self.target.borrow().as_ref().is_some_and(|t| {
            t.epoch == m.epoch
                && m.selected.as_ref() == Some(&t.room)
                && m.phase == Phase::Connected
        });
        if !valid {
            self.open.set(false);
            self.target.borrow_mut().take();
            self.composer.part(0)?.set_text("")?;
        }
        self.root.set_visible(valid && self.open.get())?;
        if !valid {
            return Ok(());
        }
        let target = self.target.borrow().clone().expect("validated thread");
        let details = m
            .tools
            .details
            .as_ref()
            .filter(|d| d.message.id == target.event);
        let mut items = Vec::new();
        if let Some(d) = details {
            items.push(d.message.clone());
            items.extend(d.thread.clone());
        } else {
            items.extend(
                m.messages
                    .iter()
                    .filter(|v| {
                        v.id == target.event || v.thread_root.as_ref() == Some(&target.event)
                    })
                    .cloned(),
            );
        }
        self.timeline.render_items(
            m,
            tr,
            &items,
            &format!("{}:{}:{}", target.epoch, target.room, target.event),
            false,
        )?;
        let pending = m
            .message_draft
            .as_ref()
            .is_some_and(|d| d.kind == ActionKind::Thread && d.pending);
        if self.was_pending.replace(pending) && m.message_draft.is_none() {
            self.composer.part(0)?.set_text("")?;
        }
        let error = m
            .tools
            .error
            .or_else(|| m.message_draft.as_ref().and_then(|d| d.error));
        let status = if let Some(e) = error {
            tr.text(e)
        } else if m.tools_busy() {
            tr.text("loading")
        } else if items.len() <= 1 {
            tr.text("daylight-thread-empty")
        } else {
            String::new()
        };
        set_text(&self.status, &status)?;
        self.status.set_visible(!status.is_empty())?;
        self.more.set_visible(details.is_some_and(|d| d.more))?;
        self.more.set_enabled(m.can_tool() && !fixture)?;
        self.close.set_enabled(!pending)?;
        let foreign_action = m
            .message_draft
            .as_ref()
            .is_some_and(|d| d.kind != ActionKind::Thread || d.target != target.event);
        self.composer
            .busy(pending || m.tools_busy() || foreign_action)?;
        self.composer
            .part(0)?
            .set_enabled(!fixture && !pending && !m.tools_busy() && !foreign_action)?;
        Ok(())
    }
}
impl Controller {
    pub fn open_thread(&self) -> Result<()> {
        self.remember_draft()?;
        let target = {
            let m = self.model.borrow();
            if !m.can_tool() {
                return Ok(());
            }
            let Some(message) = m.selected_message() else {
                return Ok(());
            };
            Target {
                epoch: m.epoch,
                room: m.selected.clone().unwrap_or_default(),
                event: message
                    .thread_root
                    .clone()
                    .unwrap_or_else(|| message.id.clone()),
            }
        };
        if self.view.thread_pane.target.borrow().as_ref() != Some(&target) {
            self.view.thread_pane.composer.part(0)?.set_text("")?;
        }
        self.model.borrow_mut().message_selected = Some(target.event.clone());
        *self.view.thread_pane.target.borrow_mut() = Some(target);
        self.view.thread_pane.open.set(true);
        self.view.ui.panel.set(crate::daylight::Panel::None);
        self.model.borrow_mut().tools.show = false;
        if self.fixture {
            self.render()
        } else {
            self.load_details()
        }
    }
    pub fn close_thread(&self) -> Result<()> {
        if self
            .model
            .borrow()
            .message_draft
            .as_ref()
            .is_some_and(|d| d.kind == ActionKind::Thread && d.pending)
        {
            return Ok(());
        }
        if self
            .model
            .borrow()
            .message_draft
            .as_ref()
            .is_some_and(|d| d.kind == ActionKind::Thread)
        {
            self.model.borrow_mut().message_draft = None;
        }
        self.view.thread_pane.open.set(false);
        self.render()
    }
    pub fn thread_more(&self) -> Result<()> {
        if let Some(t) = self.view.thread_pane.target.borrow().as_ref() {
            self.model.borrow_mut().message_selected = Some(t.event.clone());
        }
        self.more_details()
    }
    pub fn send_thread(&self, value: &str) -> Result<()> {
        if self.fixture {
            return Ok(());
        }
        let Some(target) = self.view.thread_pane.target.borrow().clone() else {
            return Ok(());
        };
        {
            let mut m = self.model.borrow_mut();
            if m.epoch != target.epoch || m.selected.as_ref() != Some(&target.room) {
                return Ok(());
            }
            if m.message_draft
                .as_ref()
                .is_some_and(|d| d.kind != ActionKind::Thread || d.target != target.event)
            {
                return Ok(());
            }
            m.message_selected = Some(target.event);
            m.begin_message_action(ActionKind::Thread);
        }
        self.view.message_controls.body.set_text(value)?;
        self.submit_message()
    }
}
