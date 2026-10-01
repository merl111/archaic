//! Daylight's geometry and native chat components, with application-owned dialogs.
use crate::{
    model::{Model, Phase},
    view::set_text,
};
use archaic_i18n::Translator;
use cui::{
    ChatCommand, ChatComponent, ChatKind, Result, Widget,
    chat::{Appearance, Avatar, NavItem, Theme},
    sys,
};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

/// IDs are never reused, including across profile switches. Queued native events
/// cannot accidentally address a different room after a sync/filter change.
#[derive(Default)]
pub struct Ids {
    next: u64,
    values: HashMap<String, u64>,
}
impl Ids {
    pub fn id(&mut self, key: &str) -> u64 {
        if let Some(id) = self.values.get(key) {
            return *id;
        }
        self.next += 1;
        self.values.insert(key.into(), self.next);
        self.next
    }
    pub fn key(&self, id: u64) -> Option<String> {
        self.values
            .iter()
            .find(|(_, v)| **v == id)
            .map(|(k, _)| k.clone())
    }
    pub fn retain(&mut self, keys: &[String]) {
        let keys = keys
            .iter()
            .map(String::as_str)
            .collect::<std::collections::HashSet<_>>();
        self.values.retain(|k, _| keys.contains(k.as_str()));
    }
}
pub fn text(value: &str, limit: usize) -> String {
    let mut value = value.replace('\0', "�");
    if value.len() > limit {
        let mut end = limit.saturating_sub(3);
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        value.truncate(end);
        value.push('…');
    }
    value
}
pub fn avatar(name: &str) -> Avatar {
    let hue = name
        .bytes()
        .fold(0u32, |n, b| n.wrapping_mul(31).wrapping_add(b as u32))
        % 360;
    Avatar {
        name: text(name, 1024),
        color: cui::chat_color(0.83, 0.12, hue as f64, 1.),
        ..Default::default()
    }
}
pub fn style(w: &Widget, t: &Theme, bg: u32, radius: f64, padding: i32) -> Result<()> {
    w.set_style(Some(&sys::cui_widget_style {
        background: bg,
        foreground: t.foreground,
        radius,
        padding,
        ..Default::default()
    }))?;
    Ok(())
}
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Panel {
    #[default]
    None,
    Account,
    Room,
    History,
}
pub struct Shell {
    brand: Widget,
    pub quick_reaction: Cell<bool>,
    pub base: Widget,
    pub body: Widget,
    pub layers: Widget,
    pub sheet_body: Widget,
    pub sidebar: Widget,
    pub main: Widget,
    pub top: Widget,
    pub spaces: ChatComponent,
    pub login_host: Widget,
    pub nav: ChatComponent,
    pub header: ChatComponent,
    pub composer: ChatComponent,
    pub inspector: ChatComponent,
    pub account_button: Widget,
    pub room_button: Widget,
    pub history_button: Widget,
    pub info_button: Widget,
    pub sheet: Widget,
    message_sheet: Widget,
    pub message_body: Widget,
    pub message_close: Widget,
    message_title: Widget,
    pub close: Widget,
    pub sheet_title: Widget,
    pub scrim: Widget,
    pub content: Widget,
    pub panel: Cell<Panel>,
    pub inspector_open: Cell<bool>,
    pub theme: RefCell<Theme>,
    pub ids: RefCell<Ids>,
    styled: RefCell<Vec<(Widget, bool, f64, i32)>>,
    modal: Cell<bool>,
    connected: Cell<bool>,
    compose_scope: RefCell<String>,
}
impl Shell {
    pub fn new(root: &Widget, tr: &Translator) -> Result<Self> {
        let t = Theme::new(Appearance::Daylight);
        root.box_set_padding(0)?;
        root.set_font("sans", 10.5, 400)?;
        style(root, &t, t.background, 0., 0)?;
        let layers = root.stack()?;
        let base = layers.stack_layer(sys::CUI_LAYER_FILL, 0, 0, 0)?;
        let page = base.box_layout(sys::CUI_VERTICAL, 0)?;
        page.expand(true)?;
        let top = page.box_layout(sys::CUI_HORIZONTAL, 12)?;
        style(&top, &t, t.background, 0., 6)?;
        top.set_min_size(1, 44)?;
        let brand = top.icon(Some(&crate::daylight_art::brand(&t)?))?;
        brand.set_icon_size(22)?;
        let name = top.label("Archaic")?;
        name.set_font("sans", 13.5, 800)?;
        let spaces = ChatComponent::create(&top, ChatKind::Spaces, &t)?;
        spaces.root.expand(false)?;
        spaces.part(0)?.set_min_size(110, 42)?;
        top.box_layout(sys::CUI_HORIZONTAL, 0)?.expand(true)?;
        let top_actions = top.box_layout(sys::CUI_HORIZONTAL, 8)?;
        let account_button =
            top.symbol_button(sys::CUI_SYMBOL_PEOPLE, &tr.text("daylight-account"))?;
        account_button.set_icon_only(false)?;
        style(&account_button, &t, t.surface, 18., 8)?;
        let login_host = page.box_layout(sys::CUI_VERTICAL, 0)?;
        login_host.expand(true)?;
        let content = page.box_layout(sys::CUI_VERTICAL, 0)?;
        content.expand(true)?;
        let margin = content.box_layout(sys::CUI_HORIZONTAL, 0)?;
        margin.expand(true)?;
        margin
            .box_layout(sys::CUI_VERTICAL, 0)?
            .set_min_size(12, 1)?;
        let body = margin.box_layout(sys::CUI_HORIZONTAL, 12)?;
        body.expand(true)?;
        margin
            .box_layout(sys::CUI_VERTICAL, 0)?
            .set_min_size(12, 1)?;
        content
            .box_layout(sys::CUI_HORIZONTAL, 0)?
            .set_min_size(1, 12)?;
        let sidebar = body.box_layout(sys::CUI_VERTICAL, 0)?;
        sidebar.set_min_size(300, 1)?;
        style(&sidebar, &t, t.surface, 20., 0)?;
        let sidebar_head = sidebar.box_layout(sys::CUI_VERTICAL, 10)?;
        sidebar_head.box_set_padding(16)?;
        let nav = ChatComponent::create(&sidebar, ChatKind::Rooms, &t)?;
        let main = body.box_layout(sys::CUI_VERTICAL, 0)?;
        main.expand(true)?;
        style(&main, &t, t.surface, 20., 0)?;
        let header = ChatComponent::create(&main, ChatKind::Header, &t)?;
        header.root.expand(false)?;
        header.part(0)?.set_min_size(1, 72)?;
        header.set_commands(&[
            ChatCommand {
                id: 1,
                label: tr.text("tool-search"),
                symbol: sys::CUI_SYMBOL_SEARCH,
                action: sys::CUI_CHAT_MORE,
                ..Default::default()
            },
            ChatCommand {
                id: 2,
                label: tr.text("daylight-about"),
                symbol: sys::CUI_SYMBOL_PANEL,
                action: sys::CUI_CHAT_MORE,
                ..Default::default()
            },
        ])?;
        // Composer is created in View after the timeline; this temporary parent
        // is the main conversation body, where child order is explicit.
        let composer_parent = main.box_layout(sys::CUI_VERTICAL, 0)?;
        composer_parent.expand(true)?;
        let composer = ChatComponent::create(&main, ChatKind::Composer, &t)?;
        let inspector = ChatComponent::create(&body, ChatKind::Inspector, &t)?;
        inspector.root.expand(false)?;
        inspector.root.set_min_size(300, 1)?;
        inspector.set_commands(&[ChatCommand {
            id: 1,
            label: tr.text("daylight-about"),
            action: sys::CUI_CHAT_MORE,
            ..Default::default()
        }])?;
        let actions = top.box_layout(sys::CUI_HORIZONTAL, 6)?;
        let room_button = actions.symbol_button(sys::CUI_SYMBOL_MENU, &tr.text("room-tools"))?;
        let history_button =
            actions.symbol_button(sys::CUI_SYMBOL_REPEAT, &tr.text("timeline-messages"))?;
        let info_button =
            actions.symbol_button(sys::CUI_SYMBOL_PANEL, &tr.text("daylight-about"))?;
        for b in [&room_button, &history_button, &info_button] {
            style(b, &t, t.surface, 16., 6)?;
            b.set_visible(false)?;
        }
        let scrim = layers.stack_layer(sys::CUI_LAYER_FILL, 0, 0, 0)?;
        style(&scrim, &t, 0x15253055, 0., 0)?;
        scrim.set_visible(false)?;
        let sheet = layers.stack_layer(sys::CUI_LAYER_CENTER, 860, 0, 24)?;
        style(&sheet, &t, t.surface, 24., 24)?;
        let sh = sheet.box_layout(sys::CUI_HORIZONTAL, 12)?;
        let sheet_title = sh.label("")?;
        sheet_title.set_font("sans", 16.5, 700)?;
        sheet_title.expand(true)?;
        let close = sh.symbol_button(sys::CUI_SYMBOL_CLOSE, &tr.text("cancel"))?;
        style(&close, &t, t.selected, 12., 6)?;
        sheet
            .box_layout(sys::CUI_VERTICAL, 0)?
            .set_min_size(1, 20)?;
        let sheet_body = sheet.box_layout(sys::CUI_VERTICAL, 16)?;
        sheet.set_visible(false)?;
        let message_sheet = layers.stack_layer(sys::CUI_LAYER_CENTER, 540, 0, 24)?;
        style(&message_sheet, &t, t.surface, 24., 24)?;
        let message_content = message_sheet.box_layout(sys::CUI_VERTICAL, 20)?;
        let message_head = message_content.box_layout(sys::CUI_HORIZONTAL, 12)?;
        let message_title = message_head.label("")?;
        message_title.set_font("sans", 16.5, 700)?;
        message_title.expand(true)?;
        let message_close =
            message_head.symbol_button(sys::CUI_SYMBOL_CLOSE, &tr.text("cancel"))?;
        let message_body = message_content.box_layout(sys::CUI_VERTICAL, 16)?;
        message_sheet.set_visible(false)?;
        let styled = vec![
            (message_sheet.clone(), true, 24., 24),
            (root.clone(), false, 0., 0),
            (sidebar.clone(), true, 20., 0),
            (main.clone(), true, 20., 0),
            (top.clone(), false, 0., 6),
            (sheet.clone(), true, 24., 24),
        ];
        Ok(Self {
            brand,
            quick_reaction: Cell::new(false),
            base,
            body,
            layers,
            sheet_body,
            sidebar: sidebar_head,
            main: composer_parent,
            top: top_actions,
            spaces,
            login_host,
            nav,
            header,
            composer,
            inspector,
            account_button,
            room_button,
            history_button,
            info_button,
            sheet,
            message_sheet,
            message_body,
            message_close,
            message_title,
            close,
            sheet_title,
            scrim,
            content,
            panel: Cell::new(Panel::None),
            inspector_open: Cell::new(true),
            theme: RefCell::new(t),
            ids: Default::default(),
            styled: RefCell::new(styled),
            modal: Cell::new(false),
            connected: Cell::new(false),
            compose_scope: Default::default(),
        })
    }
    pub fn card(&self, parent: &Widget, radius: f64, padding: i32) -> Result<Widget> {
        let card = parent.box_layout(sys::CUI_VERTICAL, 12)?;
        style(
            &card,
            &self.theme.borrow(),
            self.theme.borrow().surface,
            radius,
            padding,
        )?;
        self.styled
            .borrow_mut()
            .push((card.clone(), true, radius, padding));
        Ok(card)
    }
    pub fn set_theme(&self, dark: bool) -> Result<()> {
        let mut t = Theme::new(Appearance::Daylight);
        t.accent = t.foreground;
        t.on_accent = t.surface;
        if dark {
            t.background = 0x101c23ff;
            t.surface = 0x1b2932ff;
            t.foreground = 0xe7f0f5ff;
            t.muted = 0xa6bbc8ff;
            t.border = 0x344b58ff;
            t.selected = 0x293f4bff;
            t.accent = 0x99dbecff;
            t.on_accent = 0x112631ff;
        }
        self.brand
            .set_icon(Some(&crate::daylight_art::brand(&t)?))?;
        for (w, surface, r, p) in self.styled.borrow().iter() {
            style(
                w,
                &t,
                if *surface { t.surface } else { t.background },
                *r,
                *p,
            )?;
        }
        for c in [
            &self.spaces,
            &self.nav,
            &self.header,
            &self.composer,
            &self.inspector,
        ] {
            c.set_theme(&t)?;
        }
        *self.theme.borrow_mut() = t;
        Ok(())
    }
    pub fn render(&self, m: &Model, tr: &Translator) -> Result<()> {
        let scope = format!("{}:{}", m.epoch, m.selected.as_deref().unwrap_or(""));
        if *self.compose_scope.borrow() != scope {
            self.composer.drain();
            *self.compose_scope.borrow_mut() = scope;
        }
        self.connected.set(matches!(
            m.phase,
            Phase::Connected | Phase::SigningOut | Phase::LogoutCleanup
        ));
        let mut ids = self.ids.borrow_mut();
        let all_rooms = m.visible_rooms();
        let rooms = all_rooms
            .into_iter()
            .filter(|r| !r.info.space)
            .collect::<Vec<_>>();
        let keys: Vec<_> = m
            .rooms
            .iter()
            .map(|r| format!("{}:{}", m.epoch, r.id))
            .collect();
        ids.retain(&keys);
        let items: Vec<_> = rooms
            .iter()
            .map(|r| NavItem {
                id: ids.id(&format!("{}:{}", m.epoch, r.id)),
                title: text(&r.name, 1024),
                avatar: Avatar {
                    square: true,
                    ..avatar(&r.name)
                },
                group: tr.text(if r.membership == archaic_matrix::Membership::Invited {
                    "invitation"
                } else {
                    "rooms"
                }),
                detail: if r.membership == archaic_matrix::Membership::Invited {
                    tr.text("invitation")
                } else {
                    String::new()
                },
                unread: m
                    .activity
                    .unread
                    .get(&r.id)
                    .copied()
                    .unwrap_or(0)
                    .min(u32::MAX as u64) as u32,
                ..Default::default()
            })
            .collect();
        let mut spaces = vec![NavItem {
            id: u64::MAX,
            title: tr.text("daylight-home"),
            symbol: sys::CUI_SYMBOL_HOME,
            ..Default::default()
        }];
        spaces.extend(
            m.rooms
                .iter()
                .filter(|r| r.info.space)
                .take(8)
                .map(|r| NavItem {
                    id: ids.id(&format!("{}:{}", m.epoch, r.id)),
                    title: text(&r.name, 80),
                    avatar: avatar(&r.name),
                    ..Default::default()
                }),
        );
        self.spaces.part(0)?.set_min_size(
            (110 + (spaces.len().saturating_sub(1) * 160)).min(640) as i32,
            42,
        )?;
        self.spaces.rooms(&spaces)?;
        self.spaces.select(u64::MAX)?;
        self.nav.rooms(&items)?;
        let selected = m
            .selected
            .as_ref()
            .filter(|id| rooms.iter().any(|r| &r.id == *id))
            .map(|id| ids.id(&format!("{}:{id}", m.epoch)))
            .unwrap_or(0);
        self.nav.select(selected)?;
        let room = m.selected_room();
        let title = room
            .map(|r| text(&r.name, 1024))
            .unwrap_or_else(|| tr.text("choose-room"));
        self.header.rooms(&[NavItem {
            id: 1,
            title: title.clone(),
            detail: room.map(|r| text(&r.info.topic, 1024)).unwrap_or_default(),
            avatar: Avatar {
                square: true,
                ..avatar(&title)
            },
            ..Default::default()
        }])?;
        let mut about = vec![NavItem {
            id: 1,
            title: title.clone(),
            detail: room.map(|r| text(&r.info.topic, 1024)).unwrap_or_default(),
            avatar: Avatar {
                square: true,
                ..avatar(&title)
            },
            ..Default::default()
        }];
        if room.is_some() {
            about.extend([
                NavItem {
                    id: 2,
                    title: tr.text("daylight-members"),
                    detail: room.map(|r| r.info.members.to_string()).unwrap_or_default(),
                    ..Default::default()
                },
                NavItem {
                    id: 3,
                    title: tr.text("daylight-encryption"),
                    detail: tr.text(match room.and_then(|r| r.info.encrypted) {
                        Some(true) => "daylight-encrypted",
                        Some(false) => "daylight-unencrypted",
                        None => "daylight-unknown",
                    }),
                    ..Default::default()
                },
                NavItem {
                    id: 4,
                    title: tr.text("room-tools"),
                    symbol: sys::CUI_SYMBOL_MENU,
                    ..Default::default()
                },
                NavItem {
                    id: 5,
                    title: tr.text("organization"),
                    symbol: sys::CUI_SYMBOL_PEOPLE,
                    ..Default::default()
                },
                NavItem {
                    id: 6,
                    title: tr.text("tool-search"),
                    symbol: sys::CUI_SYMBOL_SEARCH,
                    ..Default::default()
                },
                NavItem {
                    id: 7,
                    title: tr.text("tool-files"),
                    symbol: sys::CUI_SYMBOL_ATTACH,
                    ..Default::default()
                },
                NavItem {
                    id: 8,
                    title: tr.text("refresh"),
                    symbol: sys::CUI_SYMBOL_REPEAT,
                    ..Default::default()
                },
                NavItem {
                    id: 9,
                    title: tr.text("timeline-messages"),
                    symbol: sys::CUI_SYMBOL_THREAD,
                    ..Default::default()
                },
            ]);
        }
        self.inspector.rooms(&about)?;
        set_text(
            &self.account_button,
            &if m.user.is_empty() {
                tr.text("daylight-account")
            } else {
                text(m.user.split(':').next().unwrap_or(&m.user), 40)
            },
        )?;
        self.account_button.set_tooltip(&m.user)?;
        self.room_button.set_enabled(room.is_some())?;
        self.history_button.set_enabled(room.is_some())?;
        self.content.set_visible(matches!(
            m.phase,
            Phase::Connected | Phase::SigningOut | Phase::LogoutCleanup
        ))?;
        Ok(())
    }
    pub fn modal(&self, show: bool, title: &str, compact: bool) -> Result<()> {
        self.base.set_enabled(!show)?;
        self.scrim.set_visible(show)?;
        self.sheet.set_visible(show && !compact)?;
        self.message_sheet.set_visible(show && compact)?;
        set_text(&self.message_title, title)?;
        set_text(&self.sheet_title, title)?;
        let before = self.modal.replace(show);
        if show && !before {
            if compact {
                self.message_close.focus()?;
            } else {
                self.close.focus()?;
            }
        }
        if !show && before {
            self.composer.part(0)?.focus()?;
        }
        Ok(())
    }
    pub fn is_modal(&self) -> bool {
        self.modal.get()
    }
    pub fn refresh(&self, scale: f64) -> Result<()> {
        let width = self.base.allocated_size()?.map(|s| s.0).unwrap_or(1440);
        self.inspector
            .root
            .set_visible(self.inspector_open.get() && width >= 1200)?;
        self.spaces
            .root
            .set_visible(width >= 1280 && self.connected.get())?;
        self.room_button
            .set_visible(width < 1200 && self.connected.get())?;
        self.history_button
            .set_visible(width < 1200 && self.connected.get())?;
        for c in [
            &self.spaces,
            &self.nav,
            &self.header,
            &self.composer,
            &self.inspector,
        ] {
            c.refresh(scale)?;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_server_text_is_valid_utf8_and_nul_free() {
        let s = text(&"é\0🦀".repeat(100), 64);
        assert!(s.len() <= 64);
        assert!(!s.contains('\0'));
    }
    #[test]
    fn stale_ids_never_resolve_to_new_rooms() {
        let mut ids = Ids::default();
        let old = ids.id("1:!a:x");
        ids.retain(&[]);
        let new = ids.id("2:!a:x");
        assert_ne!(old, new);
        assert!(ids.key(old).is_none());
        assert_eq!(ids.id("2:!a:x"), new);
    }
}
