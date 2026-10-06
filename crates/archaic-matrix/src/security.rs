//! Native account security uses SDK trust state and never offers manual trust overrides.
use crate::message_actions::failure;
use matrix_sdk::{
    Client,
    encryption::verification::{SasState, VerificationRequest, VerificationRequestState},
    ruma::{
        OwnedUserId, UserId,
        api::client::uiaa::{AuthData, Password, UserIdentifier},
        events::key::verification::{
            VerificationMethod, request::ToDeviceKeyVerificationRequestEvent,
        },
    },
};
use std::sync::{Arc, Mutex};
#[derive(Clone, Default)]
pub struct Secret(zeroize::Zeroizing<String>);
impl Secret {
    pub fn new(value: String) -> Self {
        Self(zeroize::Zeroizing::new(value))
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[redacted]")
    }
}
pub enum Action {
    Refresh,
    Watch(bool),
    VerifyDevice(String),
    Accept(String),
    Start(String),
    Confirm(String),
    Mismatch(String),
    Cancel(String),
    Bootstrap(Secret),
    EnableRecovery,
    Recover(Secret),
    RecoverRoom(String),
    Export {
        path: std::path::PathBuf,
        passphrase: Secret,
    },
    Import {
        path: std::path::PathBuf,
        passphrase: Secret,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub verified: bool,
    pub current: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub devices: Vec<Device>,
    pub recovery: &'static str,
    pub backup: bool,
    pub backup_exists: Option<bool>,
    pub identity: bool,
    pub flow: Option<Flow>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Flow {
    pub id: String,
    pub user: String,
    pub device: String,
    pub state: &'static str,
    pub codes: String,
}
pub(crate) struct Security {
    pub watching: bool,
    devices: Vec<Device>,
    active: Option<VerificationRequest>,
    incoming: Arc<Mutex<Vec<(OwnedUserId, String)>>>,
}
impl Security {
    pub fn new(client: &Client) -> Self {
        let incoming = Arc::new(Mutex::new(Vec::new()));
        let queue = incoming.clone();
        client.add_event_handler(move |event: ToDeviceKeyVerificationRequestEvent| {
            let queue = queue.clone();
            async move {
                let mut pending = queue.lock().unwrap_or_else(|p| p.into_inner());
                if pending.len() < 32 {
                    pending.push((event.sender, event.content.transaction_id.to_string()));
                }
            }
        });
        Self {
            watching: false,
            devices: vec![],
            active: None,
            incoming,
        }
    }
    async fn refresh_devices(&mut self, client: &Client) -> Result<(), &'static str> {
        let own = client.user_id().ok_or("session-expired")?;
        let response = client.devices().await.map_err(|e| failure(e.into()))?;
        let devices = client
            .encryption()
            .get_user_devices(own)
            .await
            .map_err(failure)?;
        self.devices = response
            .devices
            .into_iter()
            .map(|d| Device {
                current: client.device_id() == Some(d.device_id.as_ref()),
                verified: devices.get(&d.device_id).is_some_and(|d| d.is_verified()),
                name: d.display_name.unwrap_or_default(),
                id: d.device_id.to_string(),
            })
            .collect();
        self.devices.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(())
    }
    pub async fn snapshot(&mut self, client: &Client) -> Snapshot {
        if self
            .active
            .as_ref()
            .is_none_or(|r| r.is_done() || r.is_cancelled())
        {
            let next = self
                .incoming
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .pop();
            if let Some((user, id)) = next {
                self.active = client
                    .encryption()
                    .get_verification_request(&user, id)
                    .await;
            }
        }
        let encryption = client.encryption();
        Snapshot {
            devices: self.devices.clone(),
            recovery: match encryption.recovery().state() {
                matrix_sdk::encryption::recovery::RecoveryState::Enabled => {
                    "security-recovery-enabled"
                }
                matrix_sdk::encryption::recovery::RecoveryState::Disabled => {
                    "security-recovery-disabled"
                }
                matrix_sdk::encryption::recovery::RecoveryState::Incomplete => {
                    "security-recovery-incomplete"
                }
                _ => "security-recovery-unknown",
            },
            backup: encryption.backups().are_enabled().await,
            backup_exists: encryption.backups().exists_on_server().await.ok(),
            identity: encryption
                .cross_signing_status()
                .await
                .is_some_and(|s| s.has_master && s.has_self_signing && s.has_user_signing),
            flow: self.active.as_ref().map(flow),
        }
    }
    fn request(&self, id: &str) -> Result<&VerificationRequest, &'static str> {
        self.active
            .as_ref()
            .filter(|r| r.flow_id() == id && !r.is_done() && !r.is_cancelled())
            .ok_or("security-flow-stale")
    }
    pub async fn perform(
        &mut self,
        client: &Client,
        action: Action,
    ) -> Result<Option<Secret>, &'static str> {
        match action {
            Action::Watch(watching) => self.watching = watching,
            Action::Refresh => {
                self.watching = true;
                self.refresh_devices(client).await?;
            }
            Action::VerifyDevice(id) => {
                if self
                    .active
                    .as_ref()
                    .is_some_and(|r| !r.is_done() && !r.is_cancelled())
                {
                    return Err("security-flow-active");
                }
                let own = client.user_id().ok_or("session-expired")?;
                if client.device_id().is_some_and(|d| d.as_str() == id) {
                    return Err("security-device-invalid");
                }
                let device = client
                    .encryption()
                    .get_device(own, id.as_str().into())
                    .await
                    .map_err(|_| "security-failed")?
                    .ok_or("security-device-invalid")?;
                self.active = Some(
                    device
                        .request_verification_with_methods(vec![VerificationMethod::SasV1])
                        .await
                        .map_err(failure)?,
                );
            }
            Action::Accept(id) => self
                .request(&id)?
                .accept_with_methods(vec![VerificationMethod::SasV1])
                .await
                .map_err(failure)?,
            Action::Start(id) => {
                let request = self.request(&id)?;
                if let VerificationRequestState::Transitioned { verification } = request.state() {
                    verification
                        .sas()
                        .ok_or("security-flow-stale")?
                        .accept()
                        .await
                        .map_err(failure)?;
                } else {
                    request
                        .start_sas()
                        .await
                        .map_err(failure)?
                        .ok_or("security-flow-stale")?;
                }
            }
            Action::Confirm(id) => {
                let VerificationRequestState::Transitioned { verification } =
                    self.request(&id)?.state()
                else {
                    return Err("security-flow-stale");
                };
                let sas = verification.sas().ok_or("security-flow-stale")?;
                if !matches!(sas.state(), SasState::KeysExchanged { .. }) {
                    return Err("security-flow-stale");
                }
                sas.confirm().await.map_err(failure)?;
            }
            Action::Mismatch(id) => {
                let VerificationRequestState::Transitioned { verification } =
                    self.request(&id)?.state()
                else {
                    return Err("security-flow-stale");
                };
                verification
                    .sas()
                    .ok_or("security-flow-stale")?
                    .mismatch()
                    .await
                    .map_err(failure)?;
            }
            Action::Cancel(id) => self.request(&id)?.cancel().await.map_err(failure)?,
            Action::Bootstrap(password) => bootstrap(client, password.expose()).await?,
            Action::EnableRecovery => {
                let encryption = client.encryption();
                if encryption
                    .secret_storage()
                    .is_enabled()
                    .await
                    .map_err(failure)?
                {
                    return Err("security-recover-first");
                }
                if !encryption
                    .cross_signing_status()
                    .await
                    .is_some_and(|s| s.has_master && s.has_self_signing && s.has_user_signing)
                {
                    return Err("security-bootstrap-first");
                }
                if !encryption.backups().are_enabled().await
                    && encryption
                        .backups()
                        .fetch_exists_on_server()
                        .await
                        .map_err(failure)?
                {
                    return Err("security-backup-unlock-first");
                }
                return encryption
                    .recovery()
                    .enable()
                    .await
                    .map(|s| Some(Secret::new(s)))
                    .map_err(|_| "security-failed");
            }
            Action::Recover(key) => {
                if key.expose().trim().is_empty() {
                    return Err("security-secret-required");
                }
                client
                    .encryption()
                    .recovery()
                    .recover(key.expose())
                    .await
                    .map_err(|_| "security-recovery-failed")?;
            }
            Action::RecoverRoom(id) => {
                let room = crate::room_tools::joined(client, &id)?;
                client
                    .encryption()
                    .backups()
                    .download_room_keys_for_room(room.room_id())
                    .await
                    .map_err(failure)?;
            }
            Action::Export { path, passphrase } => export(client, path, passphrase).await?,
            Action::Import { path, passphrase } => {
                let info = std::fs::metadata(&path).map_err(|_| "file-read-failed")?;
                if !info.is_file() || info.len() > 25 * 1024 * 1024 {
                    return Err("file-size-limit");
                }
                if passphrase.expose().is_empty() {
                    return Err("security-secret-required");
                }
                client
                    .encryption()
                    .import_room_keys(path, passphrase.expose())
                    .await
                    .map_err(|_| "security-import-failed")?;
            }
        }
        Ok(None)
    }
}
fn flow(request: &VerificationRequest) -> Flow {
    let mut result = Flow {
        id: request.flow_id().into(),
        user: request.other_user_id().to_string(),
        device: String::new(),
        state: "security-verification-waiting",
        codes: String::new(),
    };
    match request.state() {
        VerificationRequestState::Requested {
            other_device_data, ..
        } => {
            result.state = "security-verification-requested";
            result.device = other_device_data.device_id().to_string();
        }
        VerificationRequestState::Ready {
            other_device_data, ..
        } => {
            result.state = "security-verification-ready";
            result.device = other_device_data.device_id().to_string();
        }
        VerificationRequestState::Transitioned { verification } => {
            if let Some(sas) = verification.sas() {
                result.device = sas.other_device().device_id().to_string();
                result.state = match sas.state() {
                    SasState::Started { .. } => "security-verification-start",
                    SasState::KeysExchanged { .. } => "security-verification-compare",
                    SasState::Confirmed => "security-verification-confirmed",
                    SasState::Done { .. } => "security-verification-done",
                    SasState::Cancelled(_) => "security-verification-cancelled",
                    _ => "security-verification-waiting",
                };
                if matches!(sas.state(), SasState::KeysExchanged { .. }) {
                    if let Some(emojis) = sas.emoji() {
                        result.codes = emojis
                            .iter()
                            .map(|e| format!("{} {}", e.symbol, e.description))
                            .collect::<Vec<_>>()
                            .join("   ");
                    }
                    if let Some((a, b, c)) = sas.decimals() {
                        result.codes.push_str(&format!("\n{a}  {b}  {c}"));
                    }
                }
            }
        }
        VerificationRequestState::Done => result.state = "security-verification-done",
        VerificationRequestState::Cancelled(_) => result.state = "security-verification-cancelled",
        _ => {}
    }
    result
}
async fn bootstrap(client: &Client, password: &str) -> Result<(), &'static str> {
    match client
        .encryption()
        .bootstrap_cross_signing_if_needed(None)
        .await
    {
        Ok(()) => Ok(()),
        Err(error) => {
            let Some(info) = error.as_uiaa_response() else {
                return Err(failure(error));
            };
            if password.is_empty() {
                return Err("security-secret-required");
            }
            let own = UserId::parse(client.user_id().ok_or("session-expired")?.as_str())
                .map_err(|_| "security-failed")?;
            let mut auth = Password::new(UserIdentifier::from(own), password.to_owned());
            auth.session = info.session.clone();
            client
                .encryption()
                .bootstrap_cross_signing_if_needed(Some(AuthData::Password(auth)))
                .await
                .map_err(failure)
        }
    }
}
async fn export(
    client: &Client,
    path: std::path::PathBuf,
    passphrase: Secret,
) -> Result<(), &'static str> {
    if passphrase.expose().chars().count() < 12 {
        return Err("security-export-password");
    }
    if path.exists() {
        return Err("file-exists");
    }
    let parent = path.parent().ok_or("file-save-failed")?;
    let file = tempfile::NamedTempFile::new_in(parent).map_err(|_| "file-save-failed")?;
    client
        .encryption()
        .export_room_keys(file.path().to_owned(), passphrase.expose(), |_| true)
        .await
        .map_err(failure)?;
    file.as_file().sync_all().map_err(|_| "file-save-failed")?;
    file.persist_noclobber(path)
        .map_err(|_| "file-save-failed")?;
    Ok(())
}
