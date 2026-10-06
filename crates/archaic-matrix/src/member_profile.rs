use crate::MemberProfile;
use matrix_sdk::{
    Client,
    ruma::{
        UserId,
        events::receipt::{ReceiptThread, ReceiptType},
    },
};
pub(crate) async fn load(
    client: &Client,
    room: String,
    user: String,
) -> Result<MemberProfile, &'static str> {
    let joined = crate::room_tools::joined(client, &room)?;
    let id = UserId::parse(&user).map_err(|_| "organization-invalid")?;
    let member = joined
        .get_member(&id)
        .await
        .map_err(|_| "organization-failed")?
        .ok_or("organization-failed")?;
    let verified = client
        .encryption()
        .get_user_identity(&id)
        .await
        .ok()
        .flatten()
        .is_some_and(|i| i.is_verified());
    let receipt = joined
        .load_user_receipt(ReceiptType::Read, &ReceiptThread::Unthreaded, &id)
        .await
        .ok()
        .flatten()
        .map(|(id, _)| id.to_string());
    let role = match member.suggested_role_for_power_level() {
        matrix_sdk::room::RoomMemberRole::Administrator => "profile-admin",
        matrix_sdk::room::RoomMemberRole::Moderator => "profile-moderator",
        _ => "profile-member",
    }
    .to_owned();
    Ok(MemberProfile {
        room,
        name: member.display_name().unwrap_or(&user).to_owned(),
        user,
        role,
        verified,
        receipt,
        loaded: true,
    })
}
