use crate::{MessageAction, MessageRequest};
use matrix_sdk::{
    Client, Room, RoomState,
    room::{
        edit::{EditError, EditedContent},
        reply::{EnforceThread, Reply, ReplyError},
    },
    ruma::{
        EventId, RoomId,
        events::{
            AnySyncMessageLikeEvent, AnySyncTimelineEvent, SyncMessageLikeEvent,
            reaction::ReactionEventContent,
            relation::Annotation,
            room::message::{AddMentions, MessageType, RoomMessageEventContentWithoutRelation},
        },
    },
};

pub(crate) fn failure(error: matrix_sdk::Error) -> &'static str {
    use matrix_sdk::ruma::api::error::ErrorKind;
    match error.client_api_error_kind() {
        Some(ErrorKind::UnknownToken { .. }) => "session-expired",
        Some(ErrorKind::Forbidden) => "message-forbidden",
        _ => "message-action-failed",
    }
}
pub(crate) async fn target(
    room: &Room,
    id: &EventId,
) -> Result<AnySyncTimelineEvent, &'static str> {
    let event = room.event(id, None).await.map_err(failure)?;
    let room_id: Option<String> = event
        .raw()
        .get_field("room_id")
        .map_err(|_| "message-unavailable")?;
    if room_id
        .as_deref()
        .is_some_and(|id| id != room.room_id().as_str())
    {
        return Err("message-unavailable");
    }
    let event = event
        .raw()
        .deserialize()
        .map_err(|_| "message-unavailable")?;
    Ok(event)
}
pub(crate) async fn perform(client: &Client, request: &MessageRequest) -> Result<(), &'static str> {
    perform_inner(client, request, false).await
}
pub(crate) async fn enqueue(client: &Client, request: &MessageRequest) -> Result<(), &'static str> {
    perform_inner(client, request, true).await
}
async fn perform_inner(
    client: &Client,
    request: &MessageRequest,
    queued: bool,
) -> Result<(), &'static str> {
    let room_id = RoomId::parse(&request.room_id).map_err(|_| "message-unavailable")?;
    let room = client
        .get_room(&room_id)
        .filter(|r| r.state() == RoomState::Joined)
        .ok_or("message-unavailable")?;
    let id = EventId::parse(&request.event_id).map_err(|_| "message-unavailable")?;
    let event = target(&room, &id).await?;
    if request.action == MessageAction::Delete
        && let AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::RoomMessage(
            SyncMessageLikeEvent::Redacted(message),
        )) = &event
        && message.event_id == id
        && Some(message.sender.as_ref()) == client.user_id()
    {
        // A timeout can hide a successful redaction. Reuse the transaction for its acknowledgement.
        redact(&room, id, request.transaction.clone(), queued).await?;
        return Ok(());
    }
    let AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::RoomMessage(
        SyncMessageLikeEvent::Original(original),
    )) = event
    else {
        return Err("message-unavailable");
    };
    if original.event_id != id
        || matches!(
            original.content.relates_to,
            Some(matrix_sdk::ruma::events::room::message::Relation::Replacement(_))
        )
    {
        return Err("message-unavailable");
    }
    let own_text = Some(original.sender.as_ref()) == client.user_id()
        && matches!(original.content.msgtype, MessageType::Text(_));
    match &request.action {
        MessageAction::Forward(destination) => {
            if !matches!(original.content.msgtype, MessageType::Text(_)) {
                return Err("message-forward-text-only");
            }
            let id = RoomId::parse(destination.trim()).map_err(|_| "room-unavailable")?;
            let target = client
                .get_room(&id)
                .filter(|r| r.state() == RoomState::Joined)
                .ok_or("room-unavailable")?;
            let content =
                matrix_sdk::ruma::events::room::message::RoomMessageEventContent::text_plain(
                    format!("{}:\n{}", original.sender, original.content.body()),
                );
            send(&target, content.into(), request, queued).await?;
        }
        MessageAction::Reply(body) | MessageAction::Thread(body) => {
            validate_body(body)?;
            let content = room
                .make_reply_event(
                    RoomMessageEventContentWithoutRelation::text_plain(body)
                        .add_mentions(crate::mentions::metadata(body)),
                    Reply {
                        event_id: id,
                        enforce_thread: if matches!(request.action, MessageAction::Thread(_)) {
                            EnforceThread::Threaded(
                                matrix_sdk::ruma::events::room::message::ReplyWithinThread::No,
                            )
                        } else {
                            EnforceThread::Unthreaded
                        },
                        add_mentions: AddMentions::No,
                    },
                )
                .await
                .map_err(|e| match e {
                    ReplyError::Fetch(e) => failure(*e),
                    _ => "message-unavailable",
                })?;
            send(&room, content.into(), request, queued).await?;
        }
        MessageAction::Edit(body) => {
            if !own_text {
                return Err("message-forbidden");
            }
            validate_body(body)?;
            let content = room
                .make_edit_event(
                    &id,
                    EditedContent::RoomMessage(
                        RoomMessageEventContentWithoutRelation::text_plain(body)
                            .add_mentions(crate::mentions::metadata(body)),
                    ),
                )
                .await
                .map_err(|e| match e {
                    EditError::Fetch(e) => failure(*e),
                    _ => "message-unavailable",
                })?;
            send(&room, content, request, queued).await?;
        }
        MessageAction::Delete => {
            if !own_text {
                return Err("message-forbidden");
            }
            redact(&room, id, request.transaction.clone(), queued).await?;
        }
        MessageAction::React(key) => {
            validate_key(key)?;
            send(
                &room,
                ReactionEventContent::new(Annotation::new(id, key.clone())).into(),
                request,
                queued,
            )
            .await?;
        }
        MessageAction::RemoveReactions { key, ids } => {
            remove_reactions(&room, request, key, ids, queued).await?;
        }
    }
    Ok(())
}
fn validate_body(body: &str) -> Result<(), &'static str> {
    if body.trim().is_empty() || body.len() > 32_000 {
        Err("message-body-invalid")
    } else {
        Ok(())
    }
}
fn validate_key(key: &str) -> Result<(), &'static str> {
    if key.trim().is_empty() || key.len() > 128 || key.chars().any(char::is_control) {
        Err("reaction-invalid")
    } else {
        Ok(())
    }
}
async fn remove_reactions(
    room: &Room,
    request: &MessageRequest,
    key: &str,
    ids: &[String],
    queued: bool,
) -> Result<(), &'static str> {
    validate_key(key)?;
    if ids.is_empty() || ids.len() > 500 {
        return Err("reaction-invalid");
    }
    // Validate every target before sending any redaction. Already-redacted events are
    // accepted on retry only when their identity and original author still match.
    for (index, value) in ids.iter().enumerate() {
        if ids[..index].contains(value) {
            return Err("reaction-invalid");
        }
        let id = EventId::parse(value).map_err(|_| "reaction-invalid")?;
        let event = target(room, &id).await?;
        let AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::Reaction(reaction)) = event
        else {
            return Err("message-forbidden");
        };
        if reaction.event_id() != id || reaction.sender() != room.own_user_id() {
            return Err("message-forbidden");
        }
        if let SyncMessageLikeEvent::Original(reaction) = reaction
            && (reaction.content.relates_to.event_id.as_str() != request.event_id
                || reaction.content.relates_to.key != key)
        {
            return Err("message-forbidden");
        }
    }
    for (index, value) in ids.iter().enumerate() {
        let id = EventId::parse(value).map_err(|_| "reaction-invalid")?;
        let transaction = format!("{}-{index}", request.transaction).into();
        redact(room, id, transaction, queued).await?;
    }
    Ok(())
}

async fn send(
    room: &Room,
    content: matrix_sdk::ruma::events::AnyMessageLikeEventContent,
    request: &MessageRequest,
    queued: bool,
) -> Result<(), &'static str> {
    if queued {
        room.send_queue()
            .send(content)
            .await
            .map_err(|_| "outbox-failed")?;
    } else {
        room.send(content)
            .with_transaction_id(request.transaction.clone())
            .await
            .map_err(failure)?;
    }
    Ok(())
}
async fn redact(
    room: &Room,
    id: matrix_sdk::ruma::OwnedEventId,
    transaction: crate::TransactionId,
    queued: bool,
) -> Result<(), &'static str> {
    if queued {
        room.send_queue()
            .redact(id, None)
            .await
            .map_err(|_| "outbox-failed")?;
    } else {
        room.redact(&id, None, Some(transaction))
            .await
            .map_err(|e| failure(e.into()))?;
    }
    Ok(())
}
