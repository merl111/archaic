//! Native CUI context menus. The application owns actions and Matrix scope only.
use crate::{Controller, model::Phase};
use cui::{App, ChatComponent, ChatEvent, Command, Menu, Result, sys};
use std::rc::Rc;

#[derive(Clone, Copy)]
enum Action {
    React,
    Reply,
    Thread,
    Forward,
    Copy,
    Context,
    Details,
    Edit,
    Delete,
}
#[derive(Clone)]
struct Target {
    epoch: u64,
    room: String,
    event: String,
    chat: ChatComponent,
    anchor: ChatEvent,
}
pub struct Menus {
    regular: Menu,
    own: Menu,
    commands: Vec<(Action, Command)>,
    target: Option<Target>,
}
impl Controller {
    pub fn install_message_menu(self: &Rc<Self>, app: &App) -> Result<()> {
        let regular = app.menu()?;
        let own = app.menu()?;
        let mut commands = Vec::new();
        for (action, key) in [
            (Action::React, "message-react"),
            (Action::Reply, "message-reply"),
            (Action::Thread, "message-thread"),
            (Action::Forward, "message-forward"),
            (Action::Copy, "daylight-copy"),
            (Action::Context, "message-context"),
            (Action::Details, "tool-details"),
            (Action::Edit, "message-edit"),
            (Action::Delete, "message-delete"),
        ] {
            let weak = Rc::downgrade(self);
            let command = app.command(&self.tr.text(key), 0, 0, move || {
                if let Some(c) = weak.upgrade() {
                    c.message_menu_action(action).expect("live message menu");
                }
            })?;
            if matches!(action, Action::Edit) {
                own.separator()?;
            }
            own.add(&command)?;
            if !matches!(action, Action::Edit | Action::Delete) {
                regular.add(&command)?;
            }
            commands.push((action, command));
        }
        *self.message_menu.borrow_mut() = Some(Menus {
            regular,
            own,
            commands,
            target: None,
        });
        Ok(())
    }
    pub fn open_message_menu(&self, chat: &ChatComponent, event: &ChatEvent) -> Result<()> {
        self.remember_draft()?;
        let raw = sys::cui_chat_event {
            action: event.action,
            id: event.id,
            detail_id: event.detail,
            index: event.index as u32,
            modifiers: event.modifiers,
            text: std::ptr::null(),
        };
        let Some(region) = chat.region(&raw) else {
            return Ok(());
        };
        let menu = {
            let m = self.model.borrow();
            let Some(message) = m.selected_message() else {
                return Ok(());
            };
            let Some(room) = &m.selected else {
                return Ok(());
            };
            let mut menus = self.message_menu.borrow_mut();
            let Some(menus) = menus.as_mut() else {
                return Ok(());
            };
            let available = !self.fixture && m.can_message_action() && m.message_draft.is_none();
            let own = message.editable && message.sender == m.user;
            for (action, command) in &menus.commands {
                let enabled = match action {
                    Action::Copy => message.body.is_some(),
                    Action::Edit | Action::Delete => available && own && !message.deleted,
                    Action::Forward => available && message.editable && !message.deleted,
                    Action::Context => available && m.history_pending.is_none(),
                    Action::Details => available,
                    Action::React | Action::Reply | Action::Thread => available && !message.deleted,
                };
                command.set_enabled(enabled)?;
            }
            menus.target = Some(Target {
                epoch: m.epoch,
                room: room.clone(),
                event: message.id.clone(),
                chat: chat.clone(),
                anchor: ChatEvent {
                    detail: 0,
                    ..event.clone()
                },
            });
            if own {
                menus.own.clone()
            } else {
                menus.regular.clone()
            }
        };
        // CUI resolves the live region and handles native placement/dismissal.
        // Drop model/menu borrows first: AppKit and Win32 invoke synchronously.
        let canvas = chat.part(0)?;
        canvas.canvas_focus_region(region)?;
        menu.popup_region(&canvas, region)?;
        Ok(())
    }
    fn message_menu_action(&self, action: Action) -> Result<()> {
        let target = self
            .message_menu
            .borrow_mut()
            .as_mut()
            .and_then(|m| m.target.take());
        let Some(target) = target else {
            return Ok(());
        };
        {
            let mut m = self.model.borrow_mut();
            if m.epoch != target.epoch
                || m.selected.as_ref() != Some(&target.room)
                || m.phase != Phase::Connected
                || m.message_draft.is_some()
                || m.tools_busy()
            {
                return Ok(());
            }
            m.message_selected = Some(target.event);
            if m.selected_message().is_none() {
                return Ok(());
            }
        }
        match action {
            Action::React => self.show_reactions(&target.chat, &target.anchor),
            Action::Reply => self.reply_message(),
            Action::Thread => self.open_thread(),
            Action::Forward => self.forward_message(),
            Action::Copy => {
                if let Some(message) = self.model.borrow().selected_message() {
                    self.window.clipboard_text(
                        &message.body.as_deref().unwrap_or("").replace('\0', "�"),
                    )?;
                }
                Ok(())
            }
            Action::Context => self.message_context(),
            Action::Details => {
                self.chat_tools(2)?;
                self.load_details()
            }
            Action::Edit => self.edit_message(),
            Action::Delete => self.delete_message(),
        }
    }
}
