use crate::{Command, Event, EventKind, Message, RoomSummary, homeserver_url};
use matrix_sdk::{
    Client,
    config::{RequestConfig, SyncSettings},
    room::MessagesOptions,
    ruma::{
        RoomId,
        events::{
            AnySyncMessageLikeEvent, AnySyncTimelineEvent, SyncMessageLikeEvent,
            room::message::RoomMessageEventContent,
        },
        uint,
    },
};
use std::time::Duration;
use tokio::{sync::mpsc, task::JoinHandle};

struct Session {
    epoch: u64,
    client: Client,
    selected: Option<String>,
    sync: JoinHandle<()>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.sync.abort();
    }
}

async fn emit(events: &mpsc::Sender<Event>, epoch: u64, kind: EventKind) {
    let _ = events.send(Event { epoch, kind }).await;
}

pub async fn run(mut commands: mpsc::Receiver<Command>, events: mpsc::Sender<Event>) {
    let (updates, mut update_rx) = mpsc::channel::<(u64, bool)>(1);
    let mut session: Option<Session> = None;
    loop {
        tokio::select! {
            command = commands.recv() => {
                let Some(command) = command else { break };
                if let Command::Login { epoch, server, username, password } = command {
                    // The UI permits login only when disconnected. Do not orphan an active device.
                    if session.is_none() {
                        match connect(epoch, &server, &username, &password, updates.clone()).await {
                            Ok(connected) => {
                                let user = connected.client.user_id().map(ToString::to_string).unwrap_or_default();
                                emit(&events, epoch, EventKind::Connected(user)).await;
                                session = Some(connected);
                            }
                            Err(key) => emit(&events, epoch, EventKind::Error(key)).await,
                        }
                    }
                } else if let Some(active) = &mut session
                    && handle(command, active, &events).await { session = None; }
            }
            update = update_rx.recv() => {
                if let (Some((epoch, success)), Some(active)) = (update, &session) {
                    if epoch != active.epoch { continue; }
                    emit(&events, epoch, EventKind::Status(if success { "ready" } else { "reconnecting" })).await;
                    if success { snapshot(active, &events).await; }
                }
            }
            _ = events.closed() => break,
        }
    }
}

async fn connect(
    epoch: u64,
    server: &str,
    username: &str,
    password: &str,
    updates: mpsc::Sender<(u64, bool)>,
) -> Result<Session, &'static str> {
    let url = homeserver_url(server)?;
    if username.trim().is_empty() || password.is_empty() {
        return Err("missing-credentials");
    }
    let client = Client::builder()
        .homeserver_url(url.as_str())
        .request_config(
            RequestConfig::default()
                .timeout(Duration::from_secs(12))
                .retry_limit(0),
        )
        .build()
        .await
        .map_err(|_| "login-failed")?;
    client
        .matrix_auth()
        .login_username(username.trim(), password)
        .initial_device_display_name("Archaic development preview")
        .await
        .map_err(|_| "login-failed")?;
    let syncing = client.clone();
    let sync = tokio::spawn(async move {
        loop {
            let success = syncing
                .sync_once(SyncSettings::default().timeout(Duration::from_secs(15)))
                .await
                .is_ok();
            if updates.send((epoch, success)).await.is_err() {
                break;
            }
            // Also avoid a tight loop against a server that immediately returns empty syncs.
            tokio::time::sleep(Duration::from_millis(if success { 250 } else { 3000 })).await;
        }
    });
    Ok(Session {
        epoch,
        client,
        selected: None,
        sync,
    })
}

async fn handle(command: Command, active: &mut Session, events: &mpsc::Sender<Event>) -> bool {
    let result = match command {
        Command::Select(room_id) => {
            active.selected = Some(room_id);
            history(active, events).await
        }
        Command::Refresh => {
            snapshot(active, events).await;
            Ok(())
        }
        Command::Send {
            room_id,
            body,
            transaction,
        } => {
            match RoomId::parse(room_id)
                .ok()
                .and_then(|id| active.client.get_room(&id))
            {
                Some(room) if !body.trim().is_empty() => {
                    match room
                        .send(RoomMessageEventContent::text_plain(body))
                        .with_transaction_id(transaction.clone())
                        .await
                    {
                        Ok(_) => {
                            emit(events, active.epoch, EventKind::Sent { transaction }).await;
                            history(active, events).await
                        }
                        Err(_) => Err("send-failed"),
                    }
                }
                _ => Err("send-failed"),
            }
        }
        Command::Logout => {
            if active.client.matrix_auth().logout().await.is_ok() {
                emit(events, active.epoch, EventKind::SignedOut).await;
                return true;
            }
            Err("logout-failed")
        }
        Command::Login { .. } => Ok(()),
    };
    if let Err(key) = result {
        emit(events, active.epoch, EventKind::Error(key)).await;
    }
    false
}

async fn snapshot(active: &Session, events: &mpsc::Sender<Event>) {
    let mut rooms = Vec::new();
    for room in active.client.joined_rooms() {
        let id = room.room_id().to_string();
        let name = room
            .display_name()
            .await
            .map(|name| name.to_string())
            .unwrap_or_else(|_| id.clone());
        rooms.push(RoomSummary { id, name });
    }
    rooms.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then(a.id.cmp(&b.id))
    });
    emit(events, active.epoch, EventKind::Rooms(rooms)).await;
    if let Err(key) = history(active, events).await {
        emit(events, active.epoch, EventKind::Error(key)).await;
    }
}

async fn history(active: &Session, events: &mpsc::Sender<Event>) -> Result<(), &'static str> {
    let Some(room_id) = &active.selected else {
        return Ok(());
    };
    let id = RoomId::parse(room_id).map_err(|_| "load-failed")?;
    let room = active.client.get_room(&id).ok_or("load-failed")?;
    let mut options = MessagesOptions::backward();
    options.limit = uint!(50);
    let response = room.messages(options).await.map_err(|_| "load-failed")?;
    let messages = response
        .chunk
        .into_iter()
        .rev()
        .filter_map(|event| match event.raw().deserialize().ok()? {
            AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::RoomMessage(
                SyncMessageLikeEvent::Original(message),
            )) => Some(Message {
                id: message.event_id.to_string(),
                sender: message.sender.to_string(),
                body: Some(message.content.body().to_owned()),
            }),
            AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::RoomEncrypted(message)) => {
                Some(Message {
                    id: message.event_id().to_string(),
                    sender: message.sender().to_string(),
                    body: None,
                })
            }
            _ => None,
        })
        .collect();
    emit(
        events,
        active.epoch,
        EventKind::History {
            room_id: room_id.clone(),
            messages,
        },
    )
    .await;
    Ok(())
}
