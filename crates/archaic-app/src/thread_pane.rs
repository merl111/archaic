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
        let timeline = Timeline::new(&root, tr, theme, crate::timeline_view::TimelineKind::Thread)?;
        let foot = root.box_layout(sys::CUI_VERTICAL, 8)?;
        foot.box_set_padding(14)?;
        let status = foot.label("")?;
        status.set_role(sys::CUI_ROLE_CAPTION)?;
        let more = foot.button(&tr.text("details-more"))?;
        let composer = ChatComponent::create(&root, ChatKind::Composer, theme)?;
        crate::chat_labels::apply(&composer, tr)?;
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
        let error = m
            .tools
            .error
            .or_else(|| m.message_draft.as_ref().and_then(|d| d.error));
        let status = if let Some(e) = error {
            tr.text(e)
        } else if m.tools_busy() {
            tr.text("loading")
        } else if m.with_local_echoes(&items, Some(&target.event)).len() <= 1 {
            tr.text("daylight-thread-empty")
        } else {
            String::new()
        };
        set_text(&self.status, &status)?;
        self.status.set_visible(!status.is_empty())?;
        self.more.set_visible(details.is_some_and(|d| d.more))?;
        self.more.set_enabled(m.can_tool() && !fixture)?;
        self.close.set_enabled(true)?;
        let foreign_action = m
            .message_draft
            .as_ref()
            .is_some_and(|d| d.kind != ActionKind::Thread || d.target != target.event);
        self.composer.busy(foreign_action)?;
        self.composer
            .part(0)?
            .set_enabled(!fixture && m.phase == Phase::Connected && !foreign_action)?;
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
        let prepared = self.model.borrow().prepare_thread_send(&target, value);
        let Some((id, pending)) = prepared else {
            return Ok(());
        };
        if self.enqueue(pending.command(&id)) {
            self.model.borrow_mut().start_thread_send(id, pending);
            self.view.thread_pane.composer.part(0)?.set_text("")?;
        }
        self.render()
    }
}

#[derive(Clone)]
pub struct PendingSend {
    pub room: String,
    pub root: String,
    pub latest: String,
    pub body: String,
}
impl PendingSend {
    pub fn command(&self, id: &str) -> archaic_matrix::Command {
        archaic_matrix::Command::EnqueueThread {
            room_id: self.room.clone(),
            root: self.root.clone(),
            latest: self.latest.clone(),
            body: self.body.clone(),
            transaction: id.into(),
        }
    }
}
impl Model {
    pub fn prepare_thread_send(
        &self,
        target: &Target,
        body: &str,
    ) -> Option<(String, PendingSend)> {
        if self.epoch != target.epoch
            || self.selected.as_ref() != Some(&target.room)
            || self.phase != Phase::Connected
            || body.trim().is_empty()
            || body.len() > 32_000
            || self.message_draft.is_some()
            || self.room_pending.is_some()
            || !self
                .selected_room()
                .is_some_and(|r| r.membership == archaic_matrix::Membership::Joined)
        {
            return None;
        }
        if !self.messages.iter().any(|m| m.id == target.event)
            && !self
                .tools
                .details
                .as_ref()
                .is_some_and(|d| d.message.id == target.event)
        {
            return None;
        }
        let latest = self
            .tools
            .details
            .as_ref()
            .filter(|d| d.message.id == target.event)
            .and_then(|d| d.thread.last())
            .map(|m| m.id.clone())
            .or_else(|| {
                self.messages
                    .iter()
                    .rev()
                    .find(|m| m.thread_root.as_ref() == Some(&target.event))
                    .map(|m| m.id.clone())
            })
            .unwrap_or_else(|| target.event.clone());
        Some((
            archaic_matrix::transaction_id().to_string(),
            PendingSend {
                room: target.room.clone(),
                root: target.event.clone(),
                latest,
                body: body.into(),
            },
        ))
    }
    pub fn start_thread_send(&mut self, id: String, pending: PendingSend) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.update_outbox(vec![archaic_matrix::outbox::Item {
            room_id: pending.room.clone(),
            id: id.clone(),
            body: pending.body.clone(),
            failed: false,
            message: Some(archaic_matrix::Message {
                id: format!("~{id}"),
                transaction_id: Some(id.clone()),
                sender: self.user.clone(),
                body: Some(pending.body.clone()),
                timestamp,
                thread_root: Some(pending.root.clone()),
                ..Default::default()
            }),
            target: None,
            reaction: None,
            event_id: None,
        }]);
        self.thread_sends.insert(id, pending);
    }
    pub fn thread_queued(&mut self, id: &str, result: std::result::Result<(), &'static str>) {
        if !self.thread_sends.contains_key(id) {
            return;
        }
        match result {
            Ok(()) => {
                self.thread_sends.remove(id);
                self.outbox.retain(|i| i.id != id);
            }
            Err(_) => {
                if let Some(item) = self.outbox.iter_mut().find(|i| i.id == id) {
                    item.failed = true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (Model, Target) {
        let mut m = crate::visual_fixture::model();
        let room = m.selected.clone().unwrap();
        let root = m.messages[0].id.clone();
        m.message_draft = None;
        let target = Target {
            epoch: m.epoch,
            room,
            event: root,
        };
        (m, target)
    }
    #[test]
    fn thread_send_does_not_take_the_global_action_lock_or_block_another_send() {
        let (mut m, target) = setup();
        let (id, pending) = m.prepare_thread_send(&target, "First reply").unwrap();
        m.start_thread_send(id.clone(), pending);
        assert!(m.message_draft.is_none());
        assert!(!m.sending);
        assert!(m.room_action_allowed(&archaic_matrix::RoomAction::Leave));
        let (second, _) = m.prepare_thread_send(&target, "Second reply").unwrap();
        assert_ne!(id, second);
        assert_eq!(
            m.with_local_echoes(&[], Some(&target.event))[0]
                .body
                .as_deref(),
            Some("First reply")
        );
        m.thread_queued(&id, Ok(()));
        assert!(m.thread_sends.is_empty());
        assert!(m.message_draft.is_none());
    }
    #[test]
    fn local_queue_failure_keeps_text_and_retry_without_modifying_other_room() {
        let (mut m, target) = setup();
        let (id, pending) = m.prepare_thread_send(&target, "Keep this text").unwrap();
        m.start_thread_send(id.clone(), pending);
        m.selected = Some("!other:local".into());
        m.thread_queued(&id, Err("outbox-failed"));
        assert!(m.outbox[0].failed);
        assert_eq!(m.outbox[0].body, "Keep this text");
        assert!(
            matches!(m.thread_sends[&id].command(&id),archaic_matrix::Command::EnqueueThread { room_id,body,.. } if room_id==target.room && body=="Keep this text")
        );
        assert_eq!(m.selected.as_deref(), Some("!other:local"));
        assert!(m.prepare_thread_send(&target, "stale target").is_none());
    }
}
