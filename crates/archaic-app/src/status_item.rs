//! Native tray/status item. Created and mutated only from the CUI event loop.
use crate::{Controller, model::Phase};
use cui::Result;
use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem},
};
pub struct StatusItem {
    icon: TrayIcon,
    open: MenuItem,
    quit: MenuItem,
    count: MenuItem,
    last: Option<u64>,
}
impl StatusItem {
    fn new(tr: &archaic_i18n::Translator) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let menu = Menu::new();
        let open = MenuItem::new(tr.text("desktop-show"), true, None);
        let quit = MenuItem::new(tr.text("desktop-quit"), true, None);
        let count = MenuItem::new(tr.text("desktop-no-unread"), false, None);
        menu.append_items(&[&open, &count, &quit])?;
        let icon = TrayIconBuilder::new()
            .with_tooltip("Archaic")
            .with_icon(badge_icon(false)?)
            .with_menu(Box::new(menu))
            .build()?;
        Ok(Self {
            icon,
            open,
            quit,
            count,
            last: None,
        })
    }
    fn update(
        &mut self,
        unread: u64,
        tr: &archaic_i18n::Translator,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        if self.last == Some(unread) {
            return Ok(());
        }
        let label = if unread == 0 {
            tr.text("desktop-no-unread")
        } else {
            format!("{}: {unread}", tr.text("desktop-unread"))
        };
        self.count.set_text(&label);
        self.icon.set_tooltip(Some(format!("Archaic · {label}")))?;
        self.icon.set_icon(Some(badge_icon(unread > 0)?))?;
        self.icon
            .set_title((unread > 0).then(|| unread.to_string()));
        dock_badge(unread);
        self.last = Some(unread);
        Ok(())
    }
}
fn badge_icon(unread: bool) -> std::result::Result<Icon, tray_icon::BadIcon> {
    let mut pixels = crate::branding::TRAY_RGBA.to_vec();
    if unread {
        for y in 1i32..14 {
            for x in 19i32..32 {
                if (x - 25).pow(2) + (y - 7).pow(2) <= 36 {
                    let index = ((y * 32 + x) * 4) as usize;
                    pixels[index..index + 4].copy_from_slice(&[243, 74, 101, 255]);
                }
            }
        }
    }
    Icon::from_rgba(pixels, 32, 32)
}
#[cfg(target_os = "macos")]
fn dock_badge(unread: u64) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;
    use objc2_foundation::NSString;
    if let Some(main) = MainThreadMarker::new() {
        let app = NSApplication::sharedApplication(main);
        let text = (unread > 0).then(|| NSString::from_str(&unread.to_string()));
        app.dockTile().setBadgeLabel(text.as_deref());
    }
}
#[cfg(not(target_os = "macos"))]
fn dock_badge(_: u64) {}
impl Controller {
    pub fn status_item_tick(&self) -> Result<()> {
        if self.fixture {
            return Ok(());
        }
        if !self.status_item_attempted.replace(true) {
            match StatusItem::new(&self.tr) {
                Ok(item) => *self.status_item.borrow_mut() = Some(item),
                Err(_) => {
                    self.model.borrow_mut().error = Some("desktop-tray-unavailable");
                    self.render()?;
                }
            }
        }
        let unread = {
            let m = self.model.borrow();
            if m.phase == Phase::Connected {
                m.rooms
                    .iter()
                    .filter(|r| r.membership == archaic_matrix::Membership::Joined)
                    .map(|r| m.activity.unread.get(&r.id).copied().unwrap_or(0))
                    .fold(0u64, u64::saturating_add)
            } else {
                0
            }
        };
        let mut show = false;
        let mut quit = false;
        if let Some(item) = &mut *self.status_item.borrow_mut() {
            if item.update(unread, &self.tr).is_err() {
                self.model.borrow_mut().error = Some("desktop-tray-unavailable");
            }
            while let Ok(event) = MenuEvent::receiver().try_recv() {
                show |= event.id == *item.open.id();
                quit |= event.id == *item.quit.id();
            }
            while let Ok(event) = TrayIconEvent::receiver().try_recv() {
                show |= matches!(
                    event,
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } | TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    }
                );
            }
        }
        if show {
            self.window.show()?;
        }
        if quit && let Some(app) = self.app.upgrade() {
            self.remember_draft()?;
            app.quit();
        }
        Ok(())
    }
}
