use crate::{Command, Event, EventKind, Message, RoomSummary};
use crate::{
    connection::{self, Persistence, Session, SyncUpdate},
    storage::Profile,
};
use matrix_sdk::{
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
use tokio::sync::mpsc;

async fn emit(events: &mpsc::Sender<Event>, epoch: u64, kind: EventKind) {
    let _ = events.send(Event { epoch, kind }).await;
}

/// Explicitly memory-only service, used for temporary sessions and protocol tests.
pub async fn run(commands: mpsc::Receiver<Command>, events: mpsc::Sender<Event>) {
    run_inner(commands, events, Persistence::Temporary, false).await;
}
pub async fn run_persistent(commands: mpsc::Receiver<Command>, events: mpsc::Sender<Event>) {
    run_inner(commands, events, Persistence::Unavailable, true).await;
}
pub async fn run_with_profile(
    commands: mpsc::Receiver<Command>,
    events: mpsc::Sender<Event>,
    profile: Profile,
) {
    run_inner(commands, events, Persistence::Ready(profile), true).await;
}

async fn announce(events: &mpsc::Sender<Event>, active: &Session) {
    let user = active
        .client
        .user_id()
        .map(ToString::to_string)
        .unwrap_or_default();
    emit(events, active.epoch, EventKind::Connected(user)).await;
    emit(events, active.epoch, EventKind::SessionSaved(active.saved)).await;
}

async fn restore(
    epoch: u64,
    persistence: &mut Persistence,
    updates: mpsc::Sender<(u64, SyncUpdate)>,
    events: &mpsc::Sender<Event>,
) -> Option<Session> {
    match connection::restore(epoch, persistence, updates).await {
        Ok(Some(active)) => {
            announce(events, &active).await;
            Some(active)
        }
        Ok(None) => {
            emit(events, epoch, EventKind::SignedOut).await;
            None
        }
        Err(key) => {
            emit(events, epoch, EventKind::RestoreBlocked(key)).await;
            None
        }
    }
}

async fn run_inner(
    mut commands: mpsc::Receiver<Command>,
    events: mpsc::Sender<Event>,
    mut persistence: Persistence,
    restore_on_start: bool,
) {
    let (updates, mut update_rx) = mpsc::channel::<(u64, SyncUpdate)>(1);
    let mut session = if restore_on_start {
        restore(0, &mut persistence, updates.clone(), &events).await
    } else {
        None
    };
    loop {
        tokio::select! {
            command = commands.recv() => {
                let Some(command) = command else { break };
                match command {
                    Command::Restore { epoch } if session.is_none() => {
                        session = restore(epoch, &mut persistence, updates.clone(), &events).await;
                    }
                    Command::Temporary { epoch } if session.is_none() => {
                        persistence = Persistence::Temporary;
                        emit(&events, epoch, EventKind::SignedOut).await;
                    }
                    Command::Login { epoch, server, username, password } if session.is_none() => {
                        match connection::connect(epoch, &server, &username, &password, &mut persistence, updates.clone()).await {
                            Ok(connected) => {
                                announce(&events, &connected).await;
                                session = Some(connected);
                            }
                            Err(key) => emit(&events, epoch, EventKind::Error(key)).await,
                        }
                    }
                    command => {
                        if let Some(active) = &mut session
                            && handle(command, active, &mut persistence, &events).await { session = None; }
                    }
                }
            }
            update = update_rx.recv() => {
                if let (Some((epoch, update)), Some(active)) = (update, &mut session) {
                    if epoch != active.epoch || active.revoked { continue; }
                    match update {
                        SyncUpdate::Expired => {
                            active.revoked = true;
                            emit(&events, epoch, EventKind::Error("session-expired")).await;
                        }
                        SyncUpdate::Ready => {
                            emit(&events, epoch, EventKind::Status("ready")).await;
                            snapshot(active, &events).await;
                        }
                        SyncUpdate::Retrying => emit(&events, epoch, EventKind::Status("reconnecting")).await,
                    }
                }
            }
            _ = events.closed() => break,
        }
    }
}

async fn handle(
    command: Command,
    active: &mut Session,
    persistence: &mut Persistence,
    events: &mpsc::Sender<Event>,
) -> bool {
    if active.revoked && !matches!(command, Command::Logout) {
        return false;
    }
    let result = match command {
        Command::Select(room_id) => {
            active.selected = Some(room_id);
            history(active, events).await
        }
        Command::Join(address) => {
            if let Err(key) = join_room(active, &address, events).await {
                emit(events, active.epoch, EventKind::JoinFailed(key)).await;
            }
            Ok(())
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
        Command::Logout => match logout(active, persistence).await {
            Ok(()) => {
                emit(events, active.epoch, EventKind::SignedOut).await;
                return true;
            }
            Err(key) => Err(key),
        },
        Command::Login { .. } | Command::Restore { .. } | Command::Temporary { .. } => Ok(()),
    };
    if let Err(key) = result {
        emit(events, active.epoch, EventKind::Error(key)).await;
    }
    false
}

async fn join_room(
    active: &mut Session,
    address: &str,
    events: &mpsc::Sender<Event>,
) -> Result<(), &'static str> {
    let address =
        matrix_sdk::ruma::RoomOrAliasId::parse(address.trim()).map_err(|_| "join-invalid")?;
    let room = active
        .client
        .join_room_by_id_or_alias(&address, &[])
        .await
        .map_err(|error| match error.client_api_error_kind() {
            Some(matrix_sdk::ruma::api::error::ErrorKind::Forbidden) => "join-forbidden",
            Some(matrix_sdk::ruma::api::error::ErrorKind::NotFound) => "join-not-found",
            _ => "join-failed",
        })?;
    let id = room.room_id().to_string();
    let name = room
        .display_name()
        .await
        .map(|name| name.to_string())
        .unwrap_or_else(|_| id.clone());
    active.selected = Some(id.clone());
    emit(
        events,
        active.epoch,
        EventKind::Joined(RoomSummary { id, name }),
    )
    .await;
    snapshot(active, events).await;
    Ok(())
}

async fn logout(active: &mut Session, persistence: &Persistence) -> Result<(), &'static str> {
    if !active.revoked {
        match active.client.matrix_auth().logout().await {
            Ok(_) => {}
            Err(error)
                if matches!(
                    error.client_api_error_kind(),
                    Some(matrix_sdk::ruma::api::error::ErrorKind::UnknownToken { .. })
                ) => {}
            Err(_) => return Err("logout-failed"),
        }
        active.revoked = true;
        active.stop_sync();
    }
    if let Persistence::Ready(profile) = persistence {
        profile.save(None).map_err(|_| "logout-cleanup-failed")?;
    }
    Ok(())
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
