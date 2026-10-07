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
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TimelineKind {
    Room,
    Thread,
}
pub struct Timeline {
    kind: TimelineKind,
    positions: RefCell<std::collections::VecDeque<(String, f64)>>,
    epoch: Cell<u64>,
    pub chat: ChatComponent,
    pub ids: RefCell<Ids>,
    cache: RefCell<Vec<MatrixMessage>>,
    deliveries: RefCell<Vec<archaic_matrix::outbox::Item>>,
    scope: RefCell<String>,
    dark: Cell<bool>,
    avatars: Cell<u64>,
    relation: RefCell<(String, usize)>,
}
impl Timeline {
    pub fn new(parent: &Widget, tr: &Translator, t: &Theme, kind: TimelineKind) -> Result<Self> {
        let chat = ChatComponent::create(parent, ChatKind::Timeline, t)?;
        crate::chat_labels::apply(&chat, tr)?;
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
            kind,
            positions: Default::default(),
            epoch: Cell::new(0),
            chat,
            ids: Default::default(),
            cache: Default::default(),
            deliveries: Default::default(),
            scope: Default::default(),
            dark: Cell::new(false),
            avatars: Cell::new(0),
            relation: Default::default(),
        })
    }
    pub fn set_compact(&self, compact: bool) -> Result<()> {
        let mut presentation = self.chat.presentation()?;
        presentation.messages = if compact {
            sys::CUI_CHAT_COMPACT
        } else {
            sys::CUI_CHAT_SOFT
        };
        self.chat.set_presentation(&presentation)?;
        Ok(())
    }
    pub fn set_theme(&self, t: &Theme) -> Result<()> {
        self.dark.set(t.background == 0x101c23ff);
        self.cache.borrow_mut().clear();
        self.chat.set_theme(t)?;
        let mut p = self.chat.presentation()?;
        p.mention_foreground = if self.dark.get() {
            0x99dbecff
        } else {
            0x245fa6ff
        };
        p.mention_background = t.selected;
        self.chat.set_presentation(&p)
    }
    pub fn event_id(&self, id: u64) -> Option<String> {
        self.ids.borrow().key(id)
    }
    pub fn render(&self, m: &Model, tr: &Translator) -> Result<()> {
        let status = if m.status == "loading" {
            tr.text("loading")
        } else if m.messages.iter().all(|v| v.thread_root.is_some()) {
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
        let relations_changed =
            *self.relation.borrow() != relation || self.avatars.get() != m.avatar_revision;
        *self.relation.borrow_mut() = relation;
        if !changed
            && !relations_changed
            && *self.cache.borrow() == m.messages
            && *self.deliveries.borrow() == m.outbox
        {
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
        if !force
            && !changed
            && self.avatars.get() == m.avatar_revision
            && *self.cache.borrow() == items
            && *self.deliveries.borrow() == m.outbox
        {
            return Ok(());
        }
        if self.epoch.replace(m.epoch) != m.epoch {
            self.positions.borrow_mut().clear();
        } else if changed && !self.scope.borrow().is_empty() {
            let previous = self.scope.borrow().clone();
            let mut positions = self.positions.borrow_mut();
            positions.retain(|(s, _)| s != &previous);
            positions.push_back((previous, self.chat.offset()?));
            while positions.len() > 32 {
                positions.pop_front();
            }
        }
        let mut ids = self.ids.borrow_mut();
        if changed {
            ids.retain(&[]);
        }
        let projected = project_items(items, m, tr, &mut ids, self.dark.get(), self.kind);
        self.chat.messages(&projected)?;
        if changed {
            let position = self
                .positions
                .borrow()
                .iter()
                .find(|(s, _)| s == scope)
                .map(|(_, y)| *y)
                .unwrap_or(-1.);
            self.chat.scroll(position)?;
        }
        *self.scope.borrow_mut() = scope.to_owned();
        *self.cache.borrow_mut() = items.to_vec();
        *self.deliveries.borrow_mut() = m.outbox.clone();
        self.avatars.set(m.avatar_revision);
        Ok(())
    }
}
// Keep the complete event cache for relations/counts; only the visible room
// projection excludes thread replies. The thread pane uses the same renderer.
fn project_items(
    items: &[MatrixMessage],
    m: &Model,
    tr: &Translator,
    ids: &mut Ids,
    dark: bool,
    kind: TimelineKind,
) -> Vec<Message> {
    let thread = if kind == TimelineKind::Thread {
        items
            .first()
            .map(|v| v.thread_root.as_deref().unwrap_or(&v.id))
    } else {
        None
    };
    let items = m.with_local_echoes(items, thread);
    let visible: Vec<_> = items
        .iter()
        .filter(|v| kind == TimelineKind::Thread || v.thread_root.is_none())
        .collect();
    ids.retain(&visible.iter().map(|v| v.id.clone()).collect::<Vec<_>>());
    let mut projected = visible
        .into_iter()
        .map(|v| {
            let mut item = project(v, m, tr, ids, dark);
            let readers = if kind == TimelineKind::Thread {
                &v.thread_read_by
            } else {
                &v.read_by
            };
            item.read_by = readers
                .iter()
                .filter(|u| *u != &m.user && !m.blocked.contains(*u))
                .take(128)
                .enumerate()
                .map(|(i, user)| {
                    let name = text(m.user_name(user), 1024);
                    cui::chat::NavItem {
                        id: i as u64 + 1,
                        title: name.clone(),
                        detail: format!("{} {}", tr.text("message-read-by"), name),
                        trailing: user.clone(),
                        avatar: cui::chat::Avatar {
                            image: m
                                .selected
                                .as_ref()
                                .and_then(|r| m.avatars.get(&format!("user:{r}:{user}")))
                                .cloned(),
                            ..avatar(&name)
                        },
                        ..Default::default()
                    }
                })
                .collect();
            if let Some(delivery) = m.delivery_for_message(v) {
                item.delivery = if delivery.failed {
                    sys::CUI_CHAT_SEND_FAILED
                } else if delivery.event_id.is_none() {
                    sys::CUI_CHAT_SENDING
                } else {
                    sys::CUI_CHAT_DELIVERED
                };
                if delivery.event_id.is_none() {
                    item.read_by.clear();
                }
            } else if v.sender == m.user {
                item.delivery = sys::CUI_CHAT_DELIVERED;
            }
            item.delivery_label = match item.delivery {
                sys::CUI_CHAT_SENDING => tr.text("delivery-sending"),
                sys::CUI_CHAT_DELIVERED => tr.text("delivery-delivered"),
                sys::CUI_CHAT_SEND_FAILED => tr.text("delivery-failed"),
                _ => String::new(),
            };
            item
        })
        .collect::<Vec<_>>();
    date_separators(&mut projected);
    projected
}

fn project(v: &MatrixMessage, m: &Model, tr: &Translator, ids: &mut Ids, dark: bool) -> Message {
    let body = if v.deleted {
        tr.text("message-deleted")
    } else if v
        .attachment
        .as_ref()
        .is_some_and(|a| v.body.as_deref() == Some(&a.name))
    {
        String::new()
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
        outgoing: v.sender == m.user,
        author: cui::chat::Avatar {
            image: m
                .selected
                .as_ref()
                .and_then(|r| m.avatars.get(&format!("user:{r}:{}", v.sender)))
                .cloned(),
            ..avatar(m.user_name(&v.sender))
        },
        body: if !v.deleted && !v.undecrypted {
            v.formatted_body
                .as_deref()
                .map(|html| crate::message_links::formatted(html, &body))
                .unwrap_or_else(|| crate::message_links::parse(&text(&body, 65536)))
        } else {
            crate::message_links::parse(&text(&body, 65536))
        },
        author_color: cui::chat_color(if dark { 0.82 } else { 0.45 }, 0.10, hue as f64, 1.),
        reply: reply(v, m, ids),
        attachments: attachments(v, m),
        reactions: reactions(v, m),
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
    let mut replies = BTreeMap::new();
    let details = m.tools.details.as_ref().filter(|d| d.message.id == v.id);
    for reply in m
        .messages
        .iter()
        .chain(details.into_iter().flat_map(|d| &d.thread))
    {
        if reply.thread_root.as_ref() == Some(&v.id) && !reply.deleted {
            replies.insert(&reply.id, reply);
        }
    }
    let latest = replies.values().max_by_key(|v| v.timestamp);
    let mut senders = BTreeSet::new();
    let participants = replies
        .values()
        .filter(|v| senders.insert(&v.sender))
        .take(4)
        .map(|v| cui::chat::Avatar {
            image: m
                .selected
                .as_ref()
                .and_then(|r| m.avatars.get(&format!("user:{r}:{}", v.sender)))
                .cloned(),
            ..avatar(m.user_name(&v.sender))
        })
        .collect();
    (!replies.is_empty()).then(|| cui::chat::ThreadSummary {
        count: replies.len() as u32,
        preview: latest
            .map(|v| text(v.body.as_deref().unwrap_or(""), 1024))
            .unwrap_or_default(),
        participants,
    })
}

fn reply(v: &MatrixMessage, m: &Model, ids: &mut Ids) -> Option<Reply> {
    let target = v.reply_to.as_ref()?;
    let original = m.messages.iter().find(|o| &o.id == target).or_else(|| {
        let details = m.tools.details.as_ref()?;
        std::iter::once(&details.message)
            .chain(details.thread.iter())
            .chain(details.reply.iter())
            .find(|o| &o.id == target)
    });
    Some(Reply {
        message: ids.id(target),
        author: original
            .map(|o| text(m.user_name(&o.sender), 1024))
            .unwrap_or_default(),
        text: original
            .map(|o| text(o.body.as_deref().unwrap_or(""), 4096))
            .unwrap_or_else(|| text(target, 4096)),
    })
}
fn attachments(v: &MatrixMessage, m: &Model) -> Vec<Attachment> {
    v.attachment
        .as_ref()
        .map(|a| {
            vec![Attachment {
                id: 1,
                name: text(&a.name, 1024),
                detail: a.size.map(file_size).unwrap_or_default(),
                image: (!v.deleted && a.kind == "m.image")
                    .then(|| m.image(&v.id).map(|p| p.icon.clone()))
                    .flatten(),
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

fn reactions(v: &MatrixMessage, m: &Model) -> Vec<Reaction> {
    let mut keys = BTreeMap::<&str, BTreeSet<&str>>::new();
    for r in &v.reactions {
        if r.key.len() <= 64 && !r.key.contains('\0') {
            keys.entry(&r.key).or_default().insert(&r.sender);
        }
    }
    keys.into_iter()
        .take(32)
        .map(|(key, senders)| Reaction {
            tooltip: crate::daylight::text(
                &senders
                    .iter()
                    .map(|user| {
                        let name = m.user_name(user);
                        if name == *user {
                            (*user).to_owned()
                        } else {
                            format!("{name} ({user})")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
                4096,
            ),
            key: key.into(),
            count: senders.len() as u32,
            mine: senders.contains(m.user.as_str()),
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
    fn thread_replies_stay_in_threads_and_keep_the_parent_count() {
        let parent = MatrixMessage {
            id: "$root".into(),
            body: Some("Parent".into()),
            timestamp: 1_700_000_000_000,
            ..Default::default()
        };
        let thread = MatrixMessage {
            id: "$thread".into(),
            body: Some("Thread reply".into()),
            thread_root: Some(parent.id.clone()),
            reply_to: Some(parent.id.clone()),
            timestamp: parent.timestamp + 86_400_000,
            ..Default::default()
        };
        let reply = MatrixMessage {
            id: "$reply".into(),
            body: Some("Ordinary reply".into()),
            reply_to: Some(parent.id.clone()),
            timestamp: thread.timestamp + 1000,
            ..Default::default()
        };
        let model = Model {
            messages: vec![parent.clone(), thread.clone(), reply],
            ..Default::default()
        };
        let tr = Translator::new("en");
        for dark in [false, true] {
            let room = project_items(
                &model.messages,
                &model,
                &tr,
                &mut Ids::default(),
                dark,
                TimelineKind::Room,
            );
            assert_eq!(room.len(), 2);
            assert_eq!(room[0].body.plain(), "Parent");
            assert_eq!(room[1].body.plain(), "Ordinary reply");
            assert_eq!(room[0].thread.as_ref().unwrap().count, 1);
            assert!(room[1].reply.is_some());
            assert!(!room[1].date.is_empty());
            let pane = project_items(
                &[parent.clone(), thread.clone()],
                &model,
                &tr,
                &mut Ids::default(),
                dark,
                TimelineKind::Thread,
            );
            assert_eq!(pane.len(), 2);
            assert_eq!(pane[1].body.plain(), "Thread reply");
            assert!(
                project_items(
                    std::slice::from_ref(&thread),
                    &model,
                    &tr,
                    &mut Ids::default(),
                    dark,
                    TimelineKind::Room
                )
                .is_empty()
            );
        }
        assert_eq!(model.messages.len(), 3);
    }
    #[test]
    fn reader_projection_resolves_names_and_uses_the_right_timeline() {
        let mut m = Model {
            user: "@me:x".into(),
            selected: Some("!r:x".into()),
            ..Default::default()
        };
        m.display_names
            .insert("user:!r:x:@bob:x".into(), "Bob".into());
        m.blocked.insert("@blocked:x".into());
        let message = MatrixMessage {
            id: "$one".into(),
            read_by: vec!["@me:x".into(), "@bob:x".into(), "@blocked:x".into()],
            thread_read_by: vec!["@carol:x".into()],
            ..Default::default()
        };
        for dark in [false, true] {
            let room = project_items(
                std::slice::from_ref(&message),
                &m,
                &Translator::new("en"),
                &mut Ids::default(),
                dark,
                TimelineKind::Room,
            );
            assert_eq!(room[0].read_by.len(), 1);
            assert_eq!(room[0].read_by[0].title, "Bob");
            assert_eq!(room[0].read_by[0].detail, "Read by Bob");
            assert_eq!(room[0].read_by[0].trailing, "@bob:x");
            let thread = project_items(
                std::slice::from_ref(&message),
                &m,
                &Translator::new("en"),
                &mut Ids::default(),
                dark,
                TimelineKind::Thread,
            );
            assert_eq!(thread[0].read_by[0].trailing, "@carol:x");
        }
    }
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
        assert_eq!(projected.reactions[0].tooltip, "@a:x");
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

pub fn room_time(ms: u64) -> String {
    let Some(time) = timestamp(ms).filter(|_| ms != 0) else {
        return String::new();
    };
    if time.date_naive() == chrono::Local::now().date_naive() {
        time.format("%H:%M").to_string()
    } else {
        time.format("%d.%m").to_string()
    }
}
