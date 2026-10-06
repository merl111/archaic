//! The SDK owns persistence, transaction IDs and retry ordering.
use matrix_sdk::{
    Client,
    ruma::{RoomId, events::AnyMessageLikeEventContent},
    send_queue::LocalEchoContent,
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub room_id: String,
    pub id: String,
    pub body: String,
    pub failed: bool,
    pub message: Option<crate::Message>,
    pub target: Option<String>,
    pub reaction: Option<String>,
    pub event_id: Option<String>,
}

pub(crate) fn local_echo(
    client: &Client,
    room: &str,
    event: &matrix_sdk::send_queue::LocalEcho,
) -> Option<Item> {
    let mut item = Item {
        room_id: room.into(),
        id: event.transaction_id.to_string(),
        body: String::new(),
        failed: false,
        message: None,
        target: None,
        reaction: None,
        event_id: None,
    };
    match &event.content {
        LocalEchoContent::Event {
            serialized_event,
            send_error,
            ..
        } => {
            let (raw, kind) = serialized_event.raw();
            let content: serde_json::Value = serde_json::from_str(raw.json().get()).ok()?;
            item.failed = send_error.is_some();
            item.body = content["body"].as_str().unwrap_or("Event").to_owned();
            if kind == "m.reaction" || content["m.relates_to"]["rel_type"] == "m.replace" {
                item.target = content["m.relates_to"]["event_id"]
                    .as_str()
                    .map(str::to_owned);
                item.reaction = content["m.relates_to"]["key"].as_str().map(str::to_owned);
            } else {
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let entry = crate::timeline::decode(
                    serde_json::json!({
                        "event_id": format!("~{}", item.id), "sender":client.user_id(),
                        "origin_server_ts":timestamp, "type":kind, "content":content,
                        "unsigned":{"transaction_id":item.id}
                    }),
                    true,
                );
                item.message = crate::timeline::aggregate(&[entry]).into_iter().next();
            }
        }
        LocalEchoContent::Redaction {
            redacts,
            send_error,
            ..
        } => {
            item.target = Some(redacts.to_string());
            item.failed = send_error.is_some();
        }
        LocalEchoContent::React {
            key, applies_to, ..
        } => {
            item.target = Some(format!("~{applies_to}"));
            item.reaction = Some(key.clone());
        }
    }
    item.failed |= client
        .get_room(&matrix_sdk::ruma::RoomId::parse(room).ok()?)
        .is_some_and(|r| !r.send_queue().is_enabled());
    Some(item)
}

pub(crate) async fn snapshot(client: &Client) -> Result<Vec<Item>, &'static str> {
    let echoes = client
        .send_queue()
        .local_echoes()
        .await
        .map_err(|_| "outbox-failed")?;
    Ok(echoes
        .into_iter()
        .flat_map(|(room, events)| {
            events
                .into_iter()
                .filter_map(move |event| local_echo(client, room.as_str(), &event))
        })
        .collect())
}
/// Queue a reply to a known thread without refetching its root or history.
pub(crate) async fn enqueue_thread(
    client: &Client,
    room: &str,
    root: &str,
    latest: &str,
    body: String,
) -> Result<(), &'static str> {
    use matrix_sdk::ruma::{
        EventId,
        events::{relation::Thread, room::message::Relation},
    };
    if body.trim().is_empty() || body.len() > 32_000 {
        return Err("message-body-invalid");
    }
    let root = EventId::parse(root).map_err(|_| "message-unavailable")?;
    let latest = EventId::parse(latest).map_err(|_| "message-unavailable")?;
    let room = client
        .get_room(&RoomId::parse(room).map_err(|_| "room-unavailable")?)
        .filter(|r| r.state() == matrix_sdk::RoomState::Joined)
        .ok_or("room-unavailable")?;
    let mut content = crate::mentions::content(body);
    content.relates_to = Some(Relation::Thread(Thread::plain(root, latest)));
    room.send_queue()
        .send(content.into())
        .await
        .map_err(|_| "outbox-failed")?;
    Ok(())
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
            crate::mentions::content(body),
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
