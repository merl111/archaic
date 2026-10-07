use crate::{ToolAction, ToolData, ToolRequest, message_actions::failure};
use matrix_sdk::{
    Client, Room, RoomState,
    ruma::{
        EventId, RoomId, api::client::receipt::create_receipt::v3::ReceiptType,
        events::receipt::ReceiptThread,
    },
};
#[derive(Default)]
pub(crate) struct RoomTools {
    pub attachments: crate::attachments::Attachments,
    search: crate::search::Search,
    pub details: crate::details::Details,
}
impl RoomTools {
    pub async fn perform(
        &mut self,
        client: &Client,
        request: &ToolRequest,
    ) -> Result<ToolData, &'static str> {
        let room = joined(client, &request.room_id)?;
        match &request.action {
            ToolAction::Sticker { .. } | ToolAction::Voice { .. } | ToolAction::Poll { .. } => {
                crate::composer_extras::send(&room, request).await?;
                Ok(ToolData::Uploaded)
            }
            ToolAction::QueueUpload { .. } | ToolAction::Preview { .. } => Err("transfer-busy"),
            ToolAction::Upload { path } => {
                self.attachments.upload(&room, request, path).await?;
                Ok(ToolData::Uploaded)
            }
            ToolAction::Download { event_id, path } => {
                crate::attachments::download(&room, event_id, path).await?;
                Ok(ToolData::Downloaded)
            }
            ToolAction::MarkRead { event_id, private }
            | ToolAction::MarkThreadRead { event_id, private } => {
                send_receipt(
                    &room,
                    event_id,
                    *private,
                    matches!(request.action, ToolAction::MarkThreadRead { .. }),
                )
                .await?;
                Ok(ToolData::Read)
            }
            ToolAction::Search { query, mode, more } => self
                .search
                .load(&room, query, *mode, *more)
                .await
                .map(ToolData::Search),
            ToolAction::Details { event_id, more } => self
                .details
                .load(&room, event_id, *more)
                .await
                .map(|details| ToolData::Details(Box::new(details))),
        }
    }
}
pub(crate) fn joined(client: &Client, id: &str) -> Result<Room, &'static str> {
    let id = RoomId::parse(id).map_err(|_| "message-unavailable")?;
    client
        .get_room(&id)
        .filter(|r| r.state() == RoomState::Joined)
        .ok_or("message-unavailable")
}

async fn send_receipt(
    room: &Room,
    event_id: &str,
    private: bool,
    threaded: bool,
) -> Result<(), &'static str> {
    let id = EventId::parse(event_id).map_err(|_| "message-unavailable")?;
    let event = crate::message_actions::target(room, &id).await?;
    if event.event_id() != id {
        return Err("message-unavailable");
    }
    let thread = if threaded {
        use matrix_sdk::ruma::events::{
            AnySyncMessageLikeEvent, AnySyncTimelineEvent, SyncMessageLikeEvent,
            room::message::Relation,
        };
        let root = match &event {
            AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::RoomMessage(
                SyncMessageLikeEvent::Original(m),
            )) => match &m.content.relates_to {
                Some(Relation::Thread(t)) => t.event_id.clone(),
                _ => id.clone(),
            },
            _ => return Err("message-unavailable"),
        };
        ReceiptThread::Thread(root)
    } else {
        ReceiptThread::Unthreaded
    };
    room.send_single_receipt(
        if private {
            ReceiptType::ReadPrivate
        } else {
            ReceiptType::Read
        },
        thread,
        id,
    )
    .await
    .map_err(failure)?;
    Ok(())
}
