//! The right inspector alternates between room information and a real member profile.
use crate::{Controller, model::Model};
use archaic_i18n::Translator;
use archaic_matrix::{Command, MemberProfile};
use cui::{
    Result,
    chat::{Avatar, NavItem},
};

pub fn items(p: &MemberProfile, m: &Model, tr: &Translator) -> Vec<NavItem> {
    let mut items = vec![
        NavItem {
            id: 1,
            title: crate::daylight::text(&p.name, 1024),
            detail: p.user.clone(),
            avatar: Avatar {
                image: m
                    .avatars
                    .get(&format!("user:{}:{}", p.room, p.user))
                    .cloned(),
                ..crate::daylight::avatar(&p.name)
            },
            ..Default::default()
        },
        NavItem {
            id: 2,
            title: tr.text("profile-role"),
            detail: tr.text(if p.loaded { &p.role } else { "loading" }),
            ..Default::default()
        },
        NavItem {
            id: 3,
            title: tr.text("profile-identity"),
            detail: tr.text(if !p.loaded {
                "loading"
            } else if p.verified {
                "profile-verified"
            } else {
                "profile-unverified"
            }),
            ..Default::default()
        },
    ];
    for (id, key) in [
        (101, "profile-message"),
        (102, "profile-receipt"),
        (103, "profile-share"),
        (104, "profile-mention"),
        (105, "profile-copy"),
        (
            106,
            if m.blocked.contains(&p.user) {
                "profile-unignore"
            } else {
                "profile-ignore"
            },
        ),
    ] {
        items.push(NavItem {
            id,
            title: tr.text(key),
            disabled: (id == 102 && p.receipt.is_none())
                || ((id == 101 || id == 106) && p.user == m.user),
            ..Default::default()
        });
    }
    items
}
impl Controller {
    pub fn open_user_profile(&self) -> Result<()> {
        let user = self
            .model
            .borrow()
            .selected_message()
            .map(|m| m.sender.clone());
        if let Some(user) = user {
            self.open_member_profile(user)?;
        }
        Ok(())
    }
    pub fn open_member_profile(&self, user: String) -> Result<()> {
        let mut m = self.model.borrow_mut();
        let Some(room) = m.selected.clone() else {
            return Ok(());
        };
        m.profile = Some(MemberProfile {
            room: room.clone(),
            name: user.clone(),
            user: user.clone(),
            ..Default::default()
        });
        drop(m);
        if !self.fixture {
            self.enqueue(Command::MemberProfile { room, user });
        }
        self.view.ui.inspector_open.set(true);
        self.render()
    }
    pub fn profile_action(&self, id: u64) -> Result<()> {
        let Some(p) = self.model.borrow().profile.clone() else {
            return Ok(());
        };
        if self.model.borrow().selected.as_ref() != Some(&p.room) {
            return Ok(());
        }
        match id {
            100 => {
                self.model.borrow_mut().profile = None;
            }
            101 if !self.fixture && p.user != self.model.borrow().user => {
                self.enqueue(Command::NewConversation(
                    archaic_matrix::NewConversation::Direct(p.user),
                ));
            }
            102 => {
                if let Some(event_id) = p.receipt
                    && self.enqueue(Command::Context {
                        room_id: p.room,
                        event_id,
                    })
                {
                    self.model.borrow_mut().history_pending =
                        Some(archaic_matrix::HistoryLoad::Latest);
                }
            }
            103 => {
                self.window
                    .clipboard_text(&format!("https://matrix.to/#/{}", p.user))?;
            }
            104 => {
                let mut body = self.view.composer.text()?;
                if !body.is_empty() && !body.ends_with(char::is_whitespace) {
                    body.push(' ');
                }
                body.push_str(&p.user);
                body.push(' ');
                self.view.composer.set_text(&body)?;
                self.composer_input()?;
                self.view.composer.focus()?;
            }
            105 => {
                self.window.clipboard_text(&p.user)?;
            }
            106 if !self.fixture && p.user != self.model.borrow().user => {
                let enabled = !self.model.borrow().blocked.contains(&p.user);
                let id = archaic_matrix::transaction_id();
                if self.enqueue(Command::Organization {
                    id: id.clone(),
                    action: archaic_matrix::organization::Action::Ignore {
                        user: p.user,
                        enabled,
                    },
                }) {
                    self.model.borrow_mut().organization.pending = Some(id);
                }
            }
            _ => {}
        }
        self.render()
    }
}
