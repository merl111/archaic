//! Session-scoped image previews. The Matrix worker owns download/decode work;
//! the UI retains only bounded pixels and CUI assets.
use crate::{model::Model, view::set_text};
use archaic_i18n::Translator;
use cui::{Result, Widget, sys};
use std::cell::RefCell;

pub struct CachedImage {
    pub room: String,
    pub event: String,
    pub data: archaic_matrix::ImagePreview,
    pub icon: cui::Icon,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn previews_are_scoped_bounded_and_ignore_old_sessions() {
        let mut m = Model {
            epoch: 2,
            selected: Some("!a:local".into()),
            ..Default::default()
        };
        let data = || archaic_matrix::ImagePreview {
            pixels: vec![20, 30, 40, 255],
            width: 1,
            height: 1,
        };
        for i in 0..60 {
            m.cache_image("!a:local".into(), format!("${i}"), data());
        }
        assert_eq!(m.images.len(), 48);
        assert!(m.image("$0").is_none() && m.image("$59").is_some());
        m.selected = Some("!b:local".into());
        assert!(m.image("$59").is_none());
        m.apply(archaic_matrix::Event {
            epoch: 1,
            kind: archaic_matrix::EventKind::ImagePreview {
                room_id: "!b:local".into(),
                event_id: "$old".into(),
                result: Ok(data()),
            },
        });
        assert!(m.image("$old").is_none());
        m.image_open = Some(("!b:local".into(), "$59".into()));
        m.clear_history();
        assert!(m.image_open.is_none());
    }

    #[test]
    fn redacted_image_closes_the_viewer() {
        let mut m = Model {
            selected: Some("!a:local".into()),
            image_open: Some(("!a:local".into(), "$image".into())),
            messages: vec![archaic_matrix::Message {
                id: "$image".into(),
                deleted: true,
                ..Default::default()
            }],
            ..Default::default()
        };
        m.apply(archaic_matrix::Event {
            epoch: m.epoch,
            kind: archaic_matrix::EventKind::Status("ready"),
        });
        assert!(m.image_open.is_none());
    }
}
impl Model {
    pub fn image(&self, event: &str) -> Option<&CachedImage> {
        let local = self
            .delivery_for(event)
            .and_then(|d| d.message.as_ref())
            .map(|m| m.id.as_str());
        self.images.iter().rev().find(|p| {
            Some(&p.room) == self.selected.as_ref()
                && (p.event == event || Some(p.event.as_str()) == local)
        })
    }
    pub fn image_message(&self, event: &str) -> Option<&archaic_matrix::Message> {
        self.messages
            .iter()
            .find(|m| m.id == event)
            .or_else(|| {
                self.tools.details.as_ref().and_then(|d| {
                    std::iter::once(&d.message)
                        .chain(&d.thread)
                        .find(|m| m.id == event)
                })
            })
            .or_else(|| self.delivery_for(event).and_then(|i| i.message.as_ref()))
    }
    pub fn cache_image(&mut self, room: String, event: String, data: archaic_matrix::ImagePreview) {
        if data.width == 0 || data.height == 0 || data.width > 1024 || data.height > 768 {
            return;
        }
        let Ok(icon) = cui::Icon::rgba(&data.pixels, data.width as i32, data.height as i32) else {
            return;
        };
        self.images.retain(|p| p.room != room || p.event != event);
        self.images.push_back(CachedImage {
            room,
            event,
            data,
            icon,
        });
        while self.images.len() > 48 {
            self.images.pop_front();
        }
        self.avatar_revision = self.avatar_revision.wrapping_add(1);
    }
}
pub struct Viewer {
    pub root: Widget,
    image: Widget,
    stage: Widget,
    name: Widget,
    status: Widget,
    pub retry: Widget,
    pub download: Widget,
    version: RefCell<String>,
}
impl Viewer {
    pub fn resize(&self, height: i32, text_scale: f64) -> Result<()> {
        let image_height = (f64::from(height) - 150. * text_scale - 64.).clamp(120., 420.);
        self.stage.set_min_size(1, image_height as i32)?;
        Ok(())
    }
    pub fn new(parent: &Widget, tr: &Translator) -> Result<Self> {
        let root = parent.box_layout(sys::CUI_VERTICAL, 12)?;
        root.expand(true)?;
        let stage = root.box_layout(sys::CUI_VERTICAL, 0)?;
        stage.set_min_size(1, 420)?;
        stage.expand(true)?;
        let image = stage.image()?;
        image.expand(true)?;
        image.accessibility(&tr.text("preview-file"), "")?;
        let row = root.box_layout(sys::CUI_HORIZONTAL, 12)?;
        let name = row.label("")?;
        name.expand(true)?;
        let retry = row.button(&tr.text("refresh"))?;
        let download = row.button(&tr.text("download-file"))?;
        let status = root.label("")?;
        status.set_role(sys::CUI_ROLE_CAPTION)?;
        root.set_visible(false)?;
        Ok(Self {
            root,
            image,
            stage,
            name,
            status,
            retry,
            download,
            version: Default::default(),
        })
    }
    pub fn render(&self, m: &Model, tr: &Translator) -> Result<()> {
        let Some((room, event)) = &m.image_open else {
            self.version.borrow_mut().clear();
            return Ok(());
        };
        if m.selected.as_ref() != Some(room) {
            return Ok(());
        }
        let full = m.tools.preview.as_ref().filter(|p| &p.event == event);
        let thumb = m.image(event);
        let pixels = full
            .map(|p| (&p.pixels, p.width, p.height))
            .or_else(|| thumb.map(|p| (&p.data.pixels, p.data.width, p.data.height)));
        let version = format!(
            "{}:{room}:{event}:{}:{}",
            m.epoch,
            full.map(|p| p.id.as_str()).unwrap_or("thumbnail"),
            pixels.is_some()
        );
        if *self.version.borrow() != version {
            self.image.set_visible(pixels.is_some())?;
            if let Some((bytes, w, h)) = pixels {
                self.image.rgba(bytes, w as i32, h as i32)?;
            }
            *self.version.borrow_mut() = version;
        }
        let name = m
            .image_message(event)
            .and_then(|v| v.attachment.as_ref())
            .map(|a| a.name.as_str())
            .unwrap_or("");
        set_text(&self.name, &crate::daylight::text(name, 1024))?;
        set_text(
            &self.status,
            &m.tools.error.map(|e| tr.text(e)).unwrap_or_else(|| {
                if m.tools.pending.is_some() {
                    tr.text("loading")
                } else if pixels.is_none() {
                    tr.text("preview-invalid")
                } else {
                    String::new()
                }
            }),
        )?;
        self.retry
            .set_enabled(m.can_tool() && event.starts_with('$'))?;
        self.download
            .set_enabled(m.can_tool() && event.starts_with('$'))
    }
}
