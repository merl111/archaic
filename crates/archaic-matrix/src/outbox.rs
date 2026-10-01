//! The SDK owns persistence, transaction IDs and retry ordering.
use matrix_sdk::{
    Client,
    ruma::{
        RoomId,
        events::{AnyMessageLikeEventContent, room::message::RoomMessageEventContent},
    },
    send_queue::LocalEchoContent,
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub room_id: String,
    pub id: String,
    pub body: String,
    pub failed: bool,
}
pub(crate) async fn snapshot(client: &Client) -> Result<Vec<Item>, &'static str> {
    let echoes = client
        .send_queue()
        .local_echoes()
        .await
        .map_err(|_| "outbox-failed")?;
    let mut items = Vec::new();
    for (room, events) in echoes {
        for event in events {
            if let LocalEchoContent::Redaction {
                redacts,
                send_error,
                ..
            } = &event.content
            {
                items.push(Item {
                    room_id: room.to_string(),
                    id: event.transaction_id.to_string(),
                    body: format!("Redact {redacts}"),
                    failed: send_error.is_some()
                        || client
                            .get_room(&room)
                            .is_some_and(|r| !r.send_queue().is_enabled()),
                });
            }
            if let LocalEchoContent::Event {
                serialized_event,
                send_error,
                ..
            } = event.content
            {
                let body = match serialized_event.deserialize() {
                    Ok(AnyMessageLikeEventContent::RoomMessage(content)) => {
                        content.body().to_owned()
                    }
                    Ok(AnyMessageLikeEventContent::Reaction(c)) => c.relates_to.key,
                    _ => "Event".into(),
                };
                let paused = client
                    .get_room(&room)
                    .is_some_and(|r| !r.send_queue().is_enabled());
                items.push(Item {
                    room_id: room.to_string(),
                    id: event.transaction_id.to_string(),
                    body,
                    failed: send_error.is_some() || paused,
                });
            }
        }
    }
    Ok(items)
}
pub(crate) async fn enqueue(
    client: &Client,
    room_id: &str,
    body: String,
) -> Result<(), &'static str> {
    let room = RoomId::parse(room_id)
        .ok()
        .and_then(|id| client.get_room(&id))
        .ok_or("send-failed")?;
    if body.trim().is_empty() {
        return Err("send-failed");
    }
    room.send_queue()
        .send(AnyMessageLikeEventContent::RoomMessage(
            RoomMessageEventContent::text_plain(body),
        ))
        .await
        .map_err(|_| "send-failed")?;
    Ok(())
}
pub(crate) async fn action(
    client: &Client,
    room_id: &str,
    id: &str,
    retry: bool,
) -> Result<(), &'static str> {
    let echoes = client
        .send_queue()
        .local_echoes()
        .await
        .map_err(|_| "outbox-failed")?;
    let (room, events) = echoes
        .into_iter()
        .find(|(r, _)| r.as_str() == room_id)
        .ok_or("outbox-stale")?;
    for event in events {
        if event.transaction_id.as_str() != id {
            continue;
        }
        if let LocalEchoContent::Redaction { send_handle, .. } = &event.content {
            if retry {
                client
                    .state_store()
                    .update_send_queue_request_status(&room, &event.transaction_id, None)
                    .await
                    .map_err(|_| "outbox-failed")?;
                if let Some(r) = client.get_room(&room) {
                    r.send_queue().set_enabled(true);
                }
            } else {
                send_handle.abort().await.map_err(|_| "outbox-failed")?;
            }
            return Ok(());
        }
        if let LocalEchoContent::Event { send_handle, .. } = event.content {
            if retry {
                send_handle.unwedge().await.map_err(|_| "outbox-failed")?;
                if let Some(room) = client.get_room(&room) {
                    room.send_queue().set_enabled(true);
                }
            } else {
                send_handle.abort().await.map_err(|_| "outbox-failed")?;
            }
            return Ok(());
        }
    }
    Err("outbox-stale")
}
