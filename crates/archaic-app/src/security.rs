use crate::{
    Controller,
    model::{Model, Phase},
    view::set_text,
    widgets::label,
};
use archaic_i18n::Translator;
use archaic_matrix::{
    Command, TransactionId,
    security::{Action, Secret, Snapshot},
    transaction_id,
};
use cui::{Result, Widget, sys};
use std::cell::RefCell;
#[derive(Default)]
pub struct State {
    pub show: bool,
    pub mode: i32,
    pub pending: Option<TransactionId>,
    pub dialog: bool,
    pub snapshot: Snapshot,
    pub error: Option<&'static str>,
    pub key: Option<Secret>,
}
pub struct Controls {
    root: Widget,
    pub mode: Widget,
    pub close: Widget,
    pub refresh: Widget,
    summary: Widget,
    error: Widget,
    verification: Widget,
    pub devices: Widget,
    pub verify: Widget,
    flow: Widget,
    pub accept: Widget,
    pub start: Widget,
    pub confirm: Widget,
    pub mismatch: Widget,
    pub cancel: Widget,
    recovery: Widget,
    backup_status: Widget,
    pub password: Widget,
    pub recovery_key: Widget,
    pub bootstrap: Widget,
    pub enable: Widget,
    pub recover: Widget,
    key: Widget,
    pub dismiss_key: Widget,
    keys: Widget,
    pub passphrase: Widget,
    pub export: Widget,
    pub import: Widget,
    pub room_keys: Widget,
    device_labels: RefCell<Vec<String>>,
    device_ids: RefCell<Vec<String>>,
}
impl Controls {
    pub fn new(parent: &Widget, tr: &Translator) -> Result<Self> {
        let root = parent.box_layout(sys::CUI_VERTICAL, 12)?;
        root.box_set_padding(0)?;
        root.expand(true)?;
        let header = root.box_layout(sys::CUI_HORIZONTAL, 12)?;
        let summary = label(&header, "", sys::CUI_ROLE_CAPTION)?;
        summary.expand(true)?;
        let refresh = header.button(&tr.text("refresh"))?;
        let close = header.button(&tr.text("security-close"))?;
        close.set_visible(false)?;
        let mode = root.select(&[
            &tr.text("security-devices"),
            &tr.text("security-recovery"),
            &tr.text("security-keys"),
        ])?;
        mode.set_selected(0)?;
        let verification = root.box_layout(sys::CUI_VERTICAL, 10)?;
        let devices = verification.select(&[])?;
        devices.accessibility(&tr.text("security-devices"), "")?;
        let verify = verification.button(&tr.text("security-verify"))?;
        let flow = verification.textarea("")?;
        flow.set_read_only(true)?;
        flow.expand(true)?;
        let actions = verification.box_layout(sys::CUI_HORIZONTAL, 8)?;
        let accept = actions.button(&tr.text("security-accept"))?;
        let start = actions.button(&tr.text("security-start"))?;
        let confirm = actions.button(&tr.text("security-confirm"))?;
        let mismatch = actions.button(&tr.text("security-mismatch"))?;
        let cancel = actions.button(&tr.text("cancel"))?;
        let recovery = root.box_layout(sys::CUI_VERTICAL, 10)?;
        let backup_status = label(&recovery, "", sys::CUI_ROLE_BODY)?;
        label(
            &recovery,
            &tr.text("security-recovery-hint"),
            sys::CUI_ROLE_CAPTION,
        )?;
        label(&recovery, &tr.text("password"), sys::CUI_ROLE_CAPTION)?;
        let password = recovery.password("")?;
        let bootstrap = recovery.button(&tr.text("security-bootstrap"))?;
        label(
            &recovery,
            &tr.text("security-recovery-key"),
            sys::CUI_ROLE_CAPTION,
        )?;
        let recovery_key = recovery.password("")?;
        let row = recovery.box_layout(sys::CUI_HORIZONTAL, 8)?;
        let recover = row.button(&tr.text("security-recover"))?;
        let enable = row.button(&tr.text("security-enable"))?;
        let key = recovery.textarea("")?;
        key.set_read_only(true)?;
        let dismiss_key = recovery.button(&tr.text("security-key-saved"))?;
        let keys = root.box_layout(sys::CUI_VERTICAL, 10)?;
        label(
            &keys,
            &tr.text("security-keyfile-hint"),
            sys::CUI_ROLE_CAPTION,
        )?;
        label(
            &keys,
            &tr.text("security-keyfile-password"),
            sys::CUI_ROLE_CAPTION,
        )?;
        let passphrase = keys.password("")?;
        passphrase.accessibility(&tr.text("security-keyfile-password"), "")?;
        let row = keys.box_layout(sys::CUI_HORIZONTAL, 8)?;
        let export = row.button(&tr.text("security-export"))?;
        let import = row.button(&tr.text("security-import"))?;
        let room_keys = recovery.button(&tr.text("security-room-keys"))?;
        let error = label(&root, "", sys::CUI_ROLE_DANGER)?;
        Ok(Self {
            root,
            mode,
            close,
            refresh,
            summary,
            error,
            verification,
            devices,
            verify,
            flow,
            accept,
            start,
            confirm,
            mismatch,
            cancel,
            recovery,
            backup_status,
            password,
            recovery_key,
            bootstrap,
            enable,
            recover,
            key,
            dismiss_key,
            keys,
            passphrase,
            export,
            import,
            room_keys,
            device_labels: RefCell::new(vec![]),
            device_ids: RefCell::new(vec![]),
        })
    }
    pub fn clear(&self) -> Result<()> {
        for field in [
            &self.password,
            &self.recovery_key,
            &self.passphrase,
            &self.key,
        ] {
            field.set_text("")?;
        }
        Ok(())
    }
    pub fn render(&self, m: &Model, tr: &Translator, fixture: bool) -> Result<()> {
        self.root
            .set_visible(m.security.show && m.phase == Phase::Connected)?;
        if !m.security.show {
            return self.key.set_text("");
        }
        let s = &m.security;
        let enabled = s.pending.is_none() && !s.dialog && !fixture && m.phase == Phase::Connected;
        self.mode.set_selected(s.mode)?;
        self.mode.set_enabled(enabled && s.key.is_none())?;
        self.verification.set_visible(s.mode == 0)?;
        self.recovery.set_visible(s.mode == 1)?;
        set_text(
            &self.backup_status,
            &tr.text(if s.snapshot.backup {
                "security-backup-active"
            } else {
                match s.snapshot.backup_exists {
                    Some(true) => "security-backup-locked",
                    Some(false) => "security-backup-none",
                    None => "security-backup-unknown",
                }
            }),
        )?;
        self.keys.set_visible(s.mode == 2)?;
        self.close.set_enabled(enabled && s.key.is_none())?;
        self.refresh.set_enabled(enabled)?;
        set_text(
            &self.summary,
            &format!(
                "{}\n{}: {} · {}: {}",
                tr.text(if s.snapshot.recovery.is_empty() {
                    "security-recovery-unknown"
                } else {
                    s.snapshot.recovery
                }),
                tr.text("security-identity"),
                tr.text(if s.snapshot.identity {
                    "security-available"
                } else {
                    "security-missing"
                }),
                tr.text("security-backup"),
                tr.text(if s.snapshot.backup {
                    "security-available"
                } else {
                    "security-missing"
                })
            ),
        )?;
        let names = s
            .snapshot
            .devices
            .iter()
            .map(|d| {
                format!(
                    "{} · {} · {}",
                    d.id,
                    d.name,
                    tr.text(if d.current {
                        "security-current"
                    } else if d.verified {
                        "security-verified"
                    } else {
                        "security-unverified"
                    })
                )
                .replace('\0', "�")
            })
            .collect::<Vec<_>>();
        if *self.device_labels.borrow() != names {
            let selected = usize::try_from(self.devices.get_selected()?)
                .ok()
                .and_then(|i| self.device_ids.borrow().get(i).cloned());
            let index = selected
                .and_then(|id| s.snapshot.devices.iter().position(|d| d.id == id))
                .or_else(|| s.snapshot.devices.iter().position(|d| !d.current))
                .or_else(|| (!s.snapshot.devices.is_empty()).then_some(0));

            self.devices
                .set_items(&names.iter().map(String::as_str).collect::<Vec<_>>())?;
            self.devices
                .set_selected(index.map(|i| i as i32).unwrap_or(-1))?;
            *self.device_ids.borrow_mut() =
                s.snapshot.devices.iter().map(|d| d.id.clone()).collect();
            *self.device_labels.borrow_mut() = names;
        }
        self.devices.set_enabled(enabled)?;
        let selected = usize::try_from(self.devices.get_selected()?)
            .ok()
            .and_then(|i| s.snapshot.devices.get(i));
        self.verify
            .set_enabled(enabled && selected.is_some_and(|d| !d.current))?;
        let state = s.snapshot.flow.as_ref().map(|f| f.state).unwrap_or("");
        self.accept
            .set_enabled(enabled && state == "security-verification-requested")?;
        self.start.set_enabled(
            enabled
                && matches!(
                    state,
                    "security-verification-ready" | "security-verification-start"
                ),
        )?;
        self.confirm
            .set_enabled(enabled && state == "security-verification-compare")?;
        self.mismatch
            .set_enabled(enabled && state == "security-verification-compare")?;
        self.cancel.set_enabled(
            enabled
                && !matches!(
                    state,
                    "" | "security-verification-done" | "security-verification-cancelled"
                ),
        )?;
        set_text(
            &self.flow,
            &s.snapshot
                .flow
                .as_ref()
                .map(|f| {
                    format!(
                        "{} · {}\n{}\n\n{}\n\n{}",
                        f.user,
                        f.device,
                        tr.text(f.state),
                        f.codes,
                        tr.text("security-compare-hint")
                    )
                })
                .unwrap_or_else(|| tr.text("security-verification-none")),
        )?;
        for control in [
            &self.password,
            &self.recovery_key,
            &self.bootstrap,
            &self.recover,
            &self.passphrase,
            &self.export,
            &self.import,
        ] {
            control.set_enabled(enabled && s.key.is_none())?;
        }
        self.enable.set_enabled(
            enabled
                && s.key.is_none()
                && s.snapshot.identity
                && s.snapshot.recovery == "security-recovery-disabled",
        )?;
        self.room_keys
            .set_enabled(enabled && m.selected.is_some() && s.snapshot.backup)?;
        self.key.set_visible(s.key.is_some())?;
        self.dismiss_key.set_visible(s.key.is_some())?;
        set_text(&self.key, s.key.as_ref().map(Secret::expose).unwrap_or(""))?;
        self.error
            .set_visible(s.error.is_some() || s.pending.is_some() || s.dialog)?;
        set_text(
            &self.error,
            &s.error.map(|k| tr.text(k)).unwrap_or_else(|| {
                if enabled {
                    String::new()
                } else {
                    tr.text("tool-working")
                }
            }),
        )
    }
}
impl Controller {
    fn security_send(&self, action: Action) -> Result<()> {
        let m = self.model.borrow();
        if self.fixture
            || m.phase != Phase::Connected
            || m.security.pending.is_some()
            || m.security.dialog
        {
            return Ok(());
        }
        drop(m);
        let id = transaction_id();
        if self.enqueue(Command::Security {
            id: id.clone(),
            action,
        }) {
            let mut m = self.model.borrow_mut();
            m.security.pending = Some(id);
            m.security.error = None;
        }
        self.render()
    }
    pub fn open_security(&self) -> Result<()> {
        self.remember_draft()?;
        let m = self.model.borrow();
        if m.tools_busy() || m.sending || m.message_draft.is_some() || m.phase != Phase::Connected {
            return Ok(());
        }
        drop(m);
        self.view.settings.select(3)?;
        self.view.ui.panel.set(crate::daylight::Panel::Account);
        self.model.borrow_mut().security.show = true;
        self.security_send(Action::Refresh)
    }
    pub fn close_security(&self) -> Result<()> {
        if self.model.borrow().security.key.is_some()
            || self.model.borrow().security.pending.is_some()
            || self.model.borrow().security.dialog
        {
            return Ok(());
        }
        self.view.security.clear()?;
        self.model.borrow_mut().security.show = false;
        self.view.ui.panel.set(crate::daylight::Panel::None);
        self.security_send(Action::Watch(false))
    }
    pub fn refresh_security(&self) -> Result<()> {
        self.security_send(Action::Refresh)
    }
    pub fn security_mode(&self) -> Result<()> {
        self.model.borrow_mut().security.mode = self.view.security.mode.get_selected()?.clamp(0, 2);
        self.render()
    }
    pub fn security_selection(&self) -> Result<()> {
        self.render()
    }
    pub fn verify_device(&self) -> Result<()> {
        let index = self.view.security.devices.get_selected()?;
        let device = usize::try_from(index).ok().and_then(|i| {
            self.model
                .borrow()
                .security
                .snapshot
                .devices
                .get(i)
                .cloned()
        });
        if let Some(device) = device {
            self.security_send(Action::VerifyDevice(device.id))?;
        }
        Ok(())
    }
    fn verification_action(&self, action: fn(String) -> Action) -> Result<()> {
        let flow = self.model.borrow().security.snapshot.flow.clone();
        if let Some(flow) = flow {
            self.security_send(action(flow.id))?;
        }
        Ok(())
    }
    pub fn accept_verification(&self) -> Result<()> {
        self.verification_action(Action::Accept)
    }
    pub fn start_verification(&self) -> Result<()> {
        self.verification_action(Action::Start)
    }
    pub fn confirm_verification(&self) -> Result<()> {
        self.verification_action(Action::Confirm)
    }
    pub fn mismatch_verification(&self) -> Result<()> {
        self.verification_action(Action::Mismatch)
    }
    pub fn cancel_verification(&self) -> Result<()> {
        self.verification_action(Action::Cancel)
    }
    pub fn bootstrap_security(&self) -> Result<()> {
        let secret = Secret::new(self.view.security.password.text()?);
        self.view.security.password.set_text("")?;
        self.security_send(Action::Bootstrap(secret))
    }
    pub fn recover_security(&self) -> Result<()> {
        let secret = Secret::new(self.view.security.recovery_key.text()?);
        self.view.security.recovery_key.set_text("")?;
        self.security_send(Action::Recover(secret))
    }
    pub fn enable_recovery(&self) -> Result<()> {
        self.security_send(Action::EnableRecovery)
    }
    pub fn dismiss_recovery_key(&self) -> Result<()> {
        self.model.borrow_mut().security.key = None;
        self.view.security.key.set_text("")?;
        self.render()
    }
    pub fn recover_room_keys(&self) -> Result<()> {
        let id = self.model.borrow().selected.clone();
        if let Some(id) = id {
            self.security_send(Action::RecoverRoom(id))?;
        }
        Ok(())
    }
    pub fn export_keys(&self) -> Result<()> {
        self.security_file(true)
    }
    pub fn import_keys(&self) -> Result<()> {
        self.security_file(false)
    }
    fn security_file(&self, export: bool) -> Result<()> {
        let passphrase = Secret::new(self.view.security.passphrase.text()?);
        self.view.security.passphrase.set_text("")?;
        if passphrase.expose().is_empty() {
            self.model.borrow_mut().security.error = Some("security-secret-required");
            return self.render();
        }
        let epoch = self.model.borrow().epoch;
        self.model.borrow_mut().security.dialog = true;
        let weak = self.self_weak.borrow().clone();
        let result = self.window.file_dialog(
            if export {
                sys::CUI_DIALOG_SAVE
            } else {
                sys::CUI_DIALOG_OPEN
            },
            &self.tr.text(if export {
                "security-export"
            } else {
                "security-import"
            }),
            "",
            move |_, result, path| {
                if let Some(c) = weak.upgrade() {
                    if c.model.borrow().epoch != epoch {
                        return;
                    }
                    c.model.borrow_mut().security.dialog = false;
                    if result == sys::CUI_DIALOG_ACCEPTED {
                        let action = if export {
                            Action::Export {
                                path: path.into(),
                                passphrase: passphrase.clone(),
                            }
                        } else {
                            Action::Import {
                                path: path.into(),
                                passphrase: passphrase.clone(),
                            }
                        };
                        c.security_send(action).expect("live controls");
                    } else {
                        c.render().expect("live controls");
                    }
                }
            },
        );
        if result.is_err() {
            self.model.borrow_mut().security.dialog = false;
            self.model.borrow_mut().security.error = Some("file-dialog-failed");
        }
        self.render()
    }
}

impl State {
    pub fn complete(
        &mut self,
        id: &archaic_matrix::TransactionId,
        result: std::result::Result<Option<Secret>, &'static str>,
    ) -> Option<&'static str> {
        if self.pending.as_ref() != Some(id) {
            return None;
        }
        self.pending = None;
        match result {
            Ok(key) => {
                self.error = None;
                if key.is_some() {
                    self.key = key;
                }
                None
            }
            Err(key) => {
                self.error = Some(key);
                (key == "session-expired").then_some(key)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_security_result_cannot_replace_another_operations_key() {
        let id = archaic_matrix::transaction_id();
        let mut state = State {
            pending: Some(id.clone()),
            ..Default::default()
        };
        assert_eq!(
            state.complete(
                &archaic_matrix::transaction_id(),
                Ok(Some(Secret::new("stale".into())))
            ),
            None
        );
        assert_eq!(state.pending, Some(id.clone()));
        assert!(state.key.is_none());
        assert_eq!(
            state.complete(&id, Ok(Some(Secret::new("current".into())))),
            None
        );
        assert_eq!(state.key.as_ref().unwrap().expose(), "current");
        assert!(state.pending.is_none());
        let id = archaic_matrix::transaction_id();
        state.pending = Some(id.clone());
        assert_eq!(
            state.complete(&id, Err("session-expired")),
            Some("session-expired")
        );
        assert_eq!(state.error, Some("session-expired"));
    }
}
