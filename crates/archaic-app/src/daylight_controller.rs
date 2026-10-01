use crate::{Controller, daylight::Panel, model::Phase};
use cui::{Result, sys};
impl Controller {
    pub fn apply_daylight_theme(&self, appearance: crate::appearance::Appearance) -> Result<()> {
        let dark = self
            .app
            .upgrade()
            .map(|a| a.resolved_theme() == sys::CUI_THEME_DARK)
            .unwrap_or(appearance == crate::appearance::Appearance::Dark);
        self.view.ui.set_theme(dark)?;
        let t = self.view.ui.theme.borrow();
        crate::daylight::style(&self.view.login, &t, t.background, 0., 40)?;
        crate::daylight::style(&self.view.login_card, &t, t.surface, 24., 32)?;
        crate::daylight::style(&self.view.search, &t, t.selected, 12., 8)?;
        for w in [&self.view.ui.account_button, &self.view.security_toggle] {
            crate::daylight::style(w, &t, t.surface, 18., 8)?;
        }
        crate::daylight::style(&self.view.ui.close, &t, t.selected, 12., 6)?;
        crate::daylight::style(&self.view.ui.message_close, &t, t.selected, 12., 6)?;
        self.view.sign_in.set_style(Some(&sys::cui_widget_style {
            background: t.foreground,
            foreground: t.surface,
            radius: 12.,
            padding: 10,
            ..Default::default()
        }))?;
        self.view.timeline.set_theme(&t)?;
        self.view.thread_pane.theme(&t)?;
        self.view.emoji.theme(&t)?;
        self.view.reaction.theme(&t)?;
        self.view.message_controls.theme(&t)?;
        self.view.theme_forms(&t)?;
        self.view.timeline.render(&self.model.borrow(), &self.tr)
    }
    fn open_panel(&self, panel: Panel) -> Result<()> {
        self.remember_draft()?;
        self.view.ui.panel.set(panel);
        self.render()
    }
    pub fn account_panel(&self) -> Result<()> {
        self.open_panel(Panel::Account)
    }
    pub fn room_panel(&self) -> Result<()> {
        self.open_panel(Panel::Room)
    }
    pub fn history_panel(&self) -> Result<()> {
        self.open_panel(Panel::History)
    }
    pub fn toggle_inspector(&self) -> Result<()> {
        if self
            .view
            .ui
            .base
            .allocated_size()?
            .is_some_and(|s| s.0 < 1200)
        {
            return self.room_panel();
        }
        self.view
            .ui
            .inspector_open
            .set(!self.view.ui.inspector_open.get());
        self.view.ui.refresh(self.window.scale()?)
    }
    pub fn close_panel(&self) -> Result<()> {
        if self.view.emoji.target.borrow().is_some() || self.view.reaction.target.borrow().is_some()
        {
            return self.close_emoji();
        }
        if !self.view.ui.is_modal() {
            if self
                .model
                .borrow()
                .message_draft
                .as_ref()
                .is_some_and(|d| d.kind.inline())
            {
                return self.cancel_message();
            }
            if self.view.thread_pane.open.get() {
                return self.close_thread();
            }
            return Ok(());
        }
        self.remember_draft()?;
        {
            let m = self.model.borrow();
            if m.tools_busy()
                || m.creating
                || m.joining
                || m.sending
                || m.room_pending.is_some()
                || m.security.pending.is_some()
                || m.security.dialog
                || m.security.key.is_some()
                || m.organization.pending.is_some()
                || m.message_draft.as_ref().is_some_and(|d| d.pending)
            {
                return Ok(());
            }
        }
        if self.model.borrow().security.show {
            self.close_security()?;
        }
        {
            let mut m = self.model.borrow_mut();
            m.organization.show = false;
            m.show_new = false;
            m.show_join = false;
            m.tools.show = false;
            m.message_draft = None;
            m.room_confirmation = None;
        }
        self.view.ui.panel.set(Panel::None);
        self.render()?;
        if self.model.borrow().phase == Phase::Connected {
            self.view.composer.focus()?;
        } else {
            self.view.server.focus()?;
        }
        Ok(())
    }
    pub(crate) fn chat_tools(&self, mode: i32) -> Result<()> {
        self.remember_draft()?;
        if self.model.borrow().can_tool() {
            let mut m = self.model.borrow_mut();
            m.tools.show = true;
            m.tools.mode = mode;
        }
        self.render()?;
        if mode == 1 {
            self.view.tools.query.focus()?;
        }
        Ok(())
    }
    fn chat_interactive(&self) -> bool {
        !self.view.ui.is_modal()
            && self.view.emoji.target.borrow().is_none()
            && self.view.reaction.target.borrow().is_none()
            && self.model.borrow().phase == Phase::Connected
    }
    pub fn install_dialog_shortcut(
        self: &std::rc::Rc<Self>,
        app: &cui::App,
    ) -> Result<cui::Command> {
        let weak = std::rc::Rc::downgrade(self);
        app.command(
            &self.tr.text("cancel"),
            sys::CUI_KEY_ESCAPE as u32,
            0,
            move || {
                if let Some(c) = weak.upgrade() {
                    c.close_panel().expect("live dialog");
                }
            },
        )
    }
    pub fn chat_tick(&self) -> Result<()> {
        let mut emojis = std::mem::take(&mut *self.view.emoji.events.borrow_mut());
        emojis.extend(std::mem::take(&mut *self.view.reaction.events.borrow_mut()));
        if self.view.reaction.target.borrow().is_some()
            && !self
                .view
                .reaction
                .window
                .as_ref()
                .expect("reaction popup")
                .is_visible()?
        {
            self.close_emoji()?;
            emojis.clear();
        }
        for glyph in emojis {
            self.choose_emoji(&glyph)?;
        }
        let source_room = self.model.borrow().selected.clone();
        for e in self.view.ui.spaces.drain() {
            if self.chat_interactive() {
                self.chat_space(e.id)?;
            }
        }
        for e in self.view.ui.nav.drain() {
            if self.chat_interactive()
                && e.action == sys::CUI_CHAT_OPEN_ROOM
                && let Some(id) = self.chat_room_id(e.id)
            {
                self.select_room(id)?;
            }
        }
        for e in self.view.ui.header.drain() {
            if self.chat_interactive() {
                self.chat_header(e.detail)?;
            }
        }
        for e in self.view.ui.inspector.drain() {
            if self.chat_interactive() {
                self.chat_inspector(e)?;
            }
        }
        for e in self.view.timeline.chat.drain() {
            if self.chat_interactive() {
                self.chat_message(e)?;
            }
        }
        for e in self.view.ui.composer.drain() {
            if self.chat_interactive() && self.model.borrow().selected == source_room {
                self.chat_compose(e)?;
            }
        }
        for e in self.view.thread_pane.timeline.chat.drain() {
            if self.chat_interactive()
                && self.view.thread_pane.open.get()
                && let Some(id) = self.view.thread_pane.timeline.event_id(e.id)
            {
                self.chat_message_for(e, id, true)?;
            }
        }
        for e in self.view.thread_pane.composer.drain() {
            if self.chat_interactive() && self.view.thread_pane.open.get() {
                match e.action {
                    sys::CUI_CHAT_SEND => self.send_thread(&e.text)?,
                    sys::CUI_CHAT_EMOJI => self.show_emoji(crate::emoji_picker::Kind::Thread)?,
                    _ => {}
                }
            }
        }
        self.refresh_chat_surfaces()
    }
    fn refresh_chat_surfaces(&self) -> Result<()> {
        let dark = self
            .app
            .upgrade()
            .is_some_and(|a| a.resolved_theme() == sys::CUI_THEME_DARK);
        if dark != (self.view.ui.theme.borrow().background == 0x101c23ff) {
            self.apply_daylight_theme(crate::appearance::Appearance::System)?;
        }
        let scale = self.window.scale()?;
        self.view.ui.refresh(scale)?;
        if self.view.thread_pane.open.get() {
            self.view.ui.inspector.root.set_visible(false)?;
        }
        self.view.thread_pane.timeline.chat.refresh(scale)?;
        self.view.thread_pane.composer.refresh(scale)?;
        self.view.timeline.chat.refresh(scale)?;
        Ok(())
    }
    fn chat_room_id(&self, id: u64) -> Option<String> {
        let key = self.view.ui.ids.borrow().key(id)?;
        key.strip_prefix(&format!("{}:", self.model.borrow().epoch))
            .map(str::to_owned)
    }
    fn chat_space(&self, id: u64) -> Result<()> {
        if id == u64::MAX {
            self.view.search.set_text("")?;
            return self.filter();
        }
        if let Some(room) = self.chat_room_id(id) {
            self.toggle_organization()?;
            self.view.organization.mode.set_selected(2)?;
            self.view.organization.target.set_text(&room)?;
            if !self.fixture {
                self.organization_run()?;
            }
        }
        Ok(())
    }
    fn chat_header(&self, id: u64) -> Result<()> {
        match id {
            1 => self.chat_tools(1),
            2 => self.toggle_inspector(),
            _ => Ok(()),
        }
    }
    fn chat_inspector(&self, e: cui::ChatEvent) -> Result<()> {
        if e.action == sys::CUI_CHAT_OPEN_ROOM {
            return Ok(());
        }
        match e.id {
            4 => self.room_panel(),
            5 => self.toggle_organization(),
            6 => self.chat_tools(1),
            7 => self.chat_tools(0),
            8 => self.refresh(),
            9 => self.history_panel(),
            _ => Ok(()),
        }
    }
    fn chat_compose(&self, e: cui::ChatEvent) -> Result<()> {
        match e.action {
            sys::CUI_CHAT_SEND if !self.fixture => {
                self.view.composer.set_text(&e.text)?;
                if self
                    .model
                    .borrow()
                    .message_draft
                    .as_ref()
                    .is_some_and(|d| d.kind.inline())
                {
                    self.submit_message()?;
                } else {
                    self.send()?;
                }
            }
            sys::CUI_CHAT_CHANGED => self.composer_input()?,
            sys::CUI_CHAT_CANCEL => self.cancel_message()?,
            sys::CUI_CHAT_EMOJI => self.show_emoji(crate::emoji_picker::Kind::Compose)?,
            sys::CUI_CHAT_ATTACH => {
                self.chat_tools(0)?;
                if !self.fixture {
                    self.choose_file()?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn chat_message(&self, e: cui::ChatEvent) -> Result<()> {
        if self.view.ui.is_modal() {
            return Ok(());
        }
        if e.action == sys::CUI_CHAT_LOAD_OLDER {
            return self.older_history();
        }
        let Some(id) = self.view.timeline.event_id(e.id) else {
            return Ok(());
        };
        self.chat_message_for(e, id, false)
    }
    fn chat_message_for(&self, e: cui::ChatEvent, id: String, thread: bool) -> Result<()> {
        {
            let mut m = self.model.borrow_mut();
            if m.message_draft.is_some() || m.tools_busy() {
                return Ok(());
            }
            m.message_selected = Some(id);
            if m.selected_message().is_none() {
                return Ok(());
            }
        }
        match e.action {
            sys::CUI_CHAT_REPLY => self.reply_message()?,
            sys::CUI_CHAT_THREAD => self.open_thread()?,
            sys::CUI_CHAT_MORE => {
                let chat = if thread {
                    &self.view.thread_pane.timeline.chat
                } else {
                    &self.view.timeline.chat
                };
                self.open_message_menu(chat, &e)?;
            }
            sys::CUI_CHAT_COPY => {
                if let Some(message) = self.model.borrow().selected_message() {
                    self.window.clipboard_text(
                        &message.body.as_deref().unwrap_or("").replace('\0', "�"),
                    )?;
                }
            }
            sys::CUI_CHAT_REACT if e.text.is_empty() => {
                let chat = if thread {
                    &self.view.thread_pane.timeline.chat
                } else {
                    &self.view.timeline.chat
                };
                self.show_reactions(chat, &e)?
            }
            sys::CUI_CHAT_REACT => {
                self.view.ui.quick_reaction.set(true);
                self.react_message()?;
                if !self.fixture && !e.text.is_empty() && e.text.len() <= 64 {
                    self.view.message_controls.body.set_text(&e.text)?;
                    self.submit_message()?;
                }
            }
            sys::CUI_CHAT_ATTACHMENT => {
                self.chat_tools(2)?;
                self.load_details()?;
            }
            _ => {}
        }
        Ok(())
    }
}
