//! Functional room administration controls, kept independent of the conversation composer.
use crate::{
    Controller,
    model::{Model, Phase},
    view::set_text,
    widgets::label,
};
use archaic_i18n::Translator;
use archaic_matrix::{
    Command, TransactionId,
    organization::{Action, Page},
    transaction_id,
};
use cui::{Result, Widget, sys};
const KEYS: &[&str] = &[
    "org-directory",
    "org-spaces",
    "org-hierarchy",
    "org-inspect",
    "org-members",
    "org-create-space",
    "org-name",
    "org-topic",
    "org-join-rule",
    "org-history",
    "org-guests",
    "org-power",
    "org-invite",
    "org-kick",
    "org-ban",
    "org-unban",
    "org-favorite",
    "org-unfavorite",
    "org-block",
    "org-unblock",
    "org-blocked",
    "org-add-child",
    "org-remove-child",
    "org-alias-add",
    "org-alias-remove",
    "org-canonical",
    "org-publish",
    "org-unpublish",
    "org-low-priority",
    "org-normal-priority",
    "org-avatar",
    "org-clear-avatar",
    "org-restricted",
    "org-event-power",
    "org-retention",
    "org-upgrade",
    "org-report",
    "org-redact",
    "org-knock",
    "org-parent-add",
    "org-parent-remove",
    "org-threads",
    "org-participated-threads",
    "org-subscribe-thread",
    "org-unsubscribe-thread",
];
#[derive(Default)]
pub struct State {
    pub show: bool,
    pub pending: Option<TransactionId>,
    pub page: Page,
    pub error: Option<&'static str>,
    pub key: Option<(i32, String, String, String)>,
    pub cursors: std::collections::HashSet<String>,
}
impl State {
    pub fn apply(&mut self, id: TransactionId, result: std::result::Result<Page, &'static str>) {
        if self.pending.as_ref() != Some(&id) {
            return;
        }
        self.pending = None;
        match result {
            Ok(page) => {
                if page.next.as_ref().is_some_and(|next| {
                    self.cursors.len() >= 4096 || !self.cursors.insert(next.clone())
                }) {
                    self.error = Some("history-invalid-page");
                    self.page.next = None;
                } else {
                    self.page = page;
                }
            }
            Err(e) => self.error = Some(e),
        }
    }
}
pub struct Controls {
    root: Widget,
    pub mode: Widget,
    pub target: Widget,
    pub value: Widget,
    pub extra: Widget,
    pub apply: Widget,
    pub more: Widget,
    pub select: Widget,
    pub join: Widget,
    pub close: Widget,
    pub confirm: Widget,
    rows: Widget,
    row_labels: std::cell::RefCell<Vec<String>>,
    text: Widget,
    status: Widget,
}
impl Controls {
    pub fn new(root: &Widget, tr: &Translator) -> Result<Self> {
        let root = root.box_layout(sys::CUI_VERTICAL, 8)?;
        root.expand(true)?;
        root.box_set_padding(0)?;
        let bar = root.box_layout(sys::CUI_HORIZONTAL, 8)?;

        let close = bar.button(&tr.text("security-close"))?;
        bar.set_visible(false)?;
        let names = KEYS.iter().map(|k| tr.text(k)).collect::<Vec<_>>();
        let mode = root.select(&names.iter().map(String::as_str).collect::<Vec<_>>())?;
        label(&root, &tr.text("org-fields"), sys::CUI_ROLE_CAPTION)?;
        mode.set_selected(0)?;
        let target = root.entry("")?;
        target.set_placeholder(&tr.text("org-target"))?;
        target.accessibility(&tr.text("org-target"), "")?;
        let value = root.entry("")?;
        value.set_placeholder(&tr.text("org-value"))?;
        value.accessibility(&tr.text("org-value"), "")?;
        let extra = root.entry("")?;
        extra.set_placeholder(&tr.text("org-extra"))?;
        extra.accessibility(&tr.text("org-extra"), "")?;
        let confirm = root.checkbox(&tr.text("org-confirm"), false)?;
        let bar = root.box_layout(sys::CUI_HORIZONTAL, 8)?;
        let apply = bar.button(&tr.text("org-run"))?;
        let more = bar.button(&tr.text("search-more"))?;
        let select = bar.button(&tr.text("org-select"))?;
        let join = bar.button(&tr.text("join-room"))?;
        let rows = root.list(&[])?;
        rows.expand(true)?;
        rows.accessibility(&tr.text("organization"), "")?;
        let text = root.textarea("")?;
        text.set_read_only(true)?;
        text.expand(true)?;
        let status = label(&root, "", sys::CUI_ROLE_CAPTION)?;
        Ok(Self {
            root,
            mode,
            target,
            value,
            extra,
            confirm,
            apply,
            more,
            select,
            join,
            close,
            rows,
            row_labels: Default::default(),
            text,
            status,
        })
    }
    pub fn render(&self, m: &Model, tr: &Translator) -> Result<()> {
        self.root
            .set_visible(m.organization.show && m.phase == Phase::Connected)?;
        if !m.organization.show {
            return Ok(());
        }
        let enabled = m.phase == Phase::Connected && m.organization.pending.is_none();
        for w in [
            &self.mode,
            &self.target,
            &self.value,
            &self.extra,
            &self.confirm,
            &self.apply,
            &self.close,
            &self.select,
            &self.join,
        ] {
            w.set_enabled(enabled)?;
        }
        self.join
            .set_enabled(enabled && !matches!(self.mode.get_selected()?, 41 | 42))?;
        self.more
            .set_enabled(enabled && m.organization.page.next.is_some())?;
        let labels = m
            .organization
            .page
            .rows
            .iter()
            .map(|r| {
                format!("{} · {} · {}", r.title, r.id, r.detail).replace(['\0', '\n', '\r'], " ")
            })
            .collect::<Vec<_>>();
        if *self.row_labels.borrow() != labels {
            self.rows
                .set_items(&labels.iter().map(String::as_str).collect::<Vec<_>>())?;
            *self.row_labels.borrow_mut() = labels.clone();
        }
        self.rows.set_visible(!labels.is_empty())?;
        self.text
            .set_visible(!m.organization.page.text.is_empty())?;
        set_text(&self.text, &m.organization.page.text)?;
        set_text(
            &self.status,
            &tr.text(m.organization.error.unwrap_or(if enabled {
                "tool-ready"
            } else {
                "tool-working"
            })),
        )
    }
    fn action(&self, next: Option<String>) -> Result<std::result::Result<Action, &'static str>> {
        let room = self.target.text()?.trim().to_owned();
        let value = self.value.text()?.trim().to_owned();
        let extra = self.extra.text()?.trim().to_owned();
        let mode = self.mode.get_selected()?;
        if mode >= 5 && ![20, 41, 42].contains(&mode) && !self.confirm.get_checked()? {
            return Ok(Err("org-confirm-required"));
        }
        Ok(Ok(match mode {
            0 => Action::Directory { query: value, next },
            1 => Action::Spaces,
            2 => Action::Hierarchy { room, next },
            3 => Action::Inspect(room),
            4 => Action::Members(room),
            5 => Action::CreateSpace {
                name: value,
                topic: extra,
            },
            6 => Action::SetName { room, value },
            7 => Action::SetTopic { room, value },
            8 => Action::JoinRule { room, value },
            9 => Action::HistoryVisibility { room, value },
            10 => Action::GuestAccess { room, value },
            11 => {
                let Ok(level) = extra.parse() else {
                    return Ok(Err("organization-invalid"));
                };
                Action::Power {
                    room,
                    user: value,
                    level,
                }
            }
            12..=15 => Action::Moderate {
                room,
                user: value,
                kind: ["invite", "kick", "ban", "unban"][(mode - 12) as usize].into(),
                reason: extra,
            },
            16 | 17 => Action::Favourite {
                room,
                enabled: mode == 16,
            },
            18 | 19 => Action::Ignore {
                user: value,
                enabled: mode == 18,
            },
            20 => Action::Blocked,
            21 | 22 => Action::SpaceChild {
                room,
                child: value,
                remove: mode == 22,
            },
            23..=44 => {
                use archaic_matrix::room_settings::Operation::*;
                let operations = [
                    AliasAdd,
                    AliasRemove,
                    CanonicalAlias,
                    Publish,
                    Unpublish,
                    LowPriority,
                    NormalPriority,
                    Avatar,
                    ClearAvatar,
                    Restricted,
                    EventPower,
                    Retention,
                    Upgrade,
                    Report,
                    Redact,
                    Knock,
                    ParentAdd,
                    ParentRemove,
                    Threads,
                    ParticipatedThreads,
                    SubscribeThread,
                    UnsubscribeThread,
                ];
                Action::Advanced {
                    room,
                    operation: operations[(mode - 23) as usize].clone(),
                    value,
                    extra,
                    next,
                }
            }
            _ => return Ok(Err("organization-invalid")),
        }))
    }
}
impl Controller {
    pub fn toggle_organization(&self) -> Result<()> {
        self.remember_draft()?;
        let mut m = self.model.borrow_mut();
        if m.phase == Phase::Connected
            && m.organization.pending.is_none()
            && !m.tools_busy()
            && m.security.pending.is_none()
        {
            m.organization.show = !m.organization.show;
            if m.organization.show {
                self.view
                    .organization
                    .target
                    .set_text(m.selected.as_deref().unwrap_or(""))?;
            }
        }
        drop(m);
        self.render()
    }
    pub fn organization_run(&self) -> Result<()> {
        self.organization_page(false)
    }
    pub fn organization_more(&self) -> Result<()> {
        self.organization_page(true)
    }
    fn organization_page(&self, more: bool) -> Result<()> {
        if self.fixture || self.model.borrow().organization.pending.is_some() {
            return Ok(());
        }
        let c = &self.view.organization;
        let key = (
            c.mode.get_selected()?,
            c.target.text()?,
            c.value.text()?,
            c.extra.text()?,
        );
        if more && self.model.borrow().organization.key.as_ref() != Some(&key) {
            self.model.borrow_mut().organization.error = Some("search-restart");
            return self.render();
        }
        let next = if more {
            self.model.borrow().organization.page.next.clone()
        } else {
            None
        };
        match c.action(next)? {
            Ok(action) => {
                let id = transaction_id();
                if self.enqueue(Command::Organization {
                    id: id.clone(),
                    action,
                }) {
                    let mut m = self.model.borrow_mut();
                    if !more {
                        m.organization.cursors.clear();
                    }
                    m.organization.pending = Some(id);
                    m.organization.key = Some(key);
                    m.organization.error = None;
                }
            }
            Err(e) => self.model.borrow_mut().organization.error = Some(e),
        }
        self.render()
    }
    pub fn organization_select(&self) -> Result<()> {
        let index = self.view.organization.rows.get_selected()?;
        if let Some(row) = usize::try_from(index)
            .ok()
            .and_then(|i| self.model.borrow().organization.page.rows.get(i).cloned())
        {
            if row.id.starts_with('$') {
                let Some(room) = self
                    .model
                    .borrow()
                    .organization
                    .key
                    .as_ref()
                    .filter(|(mode, _, _, _)| matches!(mode, 41 | 42))
                    .map(|(_, room, _, _)| room.trim().to_owned())
                else {
                    return Ok(());
                };
                self.remember_draft()?;
                if self.enqueue(Command::Select(room.clone())) {
                    {
                        let mut m = self.model.borrow_mut();
                        m.selected = Some(room);
                        m.clear_history();
                        m.message_selected = Some(row.id);
                        m.organization.show = false;
                        m.tools.show = true;
                        m.tools.mode = 2;
                    }
                    self.render()?;
                    self.load_details()?;
                }
            } else if row.id.starts_with('@') {
                self.view.organization.value.set_text(&row.id)?;
            } else {
                self.view.organization.target.set_text(&row.id)?;
            }
        }
        Ok(())
    }
    pub fn organization_join(&self) -> Result<()> {
        self.organization_select()?;
        let room = self.view.organization.target.text()?;
        if !self.fixture && room.starts_with('!') && self.enqueue(Command::Join(room)) {
            self.model.borrow_mut().organization.show = false;
        }
        self.render()
    }
}
