use crate::{Controller, model::Phase};
use archaic_matrix::{Command, ToolAction, ToolRequest, transaction_id};
use cui::Result;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
#[derive(Default)]
pub struct State {
    pub unread: HashMap<String, u64>,
    pub opened: Option<String>,
    pub acknowledge: Option<String>,
    pub latest: HashMap<String, String>,
    pub viewed: HashMap<String, Option<String>>,
    pub typing: HashMap<String, (Vec<String>, Instant)>,
    edited_room: Option<String>,
    last_change: Option<Instant>,
    announced: Option<String>,
    last_sent: Option<Instant>,
    pub read: HashMap<String, String>,
    last_read_attempt: Option<Instant>,
}
impl Controller {
    pub fn composer_input(&self) -> Result<()> {
        self.remember_draft()?;
        let mut model = self.model.borrow_mut();
        model.activity.edited_room = model.selected.clone();
        model.activity.last_change = Some(Instant::now());
        Ok(())
    }

    pub fn activity_tick(&self) -> Result<()> {
        self.extras_tick()?;
        self.flush_drafts();
        if self.desktop_ready() && !self.drafts_dirty() {
            let pending = self.pending_profile.borrow_mut().take();
            if let Some(name) = pending {
                self.view.profile.set_text(&name)?;
                self.switch_profile()?;
            }
            if self.startup_link.get() && self.model.borrow().phase == Phase::Connected {
                self.startup_link.set(false);
                self.open_matrix_link()?;
            }
        }
        if self.fixture {
            return Ok(());
        }
        let (connected, room, visible, event) = {
            let m = self.model.borrow();
            (
                m.phase == Phase::Connected,
                m.selected.clone(),
                !m.security.show && !m.organization.show && !m.tools.show && !m.show_new,
                m.messages
                    .iter()
                    .rev()
                    .find(|m| m.thread_root.is_none())
                    .map(|m| m.id.clone()),
            )
        };
        let focused = connected && visible && self.view.composer.has_focus()?;
        let text = self.view.composer.text()?;
        let now = Instant::now();
        let mut commands = Vec::new();
        {
            let mut m = self.model.borrow_mut();
            let a = &mut m.activity;
            let typing = focused
                && a.edited_room == room
                && self.view.typing_opt.get_checked()?
                && !text.is_empty()
                && a.last_change
                    .is_some_and(|t| now.duration_since(t) < Duration::from_secs(5));
            if let Some(old) = a.announced.clone()
                && (!typing || room.as_ref() != Some(&old))
            {
                commands.push(Command::Typing {
                    room_id: old,
                    active: false,
                });
                a.announced = None;
                a.last_sent = None;
            }
            if typing
                && a.last_sent
                    .is_none_or(|t| now.duration_since(t) > Duration::from_secs(20))
                && let Some(id) = &room
            {
                commands.push(Command::Typing {
                    room_id: id.clone(),
                    active: true,
                });
                a.announced = Some(id.clone());
                a.last_sent = Some(now);
            }
            if (focused || a.acknowledge == room)
                && a.last_read_attempt
                    .is_none_or(|t| now.duration_since(t) >= Duration::from_secs(10))
                && self.view.read_opt.get_checked()?
                && let (Some(id), Some(event)) = (&room, event)
                && a.read.get(id) != Some(&event)
            {
                // Private receipts only. Public receipts remain an explicit room-tool action.
                commands.push(Command::RoomTool(ToolRequest {
                    room_id: id.clone(),
                    id: transaction_id(),
                    action: ToolAction::MarkRead {
                        event_id: event.clone(),
                        private: true,
                    },
                }));
                a.read.insert(id.clone(), event);
                a.unread.insert(id.clone(), 0);
                a.acknowledge = None;
                a.last_read_attempt = Some(now);
            }
        }
        for command in commands {
            if self.sender.try_send(command).is_err() {
                let mut m = self.model.borrow_mut();
                m.activity.last_sent = None;
                if let Some(id) = &room {
                    m.activity.read.remove(id);
                }
            }
        }
        let typing = {
            let m = self.model.borrow();
            room.as_ref()
                .and_then(|id| m.activity.typing.get(id))
                .filter(|(_, time)| now.duration_since(*time) < Duration::from_secs(30))
                .map(|(names, _)| names.join(", "))
                .unwrap_or_default()
        };
        crate::view::set_text(
            &self.view.typing_status,
            &if let Some(key) = self.model.borrow().tools.error {
                self.tr.text(key)
            } else if typing.is_empty() {
                String::new()
            } else {
                format!("{} · {}", typing, self.tr.text("activity-typing"))
            },
        )?;
        Ok(())
    }
}
