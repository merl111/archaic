use crate::HistoryLoad;
use crate::{Command, Event, EventKind, RoomAction, membership};
use crate::{
    connection::{self, Persistence, Session, SyncUpdate},
    storage::Profile,
};
use matrix_sdk::ruma::RoomId;
use tokio::sync::mpsc;

pub(crate) async fn emit(events: &mpsc::Sender<Event>, epoch: u64, kind: EventKind) {
    let _ = events.send(Event { epoch, kind }).await;
}

/// Explicitly memory-only service, used for temporary sessions and protocol tests.
pub async fn run(commands: mpsc::Receiver<Command>, events: mpsc::Sender<Event>) {
    run_inner(commands, events, Persistence::Temporary, false).await;
}
pub async fn run_persistent(commands: mpsc::Receiver<Command>, events: mpsc::Sender<Event>) {
    run_persistent_named(commands, events, "default".into()).await;
}
pub async fn run_persistent_named(
    commands: mpsc::Receiver<Command>,
    events: mpsc::Sender<Event>,
    name: String,
) {
    run_inner(commands, events, Persistence::Unavailable(name), true).await;
}
pub async fn run_with_profile(
    commands: mpsc::Receiver<Command>,
    events: mpsc::Sender<Event>,
    profile: Profile,
) {
    run_inner(commands, events, Persistence::Ready(profile.into()), true).await;
}

async fn announce(events: &mpsc::Sender<Event>, active: &Session) {
    let user = active
        .client
        .user_id()
        .map(ToString::to_string)
        .unwrap_or_default();
    emit(events, active.epoch, EventKind::Connected(user)).await;
    emit(events, active.epoch, EventKind::SessionSaved(active.saved)).await;
    emit(
        events,
        active.epoch,
        EventKind::Drafts(active.local.drafts.clone()),
    )
    .await;
}

async fn restore(
    epoch: u64,
    persistence: &mut Persistence,
    updates: mpsc::Sender<(u64, SyncUpdate)>,
    events: &mpsc::Sender<Event>,
) -> Option<Session> {
    match connection::restore(epoch, persistence, updates).await {
        Ok(Some(mut active)) => {
            announce(events, &active).await;
            room_snapshot(&mut active, events).await;
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
                    Command::Barrier(reply) => { let _ = reply.send(()); },
                    Command::Shutdown {epoch,drafts,result}=>{
                        let saved=match &mut session {Some(active) if active.epoch==epoch=>active.local.save_drafts(drafts),_=>Ok(())};
                        let _=result.send(saved);break;
                    },
                    Command::SwitchProfile { epoch, name } => {
                        if let Err(key)=crate::storage::validate_profile_name(&name) {emit(&events,epoch,EventKind::RestoreBlocked(key)).await;continue;}
                        if let Some(active)=&session {active.client.send_queue().set_enabled(false).await;}
                        drop(session.take());
                        persistence=Persistence::Unavailable(name);
                        session=restore(epoch,&mut persistence,updates.clone(),&events).await;
                    }
                    Command::Reauthorize { oauth } => {
                        if let Some(active) = &session {
                            let epoch = active.epoch;
                            if !active.revoked || !active.soft_logout {
                                emit(&events,epoch,EventKind::Error("reauth-unavailable")).await;
                                continue;
                            }
                            let mut shutdown = None;
                            let result = {
                                let pending = tokio::time::timeout(std::time::Duration::from_secs(300), crate::reauthorize::browser(active,oauth,events.clone()));
                                tokio::pin!(pending);
                                loop {
                                    tokio::select! {
                                        result = &mut pending => break result.unwrap_or(Err("sso-timeout")),
                                        command = commands.recv() => match command {
                                            Some(Command::Barrier(reply)) => { let _=reply.send(()); },
                                            Some(command @ Command::Shutdown {..}) => { shutdown=Some(command); break Err("sso-cancelled"); },
                                            _ => break Err("sso-cancelled"),
                                        }
                                    }
                                }
                            };
                            if let Some(Command::Shutdown {epoch,drafts,result}) = shutdown {
                                let saved=match &mut session {Some(active) if active.epoch==epoch=>active.local.save_drafts(drafts),_=>Ok(())};
                                let _=result.send(saved);
                                break;
                            }
                            // Discard the client even after cancellation/failure: an exchange may
                            // have replaced its memory tokens before identity validation failed.
                            drop(session.take());
                            session=restore(epoch,&mut persistence,updates.clone(),&events).await;
                            if let Err(key)=result {emit(&events,epoch,EventKind::Error(key)).await;}
                        }
                    }
                    Command::Reauthenticate { password } => {
                        if let Some(active) = &session {
                            let epoch = active.epoch;
                            match crate::connection::reauthenticate(active, password).await {
                                Ok(()) => { drop(session.take()); session = restore(epoch, &mut persistence, updates.clone(), &events).await; },
                                Err(key) => emit(&events, epoch, EventKind::Error(key)).await,
                            }
                        }
                    }
                    Command::Restore { epoch } if session.is_none() => {
                        session = restore(epoch, &mut persistence, updates.clone(), &events).await;
                    }
                    Command::Temporary { epoch } if session.is_none() => {
                        persistence = Persistence::Temporary;
                        emit(&events, epoch, EventKind::SignedOut).await;
                    }
                    command @ (Command::BrowserLogin{..} | Command::OAuthLogin{..}) if session.is_none()=>{
                        let oauth = matches!(command, Command::OAuthLogin{..});
                        let (Command::BrowserLogin{epoch,server} | Command::OAuthLogin{epoch,server}) = command else { unreachable!() };
                        let result=tokio::select! {
                            result=connection::browser_login(epoch,&server,&mut persistence,updates.clone(),events.clone(),oauth)=>result,
                            command=commands.recv()=>{
                                match command {
                                    Some(Command::Shutdown{result,..}) => { let _=result.send(Ok(())); },
                                    Some(Command::Barrier(reply)) => { let _=reply.send(()); },
                                    _ => {}
                                }
                                Err("sso-cancelled")
                            },
                            _=events.closed()=>break,
                        };
                        match result {Ok(connected)=>{announce(&events,&connected).await;session=Some(connected);},Err(key)=>emit(&events,epoch,EventKind::Error(key)).await}
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
                    sync_update(active,update,&events).await;
                }
            }
            _ = events.closed() => break,
        }
    }
}

async fn sync_update(active: &mut Session, update: SyncUpdate, events: &mpsc::Sender<Event>) {
    let epoch = active.epoch;
    match update {
        SyncUpdate::Expired(soft) => {
            active.soft_logout = soft && active.saved;
            active.revoked = true;
            active.client.send_queue().set_enabled(false).await;
            emit(events, epoch, EventKind::Error("session-expired")).await;
            if active.soft_logout {
                emit(events, epoch, EventKind::ReauthenticationRequired).await;
            }
        }
        SyncUpdate::Progress {
            room_id,
            id,
            current,
            total,
        } => {
            emit(
                events,
                epoch,
                EventKind::TransferProgress {
                    room_id,
                    id,
                    current,
                    total,
                },
            )
            .await
        }
        SyncUpdate::RoomKeys(rooms) => {
            if active
                .selected
                .as_ref()
                .is_some_and(|room| rooms.as_ref().is_none_or(|rooms| rooms.contains(room)))
            {
                retry_room_keys(active, events).await;
            }
        }
        SyncUpdate::HistorySync(status) => {
            emit(events, active.epoch, EventKind::HistorySync(status)).await
        }
        SyncUpdate::Archived(room_id) => {
            if active.selected.as_ref() == Some(&room_id) && active.history.is_cached() {
                load_history(active, events, room_id, HistoryLoad::Recent).await;
            }
        }
        SyncUpdate::LocalEcho(item) => {
            emit(events, active.epoch, EventKind::Outbox(vec![*item])).await
        }
        SyncUpdate::Delivery {
            room_id,
            transaction,
            event_id,
        } => {
            emit(
                events,
                active.epoch,
                EventKind::Delivery {
                    room_id,
                    transaction,
                    event_id,
                },
            )
            .await;
            outbox_snapshot(active, events).await;
        }
        SyncUpdate::Outbox => outbox_snapshot(active, events).await,
        SyncUpdate::Retrying => {
            active.reconnecting = true;
            emit(events, epoch, EventKind::Status("reconnecting")).await;
        }
        SyncUpdate::Ready(response) => sync_ready(active, &response, events).await,
    }
}
async fn sync_ready(
    active: &mut Session,
    response: &matrix_sdk::sync::SyncResponse,
    events: &mpsc::Sender<Event>,
) {
    if let Some(store) = &active.token_store
        && store.failed()
    {
        let saved = store.save(&active.client).is_ok();
        active.saved = saved;
        emit(events, active.epoch, EventKind::SessionSaved(saved)).await;
    }
    if active.reconnecting {
        active.client.send_queue().set_enabled(true).await;
        active.reconnecting = false;
    }
    if let Err(key) = active.local.update_recent(response) {
        emit(events, active.epoch, EventKind::Error(key)).await;
    }
    emit(events, active.epoch, EventKind::Status("ready")).await;
    room_snapshot(active, events).await;
    crate::sync_activity::update(active, response, events).await;
    if active.history.is_cached() {
        history(active, events).await;
    }
    sync_history(active, response, events).await;
    if active.security.watching {
        emit(
            events,
            active.epoch,
            EventKind::SecuritySnapshot(active.security.snapshot(&active.client).await),
        )
        .await;
    }
}

async fn sync_history(
    active: &mut Session,
    response: &matrix_sdk::sync::SyncResponse,
    events: &mpsc::Sender<Event>,
) {
    let Some(room_id) = active.selected.clone() else {
        return;
    };
    let Ok(id) = RoomId::parse(&room_id) else {
        return;
    };
    let (Some(update), Some(room)) = (response.rooms.joined.get(&id), active.client.get_room(&id))
    else {
        return;
    };
    match active.tools.details.sync(&room, update).await {
        Ok(Some(details)) => {
            active
                .avatars
                .users(&room, &details.thread, events, active.epoch);
            active.avatars.users(
                &room,
                std::slice::from_ref(&details.message),
                events,
                active.epoch,
            );
            emit(
                events,
                active.epoch,
                EventKind::LiveDetails {
                    room_id: room_id.clone(),
                    details: Box::new(details),
                },
            )
            .await
        }
        Err(key) => {
            emit(
                events,
                active.epoch,
                EventKind::LiveDetailsFailed {
                    room_id: room_id.clone(),
                    key,
                },
            )
            .await
        }
        Ok(None) => {}
    }
    let changed = active.history.sync(&room, &update.timeline);
    let receipts_changed = update
        .ephemeral
        .iter()
        .any(|e| e.get_field::<String>("type").ok().flatten().as_deref() == Some("m.receipt"));
    let snapshot = changed.or_else(|| receipts_changed.then(|| active.history.snapshot()));
    if let Some((mut messages, info)) = snapshot {
        crate::receipts::populate(&room, &mut messages).await;
        active.avatars.users(&room, &messages, events, active.epoch);
        emit(
            events,
            active.epoch,
            EventKind::History {
                room_id,
                messages,
                info,
                kind: HistoryLoad::Recent,
            },
        )
        .await;
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
        Command::Organization { id, action } => {
            let result = crate::organization::perform(&active.client, &action).await;
            if result.is_ok()
                && let crate::organization::Action::Ignore { user, enabled } = &action
            {
                if *enabled {
                    active.blocked.insert(user.trim().to_owned());
                } else {
                    active.blocked.remove(user.trim());
                }
                emit(
                    events,
                    active.epoch,
                    EventKind::Blocked(active.blocked.clone()),
                )
                .await;
            }

            emit(events, active.epoch, EventKind::Organization { id, result }).await;
            room_snapshot(active, events).await;
            Ok(())
        }
        Command::SaveDraft { room_id, body } => {
            match active.local.save_draft(room_id.clone(), body.clone()) {
                Ok(()) => {
                    emit(
                        events,
                        active.epoch,
                        EventKind::DraftSaved { room_id, body },
                    )
                    .await;
                    Ok(())
                }
                Err(key) => Err(key),
            }
        }
        Command::Context { room_id, event_id } => {
            context(active, room_id, event_id, events).await;
            Ok(())
        }
        Command::Typing {
            room_id,
            active: typing,
        } => {
            if let Some(room) = RoomId::parse(room_id)
                .ok()
                .and_then(|id| active.client.get_room(&id))
                && room.state() == matrix_sdk::RoomState::Joined
            {
                let _ = room.typing_notice(typing).await;
            }
            Ok(())
        }
        Command::Enqueue {
            room_id,
            body,
            transaction,
        } => match crate::outbox::enqueue(&active.client, &room_id, body.clone()).await {
            Ok(()) => {
                if active.local.drafts.get(&room_id) == Some(&body)
                    && let Err(key) = active.local.save_draft(room_id, String::new())
                {
                    emit(events, active.epoch, EventKind::Error(key)).await;
                }
                emit(events, active.epoch, EventKind::Queued { transaction }).await;
                outbox_snapshot(active, events).await;
                Ok(())
            }
            Err(key) => Err(key),
        },
        Command::EnqueueThread {
            room_id,
            root,
            latest,
            body,
            transaction,
        } => {
            let result =
                crate::outbox::enqueue_thread(&active.client, &room_id, &root, &latest, body).await;
            emit(
                events,
                active.epoch,
                EventKind::ThreadQueued {
                    transaction,
                    result,
                },
            )
            .await;
            Ok(())
        }
        Command::OutboxAction { room_id, id, retry } => {
            let result = crate::outbox::action(&active.client, &room_id, &id, retry).await;
            outbox_snapshot(active, events).await;
            result
        }
        Command::MemberProfile { room, user } => {
            match crate::member_profile::load(&active.client, room, user).await {
                Ok(profile) => {
                    emit(events, active.epoch, EventKind::MemberProfile(profile)).await;
                    Ok(())
                }
                Err(key) => Err(key),
            }
        }
        Command::Select(room_id) => {
            if active.selected.as_ref() != Some(&room_id) {
                active.tools = Default::default();
            }
            active.selected = Some(room_id);
            history(active, events).await;
            Ok(())
        }
        Command::JoinVia {
            address,
            via,
            event,
        } => {
            if let Err(key) = join_room(active, &address, &via, event, events).await {
                emit(events, active.epoch, EventKind::JoinFailed(key)).await;
            }
            Ok(())
        }
        Command::Join(address) => {
            if let Err(key) = join_room(active, &address, &[], None, events).await {
                emit(events, active.epoch, EventKind::JoinFailed(key)).await;
            }
            Ok(())
        }
        Command::NewConversation(input) => {
            open_conversation(active, input, events).await;
            Ok(())
        }
        Command::RoomAction { room_id, action } => {
            room_action(active, room_id, action, events).await;
            Ok(())
        }
        Command::LoadHistory { room_id, kind } => {
            load_history(active, events, room_id, kind).await;
            Ok(())
        }
        Command::Security { id, action } => {
            security_action(active, id, action, events).await;
            Ok(())
        }
        Command::DiscardUpload => {
            active.tools.attachments = Default::default();
            Ok(())
        }
        Command::CancelTransfer { id } => {
            active.transfers.cancel(&id, active.epoch, events).await;
            Ok(())
        }
        Command::RoomTool(request) => {
            if matches!(
                request.action,
                crate::ToolAction::Download { .. }
                    | crate::ToolAction::QueueUpload { .. }
                    | crate::ToolAction::Preview { .. }
            ) {
                active
                    .transfers
                    .start(active.client.clone(), active.epoch, request, events.clone())
                    .await;
            } else {
                room_tool(active, request, events).await;
            }
            Ok(())
        }
        command @ (Command::MessageAction(_) | Command::QueueMessageAction(_)) => {
            let queued = matches!(command, Command::QueueMessageAction(_));
            let (Command::MessageAction(request) | Command::QueueMessageAction(request)) = command
            else {
                unreachable!()
            };
            message_action(active, request, events, queued).await;
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
                Some(room)
                    if room.state() == matrix_sdk::RoomState::Joined && !body.trim().is_empty() =>
                {
                    match room
                        .send(crate::mentions::content(body))
                        .with_transaction_id(transaction.clone())
                        .await
                    {
                        Ok(_) => {
                            emit(events, active.epoch, EventKind::Sent { transaction }).await;
                            history(active, events).await;
                            Ok(())
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
        Command::BrowserLogin { .. }
        | Command::OAuthLogin { .. }
        | Command::Barrier(_)
        | Command::Shutdown { .. }
        | Command::SwitchProfile { .. }
        | Command::Reauthenticate { .. }
        | Command::Reauthorize { .. }
        | Command::CancelLogin
        | Command::Login { .. }
        | Command::Restore { .. }
        | Command::Temporary { .. } => Ok(()),
    };
    if let Err(key) = result {
        emit(events, active.epoch, EventKind::Error(key)).await;
    }
    false
}

async fn security_action(
    active: &mut Session,
    id: crate::TransactionId,
    action: crate::security::Action,
    events: &mpsc::Sender<Event>,
) {
    let reload = matches!(
        action,
        crate::security::Action::Recover(_)
            | crate::security::Action::RecoverRoom(_)
            | crate::security::Action::Import { .. }
    );
    let result = active.security.perform(&active.client, action).await;
    let expired = matches!(result, Err("session-expired"));
    let recovered = reload && result.is_ok();
    emit(
        events,
        active.epoch,
        EventKind::SecurityResult { id, result },
    )
    .await;
    emit(
        events,
        active.epoch,
        EventKind::SecuritySnapshot(active.security.snapshot(&active.client).await),
    )
    .await;
    if recovered {
        retry_room_keys(active, events).await;
    }
    if expired {
        active.revoked = true;
        active.stop_sync();
    }
}

async fn context(
    active: &mut Session,
    room_id: String,
    event_id: String,
    events: &mpsc::Sender<Event>,
) {
    if active.selected.as_ref() == Some(&room_id)
        && let Some(room) = RoomId::parse(&room_id)
            .ok()
            .and_then(|id| active.client.get_room(&id))
            .filter(|r| r.state() == matrix_sdk::RoomState::Joined)
    {
        active
            .history
            .activate(&mut active.history_cache, room.room_id().as_str());
        let result = active.history.context(&room, &event_id).await;
        let kind = HistoryLoad::Latest;
        emit(
            events,
            active.epoch,
            match result {
                Ok((mut messages, info)) => {
                    crate::receipts::populate(&room, &mut messages).await;
                    active.avatars.users(&room, &messages, events, active.epoch);
                    EventKind::History {
                        room_id,
                        messages,
                        info,
                        kind,
                    }
                }
                Err(key) => EventKind::HistoryFailed { room_id, kind, key },
            },
        )
        .await;
    }
}

async fn room_tool(
    active: &mut Session,
    request: crate::ToolRequest,
    events: &mpsc::Sender<Event>,
) {
    let result = if let crate::ToolAction::Search {
        query,
        mode: crate::SearchMode::Indexed,
        more,
    } = &request.action
    {
        match crate::room_tools::joined(&active.client, &request.room_id) {
            Ok(room) => active
                .local
                .index
                .search(&room, query, *more)
                .await
                .map(crate::ToolData::Search),
            Err(key) => Err(key),
        }
    } else {
        active.tools.perform(&active.client, &request).await
    };
    if let Ok(crate::ToolData::Details(details)) = &result
        && let Ok(room) = crate::room_tools::joined(&active.client, &request.room_id)
    {
        active
            .avatars
            .users(&room, &details.thread, events, active.epoch);
        active.avatars.users(
            &room,
            std::slice::from_ref(&details.message),
            events,
            active.epoch,
        );
    }
    let expired = matches!(result, Err("session-expired"));
    let refresh = matches!(result, Ok(crate::ToolData::Uploaded));
    emit(
        events,
        active.epoch,
        EventKind::RoomToolResult { request, result },
    )
    .await;
    if expired {
        active.revoked = true;
        active.stop_sync();
        emit(events, active.epoch, EventKind::Error("session-expired")).await;
    } else if refresh {
        history(active, events).await;
    }
}

async fn message_action(
    active: &mut Session,
    request: crate::MessageRequest,
    events: &mpsc::Sender<Event>,
    queued: bool,
) {
    let result = if queued {
        crate::message_actions::enqueue(&active.client, &request).await
    } else {
        crate::message_actions::perform(&active.client, &request).await
    };
    emit(
        events,
        active.epoch,
        EventKind::MessageActionResult { request, result },
    )
    .await;
    if result == Err("session-expired") {
        active.revoked = true;
        active.stop_sync();
        emit(events, active.epoch, EventKind::Error("session-expired")).await;
    } else if result.is_ok() {
        history(active, events).await;
    }
}

async fn room_action(
    active: &mut Session,
    room_id: String,
    action: RoomAction,
    events: &mpsc::Sender<Event>,
) {
    let result = membership::perform(&active.client, &room_id, &action).await;
    if result.is_ok()
        && matches!(action, RoomAction::Leave | RoomAction::Decline)
        && active.selected.as_ref() == Some(&room_id)
    {
        active.selected = None;
        active.history = crate::history::History::default();
        active.tools = Default::default();
    }
    emit(
        events,
        active.epoch,
        EventKind::RoomActionResult {
            room_id,
            action,
            result,
        },
    )
    .await;
    if result == Err("session-expired") {
        active.revoked = true;
        active.stop_sync();
        emit(events, active.epoch, EventKind::Error("session-expired")).await;
    } else {
        snapshot(active, events).await;
    }
}

async fn join_room(
    active: &mut Session,
    address: &str,
    via: &[String],
    event: Option<String>,
    events: &mpsc::Sender<Event>,
) -> Result<(), &'static str> {
    if via.len() > 8 {
        return Err("join-invalid");
    }
    let via = via
        .iter()
        .map(matrix_sdk::ruma::ServerName::parse)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "join-invalid")?;
    if let Some(id) = &event {
        matrix_sdk::ruma::EventId::parse(id).map_err(|_| "link-invalid")?;
    }
    let address =
        matrix_sdk::ruma::RoomOrAliasId::parse(address.trim()).map_err(|_| "join-invalid")?;
    let room = active
        .client
        .join_room_by_id_or_alias(&address, &via)
        .await
        .map_err(|error| match error.client_api_error_kind() {
            Some(matrix_sdk::ruma::api::error::ErrorKind::Forbidden) => "join-forbidden",
            Some(matrix_sdk::ruma::api::error::ErrorKind::NotFound) => "join-not-found",
            _ => "join-failed",
        })?;
    let summary = membership::summary(&room).await;
    active.selected = Some(summary.id.clone());
    emit(events, active.epoch, EventKind::Joined(summary)).await;
    if let Some(event) = event {
        context(active, room.room_id().to_string(), event, events).await;
    } else {
        snapshot(active, events).await;
    }
    Ok(())
}

async fn logout(active: &mut Session, persistence: &Persistence) -> Result<(), &'static str> {
    if !active.revoked {
        match active.client.logout().await {
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
    if let Some(store) = &active.token_store {
        store.close();
    }
    if let Persistence::Ready(profile) = persistence {
        profile.save(None).map_err(|_| "logout-cleanup-failed")?;
    }
    Ok(())
}

async fn outbox_snapshot(active: &Session, events: &mpsc::Sender<Event>) {
    match crate::outbox::snapshot(&active.client).await {
        Ok(items) => emit(events, active.epoch, EventKind::Outbox(items)).await,
        Err(key) => emit(events, active.epoch, EventKind::Error(key)).await,
    }
}
async fn snapshot(active: &mut Session, events: &mpsc::Sender<Event>) {
    room_snapshot(active, events).await;
    history(active, events).await;
}
async fn room_snapshot(active: &mut Session, events: &mpsc::Sender<Event>) {
    let mut rooms = Vec::new();
    for room in active.client.rooms() {
        if matches!(
            room.state(),
            matrix_sdk::RoomState::Joined | matrix_sdk::RoomState::Invited
        ) {
            let mut summary = membership::summary(&room).await;
            summary.info.preview = active.local.previews.get(&summary.id).cloned();
            summary.info.recent = summary
                .info
                .recent
                .max(active.local.recent.get(&summary.id).copied().unwrap_or(0));
            rooms.push(summary);
        }
    }
    rooms.sort_by(|a, b| {
        let favorite = |id: &str| {
            RoomId::parse(id)
                .ok()
                .and_then(|id| active.client.get_room(&id))
                .is_some_and(|r| r.is_favourite())
        };
        if favorite(&a.id) != favorite(&b.id) {
            return favorite(&b.id).cmp(&favorite(&a.id));
        }
        (a.membership != crate::Membership::Invited)
            .cmp(&(b.membership != crate::Membership::Invited))
            .then(
                a.name
                    .to_lowercase()
                    .cmp(&b.name.to_lowercase())
                    .then(a.id.cmp(&b.id)),
            )
    });
    let joined: std::collections::HashSet<_> = rooms
        .iter()
        .filter(|r| r.membership == crate::Membership::Joined)
        .map(|r| r.id.as_str())
        .collect();
    active.history_cache.retain(|h| h.belongs_to(&joined));
    if !rooms.iter().any(|room| {
        active.selected.as_ref() == Some(&room.id) && room.membership == crate::Membership::Joined
    }) {
        active.history = crate::history::History::default();
        active.tools = Default::default();
    }
    active.avatars.rooms(
        &active.client,
        active.selected.as_deref(),
        events,
        active.epoch,
    );
    emit(events, active.epoch, EventKind::Rooms(rooms)).await;
}

async fn history(active: &mut Session, events: &mpsc::Sender<Event>) {
    if let Some(room_id) = active.selected.clone() {
        load_history(active, events, room_id, HistoryLoad::Recent).await;
    }
}

async fn load_history(
    active: &mut Session,
    events: &mpsc::Sender<Event>,
    room_id: String,
    kind: HistoryLoad,
) {
    if active.selected.as_ref() != Some(&room_id) {
        return;
    }
    let Some(room) = RoomId::parse(&room_id)
        .ok()
        .and_then(|id| active.client.get_room(&id))
    else {
        return;
    };
    if room.state() != matrix_sdk::RoomState::Joined {
        active.history = crate::history::History::default();
        active.tools = Default::default();
        return;
    }
    active.history.activate(&mut active.history_cache, &room_id);
    let event = match active.history.load(&room, kind, &active.local.index).await {
        Ok((mut messages, info)) => {
            crate::receipts::populate(&room, &mut messages).await;
            EventKind::History {
                room_id,
                messages,
                info,
                kind,
            }
        }
        Err(key) => EventKind::HistoryFailed { room_id, kind, key },
    };
    if let EventKind::History { messages, .. } = &event {
        active.avatars.users(&room, messages, events, active.epoch);
        active.avatars.rooms(
            &active.client,
            active.selected.as_deref(),
            events,
            active.epoch,
        );
    }
    emit(events, active.epoch, event).await;
}

async fn open_conversation(
    active: &mut Session,
    input: crate::NewConversation,
    events: &mpsc::Sender<Event>,
) {
    match active.conversations.open(&active.client, input).await {
        Ok(opened) => {
            let room = membership::summary(&opened.room).await;
            active.selected = Some(room.id.clone());
            emit(
                events,
                active.epoch,
                EventKind::ConversationOpened {
                    room,
                    warning: opened.warning,
                },
            )
            .await;
            snapshot(active, events).await;
        }
        Err(key) => emit(events, active.epoch, EventKind::CreateFailed(key)).await,
    }
}

async fn retry_room_keys(active: &mut Session, events: &mpsc::Sender<Event>) {
    let Some(room_id) = active.selected.clone() else {
        return;
    };
    let Some(room) = RoomId::parse(&room_id)
        .ok()
        .and_then(|id| active.client.get_room(&id))
    else {
        return;
    };
    if let Some((mut messages, info)) = active.history.retry_decryption(&room).await {
        crate::receipts::populate(&room, &mut messages).await;
        emit(
            events,
            active.epoch,
            EventKind::History {
                room_id: room_id.clone(),
                messages,
                info,
                kind: HistoryLoad::Recent,
            },
        )
        .await;
    }
    if let Some(details) = active.tools.details.retry_decryption(&room).await {
        emit(
            events,
            active.epoch,
            EventKind::LiveDetails {
                room_id,
                details: Box::new(details),
            },
        )
        .await;
    }
}
