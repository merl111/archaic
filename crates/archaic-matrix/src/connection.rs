use crate::{
    homeserver_url,
    storage::{Profile, SavedSession},
};
use matrix_sdk::{
    Client,
    config::{RequestConfig, SyncSettings},
    store::RoomLoadSettings,
};
use std::time::Duration;
use tokio::{sync::mpsc, task::JoinHandle};

#[derive(Clone, Copy)]
pub(crate) enum SyncUpdate {
    Ready,
    Retrying,
    Expired,
}

pub(crate) enum Persistence {
    Temporary,
    Unavailable,
    Ready(Profile),
}
impl Persistence {
    pub async fn profile(&mut self) -> Result<Option<&Profile>, &'static str> {
        if matches!(self, Self::Unavailable) {
            *self = Self::Ready(
                tokio::task::spawn_blocking(Profile::system)
                    .await
                    .map_err(|_| "vault-unavailable")??,
            );
        }
        Ok(match self {
            Self::Ready(profile) => Some(profile),
            _ => None,
        })
    }
}

pub(crate) struct Session {
    pub epoch: u64,
    pub client: Client,
    pub selected: Option<String>,
    pub saved: bool,
    pub revoked: bool,
    sync: JoinHandle<()>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.sync.abort();
    }
}
impl Session {
    pub fn stop_sync(&self) {
        self.sync.abort();
    }
}

fn builder(server: &str) -> Result<matrix_sdk::ClientBuilder, &'static str> {
    let url = homeserver_url(server)?;
    Ok(Client::builder()
        .homeserver_url(url.as_str())
        // The user supplied an explicit, validated base URL. Discovery is a separate future flow.
        .respect_login_well_known(false)
        .request_config(
            RequestConfig::default()
                .timeout(Duration::from_secs(12))
                .retry_limit(0),
        ))
}

pub(crate) async fn restore(
    epoch: u64,
    persistence: &mut Persistence,
    updates: mpsc::Sender<(u64, SyncUpdate)>,
) -> Result<Option<Session>, &'static str> {
    let Some(profile) = persistence.profile().await? else {
        return Ok(None);
    };
    let Some(saved) = profile.load()? else {
        return Ok(None);
    };
    let passphrase = profile.passphrase();
    let client = builder(&saved.server)?
        .sqlite_store(profile.store_path(&saved.store_id)?, Some(&passphrase))
        .build()
        .await
        .map_err(|_| "session-invalid")?;
    client
        .matrix_auth()
        .restore_session(saved.session, RoomLoadSettings::default())
        .await
        .map_err(|_| "session-invalid")?;
    if client.encryption().ed25519_key().await.as_deref() != Some(saved.fingerprint.as_str()) {
        return Err("session-key-mismatch");
    }
    Ok(Some(start(epoch, client, updates, true)))
}

pub(crate) async fn connect(
    epoch: u64,
    server: &str,
    username: &str,
    password: &str,
    persistence: &mut Persistence,
    updates: mpsc::Sender<(u64, SyncUpdate)>,
) -> Result<Session, &'static str> {
    if username.trim().is_empty() || password.is_empty() {
        return Err("missing-credentials");
    }
    let mut builder = builder(server)?;
    let profile = persistence.profile().await?;
    let store_id = if let Some(profile) = profile {
        if profile.load()?.is_some() {
            return Err("session-exists");
        }
        let (id, path) = profile.new_store()?;
        builder = builder.sqlite_store(path, Some(&profile.passphrase()));
        Some(id)
    } else {
        None
    };
    let client = builder.build().await.map_err(|_| "login-failed")?;
    // Refresh tokens are deliberately not requested until token-rotation persistence is supported.
    client
        .matrix_auth()
        .login_username(username.trim(), password)
        .initial_device_display_name("Archaic desktop")
        .await
        .map_err(|_| "login-failed")?;
    let saved = if let (Some(profile), Some(store_id)) = (profile, store_id) {
        let record = SavedSession {
            server: client.homeserver().to_string(),
            fingerprint: client
                .encryption()
                .ed25519_key()
                .await
                .ok_or("session-invalid")?,
            store_id,
            session: client.matrix_auth().session().ok_or("login-failed")?,
        };
        profile.save(Some(&record)).is_ok()
    } else {
        false
    };
    Ok(start(epoch, client, updates, saved))
}

fn start(
    epoch: u64,
    client: Client,
    updates: mpsc::Sender<(u64, SyncUpdate)>,
    saved: bool,
) -> Session {
    let syncing = client.clone();
    let sync = tokio::spawn(async move {
        loop {
            let update = match syncing
                .sync_once(SyncSettings::default().timeout(Duration::from_secs(15)))
                .await
            {
                Ok(_) => SyncUpdate::Ready,
                Err(error)
                    if matches!(
                        error.client_api_error_kind(),
                        Some(matrix_sdk::ruma::api::error::ErrorKind::UnknownToken { .. })
                    ) =>
                {
                    SyncUpdate::Expired
                }
                Err(_) => SyncUpdate::Retrying,
            };
            if updates.send((epoch, update)).await.is_err() || matches!(update, SyncUpdate::Expired)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(
                if matches!(update, SyncUpdate::Ready) {
                    250
                } else {
                    3000
                },
            ))
            .await;
        }
    });
    Session {
        epoch,
        client,
        selected: None,
        saved,
        revoked: false,
        sync,
    }
}
