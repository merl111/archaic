//! Project real Matrix messages onto the reusable Daylight timeline.
use crate::{
    daylight::{Ids, avatar, text},
    model::Model,
};
use archaic_i18n::Translator;
use archaic_matrix::Message as MatrixMessage;
use cui::{
    ChatCommand, ChatComponent, ChatKind, Result, Widget,
    chat::{Attachment, Message, Reaction, Reply, Theme},
    sys,
};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
};
pub struct Timeline {
    pub chat: ChatComponent,
    pub ids: RefCell<Ids>,
    cache: RefCell<Vec<MatrixMessage>>,
    scope: RefCell<String>,
    dark: Cell<bool>,
    relation: RefCell<(String, usize)>,
}
impl Timeline {
    pub fn new(parent: &Widget, tr: &Translator, t: &Theme) -> Result<Self> {
        let chat = ChatComponent::create(parent, ChatKind::Timeline, t)?;
        chat.set_commands(&[
            ChatCommand {
                id: 1,
                label: tr.text("message-react"),
                symbol: sys::CUI_SYMBOL_EMOJI,
                action: sys::CUI_CHAT_REACT,
                ..Default::default()
            },
            ChatCommand {
                id: 2,
                label: tr.text("message-reply"),
                symbol: sys::CUI_SYMBOL_REPLY,
                action: sys::CUI_CHAT_REPLY,
                ..Default::default()
            },
            ChatCommand {
                id: 3,
                label: tr.text("message-thread"),
                symbol: sys::CUI_SYMBOL_THREAD,
                action: sys::CUI_CHAT_THREAD,
                ..Default::default()
            },
            ChatCommand {
                id: 4,
                label: tr.text("daylight-message-actions"),
                symbol: sys::CUI_SYMBOL_MORE,
                action: sys::CUI_CHAT_MORE,
                ..Default::default()
            },
        ])?;
        Ok(Self {
            chat,
            ids: Default::default(),
            cache: Default::default(),
            scope: Default::default(),
            dark: Cell::new(false),
            relation: Default::default(),
        })
    }
    pub fn set_theme(&self, t: &Theme) -> Result<()> {
        self.dark.set(t.background == 0x101c23ff);
        self.cache.borrow_mut().clear();
        self.chat.set_theme(t)
    }
    pub fn event_id(&self, id: u64) -> Option<String> {
        self.ids.borrow().key(id)
    }
    pub fn render(&self, m: &Model, tr: &Translator) -> Result<()> {
        let status = if m.status == "loading" {
            tr.text("loading")
        } else if m.messages.is_empty() {
            tr.text("empty-history")
        } else {
            String::new()
        };
        self.chat.status(&status)?;
        let scope = format!("{}:{}", m.epoch, m.selected.as_deref().unwrap_or(""));
        let changed = *self.scope.borrow() != scope;
        let relation = m
            .tools
            .details
            .as_ref()
            .map(|d| (d.message.id.clone(), d.thread.len()))
            .unwrap_or_default();
        let relations_changed = *self.relation.borrow() != relation;
        *self.relation.borrow_mut() = relation;
        if !changed && !relations_changed && *self.cache.borrow() == m.messages {
            return Ok(());
        }
        self.render_items(m, tr, &m.messages, &scope, changed || relations_changed)
    }
    pub fn render_items(
        &self,
        m: &Model,
        tr: &Translator,
        items: &[MatrixMessage],
        scope: &str,
        force: bool,
    ) -> Result<()> {
        let changed = *self.scope.borrow() != scope;
        if !force && !changed && *self.cache.borrow() == items {
            return Ok(());
        }
        let mut ids = self.ids.borrow_mut();
        if changed {
            ids.retain(&[]);
        }
        ids.retain(&items.iter().map(|v| v.id.clone()).collect::<Vec<_>>());
        let mut projected = items
            .iter()
            .map(|v| project(v, m, tr, &mut ids, self.dark.get()))
            .collect::<Vec<_>>();
        date_separators(&mut projected);
        self.chat.messages(&projected)?;
        if changed {
            self.chat.scroll(-1.)?;
        }
        *self.scope.borrow_mut() = scope.to_owned();
        *self.cache.borrow_mut() = items.to_vec();
        Ok(())
    }
}
fn project(v: &MatrixMessage, m: &Model, tr: &Translator, ids: &mut Ids, dark: bool) -> Message {
    let body = if v.deleted {
        tr.text("message-deleted")
    } else {
        v.body.clone().unwrap_or_else(|| {
            tr.text(if v.undecrypted {
                "encrypted-message"
            } else {
                "unavailable-message"
            })
        })
    };
    let hue = v
        .sender
        .bytes()
        .fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32))
        % 360;
    Message {
        id: ids.id(&v.id),
        author: avatar(&v.sender),
        body: text(&body, 65536).into(),
        author_color: cui::chat_color(if dark { 0.82 } else { 0.45 }, 0.10, hue as f64, 1.),
        reply: reply(v, m, ids),
        attachments: attachments(v),
        reactions: reactions(v, &m.user),
        thread: thread_summary(v, m),
        time: display_time(v, tr),
        date: timestamp(v.timestamp)
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_default(),
        ..Default::default()
    }
}
fn thread_summary(v: &MatrixMessage, m: &Model) -> Option<cui::chat::ThreadSummary> {
    if v.thread_root.is_some() {
        return None;
    }
    let mut ids = BTreeSet::new();
    let details = m.tools.details.as_ref().filter(|d| d.message.id == v.id);
    for reply in m
        .messages
        .iter()
        .chain(details.into_iter().flat_map(|d| &d.thread))
    {
        if reply.thread_root.as_ref() == Some(&v.id) {
            ids.insert(&reply.id);
        }
    }
    (!ids.is_empty()).then(|| cui::chat::ThreadSummary {
        count: ids.len() as u32,
        preview: String::new(),
        participants: Vec::new(),
    })
}

fn reply(v: &MatrixMessage, m: &Model, ids: &mut Ids) -> Option<Reply> {
    let target = v.reply_to.as_ref()?;
    let original = m.messages.iter().find(|o| &o.id == target);
    Some(Reply {
        message: ids.id(target),
        author: original.map(|o| text(&o.sender, 1024)).unwrap_or_default(),
        text: original
            .map(|o| text(o.body.as_deref().unwrap_or(""), 4096))
            .unwrap_or_else(|| text(target, 4096)),
    })
}
fn attachments(v: &MatrixMessage) -> Vec<Attachment> {
    v.attachment
        .as_ref()
        .map(|a| {
            vec![Attachment {
                id: 1,
                name: text(&a.name, 1024),
                detail: a.size.map(file_size).unwrap_or_default(),
            }]
        })
        .unwrap_or_default()
}
fn file_size(bytes: u64) -> String {
    for (unit, divisor) in [
        ("GiB", 1u64 << 30),
        ("MiB", 1u64 << 20),
        ("KiB", 1u64 << 10),
    ] {
        if bytes >= divisor {
            return format!("{:.1} {unit}", bytes as f64 / divisor as f64);
        }
    }
    format!("{bytes} B")
}

fn reactions(v: &MatrixMessage, user: &str) -> Vec<Reaction> {
    let mut keys = BTreeMap::<&str, BTreeSet<&str>>::new();
    for r in &v.reactions {
        if r.key.len() <= 64 && !r.key.contains('\0') {
            keys.entry(&r.key).or_default().insert(&r.sender);
        }
    }
    keys.into_iter()
        .take(32)
        .map(|(key, senders)| Reaction {
            key: key.into(),
            count: senders.len() as u32,
            mine: senders.contains(user),
        })
        .collect()
}
fn display_time(v: &MatrixMessage, tr: &Translator) -> String {
    let time = timestamp(v.timestamp)
        .map(|d| d.format("%H:%M").to_string())
        .unwrap_or_default();
    if v.edited {
        format!("{time} · {}", tr.text("message-edited"))
    } else {
        time
    }
}
fn timestamp(value: u64) -> Option<chrono::DateTime<chrono::Local>> {
    if value == 0 {
        return None;
    }
    chrono::DateTime::from_timestamp_millis(i64::try_from(value).ok()?)
        .map(|d| d.with_timezone(&chrono::Local))
}
fn date_separators(messages: &mut [Message]) {
    let mut last = String::new();
    for m in messages {
        if m.date == last {
            m.date.clear();
        } else {
            last = m.date.clone();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hostile_text_is_bounded_and_reaction_counts_are_per_sender() {
        let mut m = Model {
            user: "@a:x".into(),
            ..Default::default()
        };
        let reaction = archaic_matrix::Reaction {
            id: "$r".into(),
            sender: m.user.clone(),
            key: "❤️".into(),
        };
        let v = MatrixMessage {
            id: "$v".into(),
            sender: "@a\0:x".into(),
            body: Some("é🦀\0".repeat(12000)),
            reactions: vec![reaction.clone(), reaction],
            ..Default::default()
        };
        m.messages.push(v.clone());
        let projected = project(&v, &m, &Translator::new("en"), &mut Ids::default(), false);
        assert!(projected.body.plain().len() <= 65536);
        assert!(!projected.body.plain().contains('\0'));
        assert!(!projected.author.name.contains('\0'));
        assert_eq!(projected.reactions[0].count, 1);
        assert!(projected.reactions[0].mine);
    }
    #[test]
    fn reply_keeps_target_identity_and_dates_only_start_groups() {
        let original = MatrixMessage {
            id: "$a".into(),
            sender: "@a:x".into(),
            body: Some("Original".into()),
            timestamp: 1_700_000_000_000,
            ..Default::default()
        };
        let response = MatrixMessage {
            id: "$b".into(),
            sender: "@b:x".into(),
            body: Some("Reply".into()),
            reply_to: Some(original.id.clone()),
            timestamp: original.timestamp + 10_000,
            ..Default::default()
        };
        let m = Model {
            messages: vec![original, response],
            ..Default::default()
        };
        let mut ids = Ids::default();
        let tr = Translator::new("en");
        let mut items = m
            .messages
            .iter()
            .map(|v| project(v, &m, &tr, &mut ids, false))
            .collect::<Vec<_>>();
        assert_eq!(items[1].reply.as_ref().unwrap().message, items[0].id);
        assert_eq!(items[1].reply.as_ref().unwrap().text, "Original");
        date_separators(&mut items);
        assert!(!items[0].date.is_empty());
        assert!(items[1].date.is_empty());
        assert!(timestamp(u64::MAX).is_none());
        assert!(timestamp(0).is_none());
    }
}
