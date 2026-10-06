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
    LocalEcho(Box<crate::outbox::Item>),
    Delivery {
        room_id: String,
        transaction: String,
        event_id: Option<String>,
    },
    Archived(String),
    HistorySync(crate::HistorySyncStatus),
    RoomKeys(Option<std::collections::HashSet<String>>),
    Progress {
        room_id: String,
        id: crate::TransactionId,
        current: u64,
        total: u64,
    },
    Retrying,
    Expired(bool),
}

pub(crate) enum Persistence {
    Temporary,
    Unavailable(String),
    Ready(Arc<Profile>),
}
impl Persistence {
    pub async fn profile(&mut self) -> Result<Option<&Arc<Profile>>, &'static str> {
        if let Self::Unavailable(name) = self {
            let name = name.clone();
            *self = Self::Ready(
                tokio::task::spawn_blocking(move || Profile::system_named(&name))
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
    pub avatars: crate::avatars::Avatars,
    pub transfers: crate::transfers::Transfers,
    pub local: crate::local_data::LocalData,
    pub history: crate::history::History,
    pub history_cache: std::collections::VecDeque<crate::history::History>,
    pub tools: crate::room_tools::RoomTools,
    pub security: crate::security::Security,
    pub conversations: crate::conversations::Conversations,
    pub revoked: bool,
    pub soft_logout: bool,
    pub blocked: std::collections::HashSet<String>,
    pub notified: std::collections::HashSet<String>,
    pub initial_sync_done: bool,
    pub reconnecting: bool,
    archive_sync: Option<JoinHandle<()>>,
    sync: JoinHandle<()>,
    queue_updates: JoinHandle<()>,
    room_keys: JoinHandle<()>,
}
impl Drop for Session {
    fn drop(&mut self) {
        if let Some(job) = &self.archive_sync {
            job.abort();
        }
        self.sync.abort();
        self.room_keys.abort();
        self.queue_updates.abort();
        if let Some(store) = &self.token_store {
            store.close();
        }
    }
}
impl Session {
    pub fn stop_sync(&self) {
        if let Some(job) = &self.archive_sync {
            job.abort();
        }
        self.sync.abort();
        self.room_keys.abort();
    }
}

fn builder(server: &str) -> Result<matrix_sdk::ClientBuilder, &'static str> {
    let url = homeserver_url(server)?;
    Ok(Client::builder()
        .homeserver_url(url.as_str())
        // The user supplied an explicit, validated base URL. Discovery is a separate future flow.
        .respect_login_well_known(false)
        .handle_refresh_tokens()
        .with_encryption_settings(matrix_sdk::encryption::EncryptionSettings {
            backup_download_strategy:
                matrix_sdk::encryption::BackupDownloadStrategy::AfterDecryptionFailure,
            ..Default::default()
        })
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
    if let Some(id) = &saved.oauth_client_id {
        client
            .oauth()
            .restore_session(
                matrix_sdk::authentication::oauth::OAuthSession {
                    client_id: matrix_sdk::authentication::oauth::ClientId::new(id.clone()),
                    user: matrix_sdk::authentication::oauth::UserSession {
                        meta: saved.session.meta.clone(),
                        tokens: saved.session.tokens.clone(),
                    },
                },
                RoomLoadSettings::default(),
            )
            .await
            .map_err(|_| "session-invalid")?;
    } else {
        client
            .matrix_auth()
            .restore_session(saved.session.clone(), RoomLoadSettings::default())
            .await
            .map_err(|_| "session-invalid")?;
    }
    if client.encryption().ed25519_key().await.as_deref() != Some(saved.fingerprint.as_str()) {
        return Err("session-key-mismatch");
    }
    let token_store = crate::token_store::TokenStore::attach(&client, profile.clone(), saved)?;
    start(epoch, client, updates, true, Some(token_store)).map(Some)
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
        false,
    )
    .await
}
pub(crate) async fn browser_login(
    epoch: u64,
    server: &str,
    persistence: &mut Persistence,
    updates: mpsc::Sender<(u64, SyncUpdate)>,
    events: mpsc::Sender<crate::Event>,
    oauth: bool,
) -> Result<Session, &'static str> {
    tokio::time::timeout(
        Duration::from_secs(300),
        connect_with(
            epoch,
            server,
            None,
            persistence,
            updates,
            Some(events),
            oauth,
        ),
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
    oauth: bool,
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
    } else if oauth {
        crate::oauth::login(&client, epoch, events.ok_or("login-failed")?).await?;
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
            session: crate::oauth::session(&client)?,
            oauth_client_id: client.oauth().client_id().map(|id| id.as_str().to_owned()),
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
    start(epoch, client, updates, saved, token_store)
}

fn start(
    epoch: u64,
    client: Client,
    updates: mpsc::Sender<(u64, SyncUpdate)>,
    saved: bool,
    token_store: Option<Arc<crate::token_store::TokenStore>>,
) -> Result<Session, &'static str> {
    let local = crate::local_data::LocalData::open(token_store.as_deref())?;
    let (history_ready, initial_sync) = tokio::sync::oneshot::channel();
    let archive_sync = token_store.as_ref().map(|_| {
        crate::archive_sync::start(
            client.clone(),
            local.index.clone(),
            epoch,
            updates.clone(),
            initial_sync,
        )
    });
    let security = crate::security::Security::new(&client);
    let room_keys = watch_room_keys(client.clone(), epoch, updates.clone());
    let queue_updates = watch_outbox(&client, epoch, updates.clone());
    let syncing = client.clone();
    let sync = tokio::spawn(async move {
        let mut history_ready = Some(history_ready);
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
                    SyncUpdate::Expired(
                        matches!(error.client_api_error_kind(), Some(matrix_sdk::ruma::api::error::ErrorKind::UnknownToken(data)) if data.soft_logout),
                    )
                }
                Err(_) => SyncUpdate::Retrying,
            };
            let expired = matches!(update, SyncUpdate::Expired(_));
            let ready = matches!(update, SyncUpdate::Ready(_));
            if updates.send((epoch, update)).await.is_err() || expired {
                break;
            }
            if ready && let Some(start) = history_ready.take() {
                let _ = start.send(());
            }
            tokio::time::sleep(Duration::from_millis(if ready { 250 } else { 3000 })).await;
        }
    });
    Ok(Session {
        archive_sync,
        local,
        transfers: Default::default(),
        avatars: Default::default(),
        epoch,
        client,
        security,
        selected: None,
        history: crate::history::History::default(),
        history_cache: Default::default(),
        tools: crate::room_tools::RoomTools::default(),
        conversations: crate::conversations::Conversations::default(),
        saved,
        token_store,
        revoked: false,
        soft_logout: false,
        blocked: Default::default(),
        notified: Default::default(),
        initial_sync_done: false,
        reconnecting: false,
        queue_updates,
        room_keys,
        sync,
    })
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
                Ok(matrix_sdk::send_queue::SendQueueUpdate {
                    room_id,
                    update: matrix_sdk::send_queue::RoomSendQueueUpdate::NewLocalEvent(echo),
                }) => {
                    if let Some(item) =
                        crate::outbox::local_echo(&queue_client, room_id.as_str(), &echo)
                        && queue_tx
                            .send((epoch, SyncUpdate::LocalEcho(Box::new(item))))
                            .await
                            .is_err()
                    {
                        break;
                    }
                }
                Ok(matrix_sdk::send_queue::SendQueueUpdate {
                    room_id,
                    update:
                        matrix_sdk::send_queue::RoomSendQueueUpdate::SentEvent {
                            transaction_id,
                            event_id,
                        },
                }) => {
                    if queue_tx
                        .send((
                            epoch,
                            SyncUpdate::Delivery {
                                room_id: room_id.to_string(),
                                transaction: transaction_id.to_string(),
                                event_id: Some(event_id.to_string()),
                            },
                        ))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(matrix_sdk::send_queue::SendQueueUpdate {
                    room_id,
                    update:
                        matrix_sdk::send_queue::RoomSendQueueUpdate::CancelledLocalEvent {
                            transaction_id,
                        },
                }) => {
                    if queue_tx
                        .send((
                            epoch,
                            SyncUpdate::Delivery {
                                room_id: room_id.to_string(),
                                transaction: transaction_id.to_string(),
                                event_id: None,
                            },
                        ))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(matrix_sdk::send_queue::SendQueueUpdate {
                    room_id,
                    update:
                        matrix_sdk::send_queue::RoomSendQueueUpdate::MediaUpload {
                            related_to,
                            progress,
                            ..
                        },
                }) => {
                    let _ = queue_tx.try_send((
                        epoch,
                        SyncUpdate::Progress {
                            room_id: room_id.to_string(),
                            id: related_to,
                            current: progress.current as u64,
                            total: progress.total as u64,
                        },
                    ));
                }
                _ => {
                    if queue_tx.send((epoch, SyncUpdate::Outbox)).await.is_err() {
                        break;
                    }
                }
            }
        }
    })
}

/// Login without activating a second SDK device; validate identity before replacing saved tokens.
pub(crate) async fn reauthenticate(
    active: &Session,
    password: crate::security::Secret,
) -> Result<(), &'static str> {
    use matrix_sdk::ruma::api::client::session::login::v3::{LoginInfo, Password, Request};
    if !active.soft_logout || !active.revoked {
        return Err("reauth-unavailable");
    }
    let current = crate::oauth::session(&active.client)?;
    let mut request = Request::new(LoginInfo::Password(Password::new(
        matrix_sdk::ruma::api::client::uiaa::UserIdentifier::from(current.meta.user_id.clone()),
        password.expose().to_owned(),
    )));
    request.device_id = Some(current.meta.device_id.clone());
    request.refresh_token = true;
    let response = active
        .client
        .send(request)
        .await
        .map_err(|_| "reauth-failed")?;
    let renewed: matrix_sdk::authentication::matrix::MatrixSession = (&response).into();
    if renewed.meta != current.meta {
        return Err("session-key-mismatch");
    }
    active
        .token_store
        .as_ref()
        .ok_or("reauth-unavailable")?
        .replace_tokens(renewed)?;
    Ok(())
}

fn watch_room_keys(
    client: Client,
    epoch: u64,
    updates: mpsc::Sender<(u64, SyncUpdate)>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        use futures_util::StreamExt;
        let Some(mut stream) = client.encryption().room_keys_received_stream().await else {
            return;
        };
        let backups = client.encryption().backups();
        let mut backup_state = backups.state_stream();
        loop {
            let rooms = tokio::select! {
                keys = stream.next() => {
                    let Some(keys) = keys else { break; };
                    // Broadcast lag requires a retry of the selected room too.
                    keys.ok().map(|keys| keys.into_iter().map(|key| key.room_id.to_string()).collect())
                }
                state = backup_state.next() => {
                    let Some(state) = state else { break; };
                    // Verification may unlock a backup before any room keys arrive.
                    // Retry the loaded ciphertext to schedule its missing-key downloads.
                    if state.unwrap_or_else(|_| backups.state()) != matrix_sdk::encryption::backups::BackupState::Enabled {
                        continue;
                    }
                    None
                }
            };
            if updates
                .send((epoch, SyncUpdate::RoomKeys(rooms)))
                .await
                .is_err()
            {
                break;
            }
        }
    })
}
