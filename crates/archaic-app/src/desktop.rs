//! Native notifications and safe Matrix-link presentation. No network work on the GUI thread.
use crate::{Controller, model::Phase};
use cui::Result;
impl Controller {
    pub fn set_notifications(&self, enabled: bool) -> Result<()> {
        let previous = self.rendering.replace(true);
        let result = self.view.notifications.set_checked(enabled);
        self.rendering.set(previous);
        result
    }
    pub fn notification_preference(&self) -> Result<()> {
        let enabled = self.view.notifications.get_checked()?;
        if !enabled {
            self.notification_sender.invalidate();
        }
        if let Some(path) = &self.preference_path
            && crate::preferences::save_notifications(
                &path.with_file_name("desktop.json"),
                &self.active_profile.borrow(),
                enabled,
            )
            .is_err()
        {
            self.model.borrow_mut().error = Some("desktop-preference-failed");
            self.render()?;
        }
        Ok(())
    }
    pub fn refresh_profiles(&self) -> Result<()> {
        if self.model.borrow().temporary || self.fixture {
            return Ok(());
        }
        let names =
            archaic_matrix::storage::profile_names().unwrap_or_else(|_| vec!["default".into()]);
        let previous = self.rendering.replace(true);
        let result = (|| {
            let labels: Vec<String> = names
                .iter()
                .map(|name| {
                    if name == "default" {
                        self.tr.text("account-main")
                    } else {
                        name.clone()
                    }
                })
                .collect();
            self.view
                .profiles
                .set_items(&labels.iter().map(String::as_str).collect::<Vec<_>>())?;
            let index = names
                .iter()
                .position(|n| n == &*self.active_profile.borrow());
            self.view
                .profiles
                .set_selected(index.map_or(-1, |i| i as i32))?;
            *self.profile_names.borrow_mut() = names;
            Ok(())
        })();
        self.rendering.set(previous);
        result
    }
    pub fn choose_profile(&self) -> Result<()> {
        let index = self.view.profiles.get_selected()?;
        let names = self.profile_names.borrow().clone();
        if let Some(name) = usize::try_from(index).ok().and_then(|i| names.get(i)) {
            self.view.profile.set_text(name)?;
            self.switch_profile()?;
        }
        Ok(())
    }

    pub fn desktop_open(&self, open: crate::instance::Open) -> Result<()> {
        self.window.show()?;
        if let Some(profile) = open.profile
            && profile != *self.active_profile.borrow()
        {
            self.view.profile.set_text(&profile)?;
            self.switch_profile()?;
        }
        if let Some(link) = open.link {
            self.view.deep_link.set_text(&link)?;
            if self.model.borrow().phase == Phase::Connected
                && self.pending_profile.borrow().is_none()
            {
                self.open_matrix_link()?;
            } else {
                self.startup_link.set(true);
            }
        }
        Ok(())
    }
    pub fn open_matrix_link(&self) -> Result<()> {
        {
            let m = self.model.borrow();
            if self.fixture
                || m.phase != Phase::Connected
                || m.tools_busy()
                || m.message_draft.is_some()
                || m.security.pending.is_some()
            {
                return Ok(());
            }
        }
        let text = self.view.deep_link.text()?;
        match archaic_matrix::links::parse(text.trim()) {
            Ok(archaic_matrix::links::Target::Room {
                address,
                event,
                via,
            }) => {
                self.remember_draft()?;
                let joined = self
                    .model
                    .borrow()
                    .rooms
                    .iter()
                    .find(|r| r.id == address && r.membership == archaic_matrix::Membership::Joined)
                    .cloned();
                if let Some(room) = joined {
                    if self.enqueue(archaic_matrix::Command::Select(room.id.clone())) {
                        let mut m = self.model.borrow_mut();
                        m.selected = Some(room.id.clone());
                        m.clear_history();
                        drop(m);
                        if let Some(event_id) = event {
                            self.enqueue(archaic_matrix::Command::Context {
                                room_id: room.id,
                                event_id,
                            });
                        }
                    }
                } else {
                    self.model.borrow_mut().show_join = true;
                    self.model.borrow_mut().link_join = Some((address.clone(), via, event));
                    self.view.join.join_address.set_text(&address)?;
                }
            }
            Ok(archaic_matrix::links::Target::User(user)) => {
                self.model.borrow_mut().show_new = true;
                self.view.new_conversation.person.set_text(&user)?;
            }
            Err(key) => self.model.borrow_mut().error = Some(key),
        }
        self.render()
    }
    pub fn desktop_notification(
        &self,
        epoch: u64,
        room_id: &str,
        name: &str,
        event_id: &str,
    ) -> Result<()> {
        let m = self.model.borrow();
        if self.fixture
            || epoch != m.epoch
            || m.phase != Phase::Connected
            || !self.view.notifications.get_checked()?
        {
            return Ok(());
        }
        if m.selected.as_deref() == Some(room_id) && self.view.composer.has_focus()? {
            return Ok(());
        }
        let body = format!(
            "{}: {}",
            self.tr.text("notification-activity"),
            name.chars()
                .filter(|c| !c.is_control())
                .take(120)
                .collect::<String>()
        );
        let room = archaic_matrix::matrix_event_link(room_id, event_id);
        if let Ok(link) = room {
            self.notification_sender.send(
                body,
                crate::instance::Open {
                    link: Some(link),
                    profile: Some(self.active_profile.borrow().clone()),
                },
            );
        }
        Ok(())
    }
}

impl Controller {
    pub fn desktop_ready(&self) -> bool {
        let m = self.model.borrow();
        matches!(
            m.phase,
            Phase::Connected | Phase::SignedOut | Phase::RestoreBlocked
        ) && !m.tools_busy()
            && m.security.pending.is_none()
            && m.organization.pending.is_none()
            && m.message_draft.is_none()
            && !m.sending
            && !m.creating
            && !m.joining
            && !m.browser_login
    }
    pub fn drafts_dirty(&self) -> bool {
        let m = self.model.borrow();
        m.drafts
            .iter()
            .any(|(id, body)| m.saved_drafts.get(id).map(String::as_str).unwrap_or("") != body)
    }
}

impl Controller {
    pub fn show_shortcuts(&self) -> Result<()> {
        let primary = if cfg!(target_os = "macos") {
            "⌘"
        } else {
            "Ctrl+"
        };
        let message = [
            ("shortcut-rooms", "K"),
            ("new-conversation", "N"),
            ("join-room", "J"),
            ("tool-search", "F"),
            ("refresh", "R"),
            ("organization", "Shift+O"),
            ("security-title", "Shift+S"),
            ("open-link", "L"),
        ]
        .iter()
        .map(|(key, shortcut)| format!("{primary}{shortcut}   {}", self.tr.text(key)))
        .collect::<Vec<_>>()
        .join("\n");
        let button = self.view.shortcuts.clone();
        self.window.alert(
            &self.tr.text("keyboard-shortcuts"),
            &message,
            &self.tr.text("cancel"),
            move |_, _, _| {
                let _ = button.focus();
            },
        )?;
        Ok(())
    }
    pub fn install_shortcuts(self: &std::rc::Rc<Self>, app: &cui::App) -> Result<()> {
        use cui::sys::{CUI_MOD_PRIMARY, CUI_MOD_SHIFT};
        let bar = app.menu()?;
        let navigation = app.menu()?;
        for (label, key, modifiers, action) in [
            ("shortcut-rooms", 'k', CUI_MOD_PRIMARY, 0),
            ("new-conversation", 'n', CUI_MOD_PRIMARY, 1),
            ("join-room", 'j', CUI_MOD_PRIMARY, 2),
            ("tool-search", 'f', CUI_MOD_PRIMARY, 3),
            ("refresh", 'r', CUI_MOD_PRIMARY, 4),
            ("organization", 'o', CUI_MOD_PRIMARY | CUI_MOD_SHIFT, 5),
            ("security-title", 's', CUI_MOD_PRIMARY | CUI_MOD_SHIFT, 6),
            ("open-link", 'l', CUI_MOD_PRIMARY, 7),
        ] {
            let weak = std::rc::Rc::downgrade(self);
            let command = app.command(
                &self.tr.text(label),
                key as u32,
                modifiers as u32,
                move || {
                    if let Some(c) = weak.upgrade() {
                        c.shortcut(action).expect("live desktop controls");
                    }
                },
            )?;
            navigation.add(&command)?;
            self.desktop_commands.borrow_mut().push(command);
        }
        bar.submenu(&self.tr.text("shortcut-navigation"), &navigation)?;
        // macOS owns the menu in the system menu bar. On other desktops the
        // visible navigation belongs in the Daylight toolbar; shortcuts remain
        // registered at the application level without an extra menu row.
        if cfg!(target_os = "macos") {
            self.window.set_menu(Some(&bar))?;
        }
        Ok(())
    }
    fn shortcut(&self, action: u8) -> Result<()> {
        if self.view.ui.is_modal() || self.view.emoji.target.borrow().is_some() {
            return Ok(());
        }
        {
            let m = self.model.borrow();
            if m.phase != Phase::Connected
                || m.tools_busy()
                || m.security.pending.is_some()
                || m.organization.pending.is_some()
                || m.message_draft.is_some()
                || m.sending
                || m.joining
                || m.creating
            {
                return Ok(());
            }
        }
        match action {
            0 => {
                self.view.ui.sidebar_open.set(true);
                self.render()?;
                self.refresh_chat_surfaces()?;
                self.view.search.focus()?;
            }
            1 => self.toggle_new()?,
            2 => self.toggle_join()?,
            3 => {
                if self.model.borrow().can_tool() {
                    self.model.borrow_mut().tools.show = true;
                    self.model.borrow_mut().tools.mode = 1;
                    self.render()?;
                    self.view.tools.query.focus()?;
                }
            }
            4 => self.refresh()?,
            5 => self.toggle_organization()?,
            6 => self.open_security()?,
            7 => {
                self.account_panel()?;
                self.view.deep_link.focus()?;
            }
            _ => {}
        }
        Ok(())
    }
}
