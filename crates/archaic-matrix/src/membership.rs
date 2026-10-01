use crate::{Membership, RoomAction, RoomSummary};
use matrix_sdk::{
    Client, Room, RoomState,
    ruma::{RoomId, UserId},
};

pub(crate) async fn summary(room: &Room) -> RoomSummary {
    let id = room.room_id().to_string();
    let name = room
        .display_name()
        .await
        .map(|name| name.to_string())
        .unwrap_or_else(|_| id.clone());
    let membership = if room.state() == RoomState::Invited {
        Membership::Invited
    } else {
        Membership::Joined
    };
    let inviter = if membership == Membership::Invited {
        room.invite_details()
            .await
            .ok()
            .map(|invite| invite.inviter_id.to_string())
    } else {
        None
    };
    let encryption = room.encryption_state();
    RoomSummary {
        info: crate::RoomInfo {
            topic: room.topic().unwrap_or_default(),
            members: room.joined_members_count(),
            encrypted: (!encryption.is_unknown()).then(|| encryption.is_encrypted()),
            space: room.is_space(),
        },
        id,
        name,
        membership,
        inviter,
    }
}

pub(crate) async fn perform(
    client: &Client,
    room_id: &str,
    action: &RoomAction,
) -> Result<(), &'static str> {
    let id = RoomId::parse(room_id).map_err(|_| "room-unavailable")?;
    let room = client.get_room(&id).ok_or("room-unavailable")?;
    let expected = match action {
        RoomAction::Accept | RoomAction::Decline => RoomState::Invited,
        RoomAction::Leave | RoomAction::Invite(_) => RoomState::Joined,
    };
    if room.state() != expected {
        return Err("room-membership-changed");
    }
    let result = match action {
        RoomAction::Accept => room.join().await,
        RoomAction::Decline | RoomAction::Leave => room.leave().await,
        RoomAction::Invite(user) => {
            let user = UserId::parse(user.trim()).map_err(|_| "invite-invalid-user")?;
            room.invite_user_by_id(&user).await
        }
    };
    result.map_err(|error| match error.client_api_error_kind() {
        Some(matrix_sdk::ruma::api::error::ErrorKind::Forbidden) => "room-action-forbidden",
        Some(matrix_sdk::ruma::api::error::ErrorKind::UnknownToken { .. }) => "session-expired",
        _ => "room-action-failed",
    })
}
