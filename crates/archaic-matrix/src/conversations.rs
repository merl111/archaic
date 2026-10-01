use crate::NewConversation;
use matrix_sdk::{
    Client, Room, RoomState,
    ruma::{
        OwnedRoomId, OwnedUserId, UserId,
        api::client::room::create_room::v3::{Request, RoomPreset},
        events::{
            InitialStateEvent, direct::DirectEventContent,
            room::encryption::RoomEncryptionEventContent,
        },
    },
};
use std::collections::HashMap;

#[derive(Default)]
pub(crate) struct Conversations {
    // Covers the interval before m.direct arrives through sync, including failed DM tagging.
    created_direct: HashMap<OwnedUserId, (OwnedRoomId, Option<&'static str>)>,
}
pub(crate) struct Opened {
    pub room: Room,
    pub warning: Option<&'static str>,
}
impl Conversations {
    pub async fn open(
        &mut self,
        client: &Client,
        input: NewConversation,
    ) -> Result<Opened, &'static str> {
        let request = request(input, client.user_id().ok_or("session-expired")?)?;
        let direct = request.is_direct.then(|| request.invite[0].clone());
        if let Some(user) = &direct {
            if let Some(room) = client
                .get_dm_rooms(user)
                .min_by(|a, b| a.room_id().cmp(b.room_id()))
            {
                return Ok(Opened {
                    room,
                    warning: None,
                });
            }
            if let Some((id, warning)) = self.created_direct.get(user)
                && let Some(room) = client
                    .get_room(id)
                    .filter(|room| room.state() == RoomState::Joined)
            {
                return Ok(Opened {
                    room,
                    warning: *warning,
                });
            }
        }
        let room = client.create_room(request).await.map_err(|error| {
            match error.client_api_error_kind() {
                Some(matrix_sdk::ruma::api::error::ErrorKind::Forbidden) => "create-forbidden",
                Some(matrix_sdk::ruma::api::error::ErrorKind::UnknownToken { .. }) => {
                    "session-expired"
                }
                _ => "create-unconfirmed",
            }
        })?;
        let mut warning = None;
        if let Some(user) = direct {
            // SDK create_room deliberately swallows m.direct update errors. Do not report
            // that a failed tag means the room was not created, or retry creating the room.
            let tagged = client
                .account()
                .fetch_account_data_static::<DirectEventContent>()
                .await
                .ok()
                .flatten()
                .and_then(|raw| raw.deserialize().ok())
                .is_some_and(|content| {
                    content
                        .get(user.as_str())
                        .is_some_and(|ids| ids.iter().any(|id| id == room.room_id()))
                });
            if !tagged {
                warning = Some("create-direct-tag-failed");
            }
            self.created_direct
                .insert(user, (room.room_id().to_owned(), warning));
        }
        Ok(Opened { room, warning })
    }
}

fn request(input: NewConversation, own_user: &UserId) -> Result<Request, &'static str> {
    let mut request = Request::new();
    request.preset = Some(RoomPreset::PrivateChat);
    request.initial_state = vec![
        InitialStateEvent::with_empty_state_key(
            RoomEncryptionEventContent::with_recommended_defaults(),
        )
        .to_raw_any(),
    ];
    match input {
        NewConversation::Direct(user) => {
            let user = invitee(&user, own_user)?;
            request.is_direct = true;
            request.invite.push(user);
        }
        NewConversation::Group {
            name,
            topic,
            invitees,
        } => {
            let name = name.trim();
            if name.is_empty() || name.len() > 255 || name.chars().any(char::is_control) {
                return Err("create-invalid-name");
            }
            if topic.len() > 4096 || topic.contains('\0') {
                return Err("create-invalid-topic");
            }
            request.name = Some(name.to_owned());
            request.topic = (!topic.trim().is_empty()).then(|| topic.trim().to_owned());
            for user in invitees
                .split([',', ';', '\n'])
                .map(str::trim)
                .filter(|user| !user.is_empty())
            {
                let user = invitee(user, own_user)?;
                if !request.invite.contains(&user) {
                    request.invite.push(user);
                }
                if request.invite.len() > 20 {
                    return Err("create-too-many-invites");
                }
            }
        }
    }
    Ok(request)
}
fn invitee(input: &str, own_user: &UserId) -> Result<OwnedUserId, &'static str> {
    let user = UserId::parse(input.trim()).map_err(|_| "create-invalid-user")?;
    if user == own_user {
        return Err("create-self-invite");
    }
    Ok(user)
}

#[cfg(test)]
mod tests {
    use super::*;
    use matrix_sdk::ruma::user_id;
    #[test]
    fn private_encrypted_group_validates_and_deduplicates_invites() {
        let request = request(
            NewConversation::Group {
                name: "  Team  ".into(),
                topic: " Plans ".into(),
                invitees: " @bob:local, @bob:local; @carol:local\n".into(),
            },
            user_id!("@alice:local"),
        )
        .unwrap();
        let json = serde_json::to_value(&request.initial_state).unwrap();
        assert_eq!(json[0]["type"], "m.room.encryption");
        assert_eq!(json[0]["content"]["algorithm"], "m.megolm.v1.aes-sha2");
        assert_eq!(request.preset, Some(RoomPreset::PrivateChat));
        assert_eq!(request.name.as_deref(), Some("Team"));
        assert_eq!(request.topic.as_deref(), Some("Plans"));
        assert_eq!(request.invite.len(), 2);
        assert!(!request.is_direct);
    }
    #[test]
    fn invalid_input_is_rejected_before_any_network_request() {
        let own = user_id!("@alice:local");
        for (input, expected) in [
            (NewConversation::Direct("bob".into()), "create-invalid-user"),
            (
                NewConversation::Direct("@alice:local".into()),
                "create-self-invite",
            ),
            (
                NewConversation::Group {
                    name: " ".into(),
                    topic: "".into(),
                    invitees: "".into(),
                },
                "create-invalid-name",
            ),
            (
                NewConversation::Group {
                    name: "Team".into(),
                    topic: "x".repeat(4097),
                    invitees: "".into(),
                },
                "create-invalid-topic",
            ),
            (
                NewConversation::Group {
                    name: "Team".into(),
                    topic: "".into(),
                    invitees: (0..21)
                        .map(|i| format!("@user{i}:local"))
                        .collect::<Vec<_>>()
                        .join(","),
                },
                "create-too-many-invites",
            ),
        ] {
            assert_eq!(request(input, own).err(), Some(expected));
        }
    }
}
