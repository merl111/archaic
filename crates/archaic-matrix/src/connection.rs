use crate::{
    homeserver_url,
    storage::{Profile, SavedSession},
};
use matrix_sdk::{
    Client,
    config::{RequestConfig, SyncSettings},
    store::RoomLoadSettings,
};
use std::{sync::Arc, time::Duration};
use tokio::{sync::mpsc, task::JoinHandle};

pub(crate) enum SyncUpdate {
    Ready(Box<matrix_sdk::sync::SyncResponse>),
    Outbox,
    Retrying,
    Expired,
}

pub(crate) enum Persistence {
    Temporary,
    Unavailable,
    Ready(Arc<Profile>),
}
impl Persistence {
    pub async fn profile(&mut self) -> Result<Option<&Arc<Profile>>, &'static str> {
        if matches!(self, Self::Unavailable) {
            *self = Self::Ready(
                tokio::task::spawn_blocking(Profile::system)
                    .await
                    .map_err(|_| "vault-unavailable")??
                    .into(),
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
    pub token_store: Option<Arc<crate::token_store::TokenStore>>,
    pub history: crate::history::History,
    pub tools: crate::room_tools::RoomTools,
    pub security: crate::security::Security,
    pub conversations: crate::conversations::Conversations,
    pub revoked: bool,
    pub reconnecting: bool,
    sync: JoinHandle<()>,
    queue_updates: JoinHandle<()>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.sync.abort();
        self.queue_updates.abort();
        if let Some(store) = &self.token_store {
            store.close();
        }
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
        .handle_refresh_tokens()
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
        .restore_session(saved.session.clone(), RoomLoadSettings::default())
        .await
        .map_err(|_| "session-invalid")?;
    if client.encryption().ed25519_key().await.as_deref() != Some(saved.fingerprint.as_str()) {
        return Err("session-key-mismatch");
    }
    let token_store = crate::token_store::TokenStore::attach(&client, profile.clone(), saved)?;
    Ok(Some(start(epoch, client, updates, true, Some(token_store))))
}

pub(crate) async fn connect(
    epoch: u64,
    server: &str,
    username: &str,
    password: &str,
    persistence: &mut Persistence,
    updates: mpsc::Sender<(u64, SyncUpdate)>,
) -> Result<Session, &'static str> {
    connect_with(
        epoch,
        server,
        Some((username, password)),
        persistence,
        updates,
        None,
    )
    .await
}
pub(crate) async fn browser_login(
    epoch: u64,
    server: &str,
    persistence: &mut Persistence,
    updates: mpsc::Sender<(u64, SyncUpdate)>,
    events: mpsc::Sender<crate::Event>,
) -> Result<Session, &'static str> {
    tokio::time::timeout(
        Duration::from_secs(300),
        connect_with(epoch, server, None, persistence, updates, Some(events)),
    )
    .await
    .map_err(|_| "sso-timeout")?
}
async fn connect_with(
    epoch: u64,
    server: &str,
    credentials: Option<(&str, &str)>,
    persistence: &mut Persistence,
    updates: mpsc::Sender<(u64, SyncUpdate)>,
    events: Option<mpsc::Sender<crate::Event>>,
) -> Result<Session, &'static str> {
    if credentials
        .is_some_and(|(username, password)| username.trim().is_empty() || password.is_empty())
    {
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
    if let Some((username, password)) = credentials {
        client
            .matrix_auth()
            .login_username(username.trim(), password)
            .initial_device_display_name("Archaic desktop")
            .request_refresh_token()
            .await
            .map_err(|_| "login-failed")?;
    } else {
        let flows = client
            .matrix_auth()
            .get_login_types()
            .await
            .map_err(|_| "login-failed")?;
        if !flows.flows.iter().any(|flow| {
            matches!(
                flow,
                matrix_sdk::ruma::api::client::session::get_login_types::v3::LoginType::Sso(_)
            )
        }) {
            return Err("sso-unavailable");
        }
        let events = events.ok_or("login-failed")?;
        client
            .matrix_auth()
            .login_sso(move |url| async move {
                events
                    .send(crate::Event {
                        epoch,
                        kind: crate::EventKind::BrowserUrl(url),
                    })
                    .await
                    .map_err(|_| std::io::Error::other("login closed"))?;
                Ok(())
            })
            .initial_device_display_name("Archaic desktop")
            .request_refresh_token()
            .await
            .map_err(|_| "login-failed")?;
    }
    let mut token_store = None;
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
        let saved = profile.save(Some(&record)).is_ok();
        token_store = Some(crate::token_store::TokenStore::attach(
            &client,
            profile.clone(),
            record,
        )?);
        saved
    } else {
        false
    };
    Ok(start(epoch, client, updates, saved, token_store))
}

fn start(
    epoch: u64,
    client: Client,
    updates: mpsc::Sender<(u64, SyncUpdate)>,
    saved: bool,
    token_store: Option<Arc<crate::token_store::TokenStore>>,
) -> Session {
    let security = crate::security::Security::new(&client);
    let queue_updates = watch_outbox(&client, epoch, updates.clone());
    let syncing = client.clone();
    let sync = tokio::spawn(async move {
        loop {
            let update = match syncing
                .sync_once(SyncSettings::default().timeout(Duration::from_secs(15)))
                .await
            {
                Ok(response) => SyncUpdate::Ready(Box::new(response)),
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
            let expired = matches!(update, SyncUpdate::Expired);
            let ready = matches!(update, SyncUpdate::Ready(_));
            if updates.send((epoch, update)).await.is_err() || expired {
                break;
            }
            tokio::time::sleep(Duration::from_millis(if ready { 250 } else { 3000 })).await;
        }
    });
    Session {
        epoch,
        client,
        security,
        selected: None,
        history: crate::history::History::default(),
        tools: crate::room_tools::RoomTools::default(),
        conversations: crate::conversations::Conversations::default(),
        saved,
        token_store,
        revoked: false,
        reconnecting: false,
        queue_updates,
        sync,
    }
}

fn watch_outbox(
    client: &Client,
    epoch: u64,
    updates: mpsc::Sender<(u64, SyncUpdate)>,
) -> JoinHandle<()> {
    let mut queue_rx = client.send_queue().subscribe();
    let queue_client = client.clone();
    let queue_tx = updates.clone();
    tokio::spawn(async move {
        queue_client
            .send_queue()
            .respawn_tasks_for_rooms_with_unsent_requests()
            .await;
        if queue_tx.send((epoch, SyncUpdate::Outbox)).await.is_err() {
            return;
        }
        loop {
            match queue_rx.recv().await {
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                _ => {
                    if queue_tx.send((epoch, SyncUpdate::Outbox)).await.is_err() {
                        break;
                    }
                }
            }
        }
    })
}
