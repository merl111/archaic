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
    Composer,
}
pub struct Shell {
    brand: Widget,
    top_spacer: Widget,
    pub quick_reaction: Cell<bool>,
    pub base: Widget,
    pub body: Widget,
    pub layers: Widget,
    pub sheet_body: Widget,
    pub sidebar: Widget,
    pub sidebar_header: Widget,
    pub sidebar_surface: Widget,
    pub sidebar_toggle: Widget,
    pub left_split: Widget,
    pub right_split: Widget,
    pub left_pane: Widget,
    pub right_pane: Widget,
    pub sidebar_open: Cell<bool>,
    pub left_fraction: Cell<f64>,
    pub right_fraction: Cell<f64>,
    pub main: Widget,
    pub top: Widget,
    pub spaces: ChatComponent,
    pub space_picker: Widget,
    pub space_ids: RefCell<Vec<u64>>,
    space_width: Cell<f64>,
    pub login_host: Widget,
    pub nav: ChatComponent,
    pub header: ChatComponent,
    pub composer: ChatComponent,
    pub inspector: ChatComponent,
    pub account_button: Widget,
    pub room_button: Widget,
    pub history_button: Widget,
    pub info_button: Widget,
    pub favorite_button: Widget,
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
    focus_return: RefCell<Option<Widget>>,
    pub text_scale: Cell<f64>,
    pub sidebar_controls: RefCell<Vec<Widget>>,
    sidebar_wide: Cell<bool>,
    sidebar_before_narrow: Cell<bool>,
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
        crate::chat_labels::apply(&spaces, tr)?;
        let space_picker = page.select(&[])?;
        space_picker.accessibility(&tr.text("navigation-spaces"), "")?;
        space_picker.set_visible(false)?;
        spaces.root.expand(true)?;
        name.set_min_size(90, 1)?;
        spaces.part(0)?.set_min_size(110, 42)?;
        let top_spacer = top.box_layout(sys::CUI_HORIZONTAL, 0)?;
        top_spacer.expand(true)?;
        let top_actions = top.box_layout(sys::CUI_HORIZONTAL, 8)?;
        let account_button =
            top.symbol_button(sys::CUI_SYMBOL_SETTINGS, &tr.text("settings-title"))?;
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
        let left_split = body.split(sys::CUI_HORIZONTAL, 0.21)?;
        left_split.expand(true)?;
        let left_pane = left_split.split_pane(0)?;
        let sidebar = left_pane.box_layout(sys::CUI_VERTICAL, 0)?;
        sidebar.expand(true)?;
        sidebar.set_min_size(200, 1)?;
        let right_split = left_split.split_pane(1)?.split(sys::CUI_HORIZONTAL, 0.73)?;
        right_split.expand(true)?;
        let right_pane = right_split.split_pane(1)?;
        style(&sidebar, &t, t.surface, 20., 0)?;
        let sidebar_head = sidebar.box_layout(sys::CUI_VERTICAL, 10)?;
        sidebar_head.box_set_padding(16)?;
        let sidebar_header = sidebar_head.box_layout(sys::CUI_HORIZONTAL, 8)?;
        let sidebar_toggle =
            sidebar_header.symbol_button(sys::CUI_SYMBOL_PANEL, &tr.text("sidebar-toggle"))?;
        style(&sidebar_toggle, &t, t.surface, 12., 8)?;
        let nav = ChatComponent::create(&sidebar, ChatKind::Rooms, &t)?;
        crate::chat_labels::apply(&nav, tr)?;
        let main = right_split
            .split_pane(0)?
            .box_layout(sys::CUI_VERTICAL, 0)?;
        main.set_min_size(380, 1)?;
        main.expand(true)?;
        style(&main, &t, t.surface, 20., 0)?;
        let header = ChatComponent::create(&main, ChatKind::Header, &t)?;
        crate::chat_labels::apply(&header, tr)?;
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
        crate::chat_labels::apply(&composer, tr)?;
        let inspector = ChatComponent::create(&right_pane, ChatKind::Inspector, &t)?;
        crate::chat_labels::apply(&inspector, tr)?;
        inspector.root.expand(true)?;
        inspector.root.set_min_size(220, 1)?;
        inspector.set_commands(&[ChatCommand {
            id: 1,
            label: tr.text("daylight-about"),
            action: sys::CUI_CHAT_MORE,
            ..Default::default()
        }])?;
        let favorite_button =
            top_actions.symbol_button(sys::CUI_SYMBOL_HEART, &tr.text("org-favorite"))?;
        style(&favorite_button, &t, t.surface, 12., 8)?;
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
        let scrim = layers.stack_backdrop(&tr.text("cancel"))?;
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
            top_spacer,
            quick_reaction: Cell::new(false),
            base,
            body,
            layers,
            sheet_body,
            sidebar: sidebar_head,
            sidebar_header,
            sidebar_surface: sidebar,
            sidebar_toggle,
            left_split,
            right_split,
            left_pane,
            right_pane,
            sidebar_open: Cell::new(true),
            left_fraction: Cell::new(0.21),
            right_fraction: Cell::new(0.73),
            main: composer_parent,
            top: top_actions,
            spaces,
            space_picker,
            space_ids: Default::default(),
            space_width: Cell::new(0.),
            login_host,
            nav,
            header,
            composer,
            inspector,
            account_button,
            room_button,
            history_button,
            info_button,
            favorite_button,
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
            inspector_open: Cell::new(false),
            theme: RefCell::new(t),
            ids: Default::default(),
            styled: RefCell::new(styled),
            modal: Cell::new(false),
            focus_return: Default::default(),
            text_scale: Cell::new(1.),
            sidebar_controls: Default::default(),
            sidebar_wide: Cell::new(true),
            sidebar_before_narrow: Cell::new(true),
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
            t.hover = 0x314754ff;
            t.rail = t.background;
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
        style(
            &self.scrim,
            &t,
            if dark { 0x000000a6 } else { 0x15253070 },
            0.,
            0,
        )?;
        for sheet in [&self.sheet, &self.message_sheet] {
            sheet.set_style(Some(&sys::cui_widget_style {
                background: t.surface,
                foreground: t.foreground,
                border: if dark { 0x526a78ff } else { 0xb6c8d5ff },
                border_width: 1.,
                radius: 24.,
                padding: 24,
            }))?;
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
        let mut rooms = all_rooms
            .into_iter()
            .filter(|r| !r.info.space)
            .collect::<Vec<_>>();
        rooms.sort_by(|a, b| {
            let group = |r: &&archaic_matrix::RoomSummary| {
                (
                    r.membership != archaic_matrix::Membership::Invited,
                    !r.info.favourite,
                    !r.info.favourite && !r.info.direct,
                )
            };
            group(a)
                .cmp(&group(b))
                .then_with(|| b.info.recent.cmp(&a.info.recent))
                .then_with(|| a.name.cmp(&b.name))
                .then_with(|| a.id.cmp(&b.id))
        });
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
                    square: !r.info.direct,
                    image: m.avatars.get(&format!("room:{}", r.id)).cloned(),
                    ..avatar(&r.name)
                },
                group: tr.text(if r.membership == archaic_matrix::Membership::Invited {
                    "invitation"
                } else if r.info.favourite {
                    "favorites"
                } else if r.info.direct {
                    "direct-messages"
                } else {
                    "rooms"
                }),
                detail: room_preview(r, m, tr),
                trailing: crate::timeline_view::room_time(r.info.recent),
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
        spaces.push(NavItem {
            id: u64::MAX - 1,
            title: tr.text("direct-messages"),
            symbol: sys::CUI_SYMBOL_PEOPLE,
            ..Default::default()
        });
        spaces.extend(
            m.rooms
                .iter()
                .filter(|r| r.info.space)
                .take(8)
                .map(|r| NavItem {
                    id: ids.id(&format!("{}:{}", m.epoch, r.id)),
                    title: text(&r.name, 80),
                    avatar: Avatar {
                        image: m.avatars.get(&format!("room:{}", r.id)).cloned(),
                        ..avatar(&r.name)
                    },
                    ..Default::default()
                }),
        );
        let width = spaces.iter().try_fold(4.0, |width, item| -> Result<f64> {
            Ok(width + cui::measure_text(&item.title, "sans", 13., 600)?.0 + 56.)
        })?;
        self.space_width.set(width.ceil());
        self.space_picker
            .set_items(&spaces.iter().map(|s| s.title.as_str()).collect::<Vec<_>>())?;
        *self.space_ids.borrow_mut() = spaces.iter().map(|s| s.id).collect();
        self.spaces.rooms(&spaces)?;
        let active_space = if m.direct_only {
            u64::MAX - 1
        } else {
            m.active_space
                .as_ref()
                .map(|id| ids.id(&format!("{}:{id}", m.epoch)))
                .unwrap_or(u64::MAX)
        };
        self.spaces.select(active_space)?;
        self.space_picker.set_selected(
            spaces
                .iter()
                .position(|s| s.id == active_space)
                .map_or(0, |i| i as i32),
        )?;
        self.nav.rooms(&items)?;
        let selected = m
            .selected
            .as_ref()
            .filter(|id| rooms.iter().any(|r| &r.id == *id))
            .map(|id| ids.id(&format!("{}:{id}", m.epoch)))
            .unwrap_or(0);
        self.nav.select(selected)?;
        let room = m.selected_room();
        self.favorite_button.set_visible(room.is_some())?;
        self.favorite_button
            .set_enabled(m.organization.pending.is_none())?;
        let favorite = room.is_some_and(|r| r.info.favourite);
        self.favorite_button
            .set_icon(Some(&cui::Icon::symbol(if favorite {
                sys::CUI_SYMBOL_HEART_FILLED
            } else {
                sys::CUI_SYMBOL_HEART
            })?))?;
        self.favorite_button.set_text(&tr.text(if favorite {
            "org-unfavorite"
        } else {
            "org-favorite"
        }))?;
        let title = room
            .map(|r| text(&r.name, 1024))
            .unwrap_or_else(|| tr.text("choose-room"));
        self.header.rooms(&[NavItem {
            id: 1,
            title: title.clone(),
            detail: room.map(|r| text(&r.info.topic, 1024)).unwrap_or_default(),
            avatar: Avatar {
                square: room.is_none_or(|r| !r.info.direct),
                image: room
                    .and_then(|r| m.avatars.get(&format!("room:{}", r.id)))
                    .cloned(),
                ..avatar(&title)
            },
            ..Default::default()
        }])?;
        let mut about = vec![NavItem {
            id: 1,
            title: title.clone(),
            detail: room.map(|r| text(&r.info.topic, 1024)).unwrap_or_default(),
            avatar: Avatar {
                square: room.is_none_or(|r| !r.info.direct),
                image: room
                    .and_then(|r| m.avatars.get(&format!("room:{}", r.id)))
                    .cloned(),
                ..avatar(&title)
            },
            ..Default::default()
        }];
        if room.is_some() {
            about.extend([
                NavItem {
                    id: 2,
                    title: tr.text("daylight-members"),
                    detail: room
                        .map(|r| {
                            m.member_count(r)
                                .map(|n| n.to_string())
                                .unwrap_or_else(|| tr.text("members-loading"))
                        })
                        .unwrap_or_default(),
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
        if let Some(profile) = &m.profile
            && m.selected.as_ref() == Some(&profile.room)
        {
            about = crate::profile::items(profile, m, tr);
            self.inspector.set_commands(&[ChatCommand {
                id: 100,
                label: tr.text("profile-back"),
                action: sys::CUI_CHAT_MORE,
                ..Default::default()
            }])?;
        } else {
            self.inspector.set_commands(&[ChatCommand {
                id: 1,
                label: tr.text("daylight-about"),
                action: sys::CUI_CHAT_MORE,
                ..Default::default()
            }])?;
        }
        self.inspector.rooms(&about)?;
        set_text(&self.account_button, &tr.text("settings-title"))?;
        self.account_button.set_tooltip(m.account_name())?;
        self.room_button.set_enabled(room.is_some())?;
        self.history_button.set_enabled(room.is_some())?;
        self.content.set_visible(matches!(
            m.phase,
            Phase::Connected | Phase::SigningOut | Phase::LogoutCleanup
        ))?;
        Ok(())
    }
    pub fn modal(&self, show: bool, title: &str, compact: bool) -> Result<()> {
        if show && !self.modal.get() {
            *self.focus_return.borrow_mut() = self.base.focused_descendant()?;
        }
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
            let restored = self
                .focus_return
                .borrow_mut()
                .take()
                .map(|w| w.focus())
                .transpose()?
                .unwrap_or(false);
            if !restored && self.connected.get() {
                self.composer.part(0)?.focus()?;
            }
        }
        Ok(())
    }
    pub fn is_modal(&self) -> bool {
        self.modal.get()
    }
    pub fn sidebar_expanded(&self) -> bool {
        self.sidebar_open.get()
    }
    pub fn refresh(&self, scale: f64) -> Result<()> {
        let width = self.base.allocated_size()?.map(|s| s.0).unwrap_or(1440);
        let zoom = self.text_scale.get();
        let wide = width as f64 >= 1100. * zoom;
        let was_wide = self.sidebar_wide.replace(wide);
        if was_wide && !wide {
            self.sidebar_before_narrow.set(self.sidebar_open.get());
            self.left_fraction
                .set(self.left_split.split_get_position()?);
            self.sidebar_open.set(false);
        }
        if !was_wide && wide {
            self.sidebar_open.set(self.sidebar_before_narrow.get());
        }
        let expanded = self.sidebar_expanded();
        for control in self.sidebar_controls.borrow().iter() {
            control.set_visible(expanded)?;
        }
        self.spaces
            .part(0)?
            .set_min_size(110, (42. * zoom) as i32)?;
        self.header.part(0)?.set_min_size(1, (72. * zoom) as i32)?;
        self.account_button.set_icon_only(!wide)?;
        self.right_pane
            .set_visible(self.inspector_open.get() && width as f64 >= 1400. * zoom)?;
        self.left_pane.set_visible(true)?;
        self.sidebar_surface.set_min_size(
            if expanded {
                (240. * zoom) as i32
            } else {
                (80. * zoom) as i32
            },
            1,
        )?;
        self.sidebar
            .box_set_padding(if expanded { 16 } else { 8 })?;
        if !expanded {
            let split_width = self
                .left_split
                .allocated_size()?
                .map(|s| s.0)
                .unwrap_or(width);
            self.left_split
                .split_set_position(80. * zoom / split_width.max(1) as f64)?;
        } else if !was_wide {
            self.left_split
                .split_set_position(self.left_fraction.get())?;
        }
        self.sidebar_toggle.set_visible(self.connected.get())?;
        let compact_spaces = !wide || (self.space_width.get() + 360.) * zoom > width as f64;
        self.top_spacer
            .expand(!self.connected.get() || compact_spaces)?;
        self.spaces
            .root
            .set_visible(self.connected.get() && !compact_spaces)?;
        self.space_picker
            .set_visible(self.connected.get() && compact_spaces)?;
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

fn room_preview(r: &archaic_matrix::RoomSummary, m: &Model, tr: &Translator) -> String {
    if r.membership == archaic_matrix::Membership::Invited {
        return tr.text("invitation");
    }
    let loaded = if m.selected.as_ref() == Some(&r.id) {
        Some(&m.messages)
    } else {
        m.history_cache
            .iter()
            .find(|h| h.room == r.id)
            .map(|h| &h.messages)
    };
    if let Some(last) = loaded
        .into_iter()
        .flatten()
        .filter(|v| !v.deleted && v.thread_root.is_none())
        .max_by_key(|v| v.timestamp)
        .filter(|v| v.timestamp >= r.info.recent)
    {
        return text(
            &last
                .body
                .as_deref()
                .unwrap_or("")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
            256,
        );
    }
    if let Some(p) = r
        .info
        .preview
        .as_ref()
        .filter(|p| !m.blocked.contains(&p.sender))
    {
        if p.encrypted {
            return tr.text("encrypted-message");
        }
        return text(
            &p.body.split_whitespace().collect::<Vec<_>>().join(" "),
            256,
        );
    }
    text(&r.info.topic, 256)
}
