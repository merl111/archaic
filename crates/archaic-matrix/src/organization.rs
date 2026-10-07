//! Room administration through typed SDK operations. The server remains authoritative for permissions.
use crate::message_actions::failure;
use matrix_sdk::{
    Client, Room, RoomMemberships,
    ruma::{
        RoomId, UserId,
        events::{
            StateEventType,
            room::{
                guest_access::RoomGuestAccessEventContent,
                history_visibility::RoomHistoryVisibilityEventContent,
                join_rules::RoomJoinRulesEventContent,
            },
        },
        uint,
    },
};
use serde_json::json;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Advanced {
        room: String,
        operation: crate::room_settings::Operation,
        value: String,
        extra: String,
        next: Option<String>,
    },
    Directory {
        query: String,
        next: Option<String>,
    },
    Hierarchy {
        room: String,
        next: Option<String>,
    },
    Inspect(String),
    Members(String),
    Spaces,
    Blocked,
    CreateSpace {
        name: String,
        topic: String,
    },
    SetName {
        room: String,
        value: String,
    },
    SetTopic {
        room: String,
        value: String,
    },
    JoinRule {
        room: String,
        value: String,
    },
    HistoryVisibility {
        room: String,
        value: String,
    },
    GuestAccess {
        room: String,
        value: String,
    },
    Power {
        room: String,
        user: String,
        level: i64,
    },
    Moderate {
        room: String,
        user: String,
        kind: String,
        reason: String,
    },
    Favourite {
        room: String,
        enabled: bool,
    },
    Ignore {
        user: String,
        enabled: bool,
    },
    SpaceChild {
        room: String,
        child: String,
        remove: bool,
    },
}
#[derive(Clone, Debug, Default)]
pub struct Page {
    pub rows: Vec<Row>,
    pub next: Option<String>,
    pub text: String,
}
#[derive(Clone, Debug)]
pub struct Row {
    pub id: String,
    pub title: String,
    pub detail: String,
}
pub(crate) async fn perform(client: &Client, action: &Action) -> Result<Page, &'static str> {
    match action {
        Action::Advanced {
            room,
            operation,
            value,
            extra,
            next,
        } => {
            crate::room_settings::perform(client, room, operation, value, extra, next.clone()).await
        }
        Action::Directory { query, next } => {
            if query.len() > 256 || next.as_ref().is_some_and(|v| v.len() > 4096) {
                return Err("organization-invalid");
            }
            let mut req=matrix_sdk::ruma::api::client::directory::get_public_rooms_filtered::v3::Request::new();
            req.limit = Some(uint!(50));
            req.since = next.clone();
            req.filter.generic_search_term =
                (!query.trim().is_empty()).then(|| query.trim().to_owned());
            let response = client
                .public_rooms_filtered(req)
                .await
                .map_err(|e| failure(e.into()))?;
            if response.chunk.len() > 50 || (next.is_some() && next == &response.next_batch) {
                return Err("history-invalid-page");
            }
            Ok(Page {
                next: response.next_batch,
                rows: response
                    .chunk
                    .into_iter()
                    .map(|r| Row {
                        id: r.room_id.to_string(),
                        title: r.name.unwrap_or_else(|| r.room_id.to_string()),
                        detail: r.topic.unwrap_or_default(),
                    })
                    .collect(),
                ..Default::default()
            })
        }
        Action::Hierarchy { room, next } => {
            let mut req = matrix_sdk::ruma::api::client::space::get_hierarchy::v1::Request::new(
                RoomId::parse(room).map_err(|_| "organization-invalid")?,
            );
            req.limit = Some(uint!(50));
            req.from = next.clone();
            req.max_depth = Some(uint!(3));
            let response = client.send(req).await.map_err(|e| failure(e.into()))?;
            if response.rooms.len() > 50 || (next.is_some() && next == &response.next_batch) {
                return Err("history-invalid-page");
            }
            Ok(Page {
                next: response.next_batch,
                rows: response
                    .rooms
                    .into_iter()
                    .map(|r| r.summary)
                    .map(|r| Row {
                        id: r.room_id.to_string(),
                        title: r.name.unwrap_or_else(|| r.room_id.to_string()),
                        detail: r.topic.unwrap_or_default(),
                    })
                    .collect(),
                ..Default::default()
            })
        }
        Action::Spaces => {
            let mut rows = Vec::new();
            for r in client.joined_rooms().into_iter().filter(|r| r.is_space()) {
                rows.push(Row {
                    id: r.room_id().to_string(),
                    title: r
                        .display_name()
                        .await
                        .map_err(|_| "organization-failed")?
                        .to_string(),
                    detail: r.topic().unwrap_or_default(),
                });
            }
            Ok(Page {
                rows,
                ..Default::default()
            })
        }
        Action::Blocked => {
            let ignored=client.account().fetch_account_data_static::<matrix_sdk::ruma::events::ignored_user_list::IgnoredUserListEventContent>().await.map_err(failure)?.map(|v|v.deserialize()).transpose().map_err(|_|"organization-invalid")?.unwrap_or_default();
            Ok(Page {
                rows: ignored
                    .ignored_users
                    .keys()
                    .map(|id| Row {
                        id: id.to_string(),
                        title: id.to_string(),
                        detail: String::new(),
                    })
                    .collect(),
                ..Default::default()
            })
        }
        Action::Members(id) => {
            let room = joined(client, id)?;
            let members = room
                .members(RoomMemberships::all())
                .await
                .map_err(failure)?;
            Ok(Page {
                rows: members
                    .into_iter()
                    .map(|m| Row {
                        id: m.user_id().to_string(),
                        title: m.display_name().unwrap_or(m.user_id().as_str()).to_owned(),
                        detail: format!("{} · {:?}", m.membership(), m.power_level()),
                    })
                    .collect(),
                ..Default::default()
            })
        }
        Action::Inspect(id) => {
            let room = joined(client, id)?;
            let power = room
                .power_levels()
                .await
                .map_err(|_| "organization-failed")?;
            Ok(Page{text:format!("{}\n{}\n{}\n\n{}",room.display_name().await.map_err(|_|"organization-failed")?,id,room.topic().unwrap_or_default(),serde_json::to_string_pretty(&matrix_sdk::ruma::events::room::power_levels::RoomPowerLevelsEventContent::try_from(power).map_err(|_|"organization-failed")?).map_err(|_|"organization-failed")?),..Default::default()})
        }
        Action::CreateSpace { name, topic } => {
            if name.trim().is_empty() || name.len() > 255 || topic.len() > 4096 {
                return Err("organization-invalid");
            }
            let mut req = matrix_sdk::ruma::api::client::room::create_room::v3::Request::new();
            req.name = Some(name.clone());
            req.topic = Some(topic.clone());
            req.preset =
                Some(matrix_sdk::ruma::api::client::room::create_room::v3::RoomPreset::PrivateChat);
            req.creation_content = Some(matrix_sdk::ruma::serde::Raw::from_json(
                serde_json::value::to_raw_value(&json!({"type":"m.space"}))
                    .map_err(|_| "organization-invalid")?,
            ));
            let room = client.create_room(req).await.map_err(failure)?;
            Ok(Page {
                text: room.room_id().to_string(),
                ..Default::default()
            })
        }
        Action::Ignore { user, enabled } => {
            let user = UserId::parse(user.trim()).map_err(|_| "organization-invalid")?;
            if *enabled {
                client.account().ignore_user(&user).await.map_err(failure)?;
            } else {
                client
                    .account()
                    .unignore_user(&user)
                    .await
                    .map_err(failure)?;
            }
            Ok(Page::default())
        }
        Action::Favourite { room, enabled } => {
            joined(client, room)?
                .set_is_favourite(*enabled, None)
                .await
                .map_err(failure)?;
            Ok(Page::default())
        }
        Action::SetName { room, value } => {
            let r = state_room(client, room, StateEventType::RoomName).await?;
            if value.len() > 255 {
                return Err("organization-invalid");
            }
            r.set_name(value.clone()).await.map_err(failure)?;
            Ok(Page::default())
        }
        Action::SetTopic { room, value } => {
            let r = state_room(client, room, StateEventType::RoomTopic).await?;
            if value.len() > 4096 {
                return Err("organization-invalid");
            }
            r.set_room_topic(value).await.map_err(failure)?;
            Ok(Page::default())
        }
        Action::JoinRule { room, value } => {
            let r = state_room(client, room, StateEventType::RoomJoinRules).await?;
            if !["public", "invite", "knock"].contains(&value.as_str()) {
                return Err("organization-invalid");
            }
            let content: RoomJoinRulesEventContent =
                serde_json::from_value(json!({"join_rule":value}))
                    .map_err(|_| "organization-invalid")?;
            r.send_state_event(content).await.map_err(failure)?;
            Ok(Page::default())
        }
        Action::HistoryVisibility { room, value } => {
            let r = state_room(client, room, StateEventType::RoomHistoryVisibility).await?;
            if !["invited", "joined", "shared", "world_readable"].contains(&value.as_str()) {
                return Err("organization-invalid");
            }
            let content: RoomHistoryVisibilityEventContent =
                serde_json::from_value(json!({"history_visibility":value}))
                    .map_err(|_| "organization-invalid")?;
            r.send_state_event(content).await.map_err(failure)?;
            Ok(Page::default())
        }
        Action::GuestAccess { room, value } => {
            let r = state_room(client, room, StateEventType::RoomGuestAccess).await?;
            if !["can_join", "forbidden"].contains(&value.as_str()) {
                return Err("organization-invalid");
            }
            let content: RoomGuestAccessEventContent =
                serde_json::from_value(json!({"guest_access":value}))
                    .map_err(|_| "organization-invalid")?;
            r.send_state_event(content).await.map_err(failure)?;
            Ok(Page::default())
        }
        Action::Power { room, user, level } => {
            let r = state_room(client, room, StateEventType::RoomPowerLevels).await?;
            let user = UserId::parse(user).map_err(|_| "organization-invalid")?;
            let level =
                matrix_sdk::ruma::Int::try_from(*level).map_err(|_| "organization-invalid")?;
            let power = r.power_levels().await.map_err(|_| "organization-failed")?;
            if !power.user_can_change_user_power_level(r.own_user_id(), &user)
                || level > power.for_user(r.own_user_id())
            {
                return Err("room-action-forbidden");
            }
            r.update_power_levels(vec![(&user, level)])
                .await
                .map_err(failure)?;
            Ok(Page::default())
        }
        Action::Moderate {
            room,
            user,
            kind,
            reason,
        } => {
            let r = joined(client, room)?;
            let user = UserId::parse(user).map_err(|_| "organization-invalid")?;
            if reason.len() > 4096 {
                return Err("organization-invalid");
            }
            let p = r.power_levels().await.map_err(|_| "organization-failed")?;
            let own = r.own_user_id();
            let allowed = match kind.as_str() {
                "kick" => p.user_can_kick_user(own, &user),
                "ban" => p.user_can_ban_user(own, &user),
                "unban" => p.user_can_unban_user(own, &user),
                "invite" => p.user_can_invite(own),
                _ => false,
            };
            if !allowed {
                return Err("room-action-forbidden");
            }
            let reason = (!reason.is_empty()).then_some(reason.as_str());
            match kind.as_str() {
                "kick" => r.kick_user(&user, reason).await,
                "ban" => r.ban_user(&user, reason).await,
                "unban" => r.unban_user(&user, reason).await,
                _ => r.invite_user_by_id(&user).await,
            }
            .map_err(failure)?;
            Ok(Page::default())
        }
        Action::SpaceChild {
            room,
            child,
            remove,
        } => {
            let r = state_room(client, room, StateEventType::SpaceChild).await?;
            if !r.is_space() {
                return Err("organization-invalid");
            }
            let child = RoomId::parse(child).map_err(|_| "organization-invalid")?;
            if child == r.room_id() {
                return Err("organization-invalid");
            }
            let value = if *remove {
                json!({})
            } else {
                json!({"via":[client.user_id().ok_or("session-expired")?.server_name()],"suggested":true})
            };
            r.send_state_event_raw(
                "m.space.child",
                child.as_str(),
                matrix_sdk::ruma::serde::Raw::from_json(
                    serde_json::value::to_raw_value(&value).map_err(|_| "organization-invalid")?,
                ),
            )
            .await
            .map_err(failure)?;
            Ok(Page::default())
        }
    }
}
fn joined(client: &Client, id: &str) -> Result<Room, &'static str> {
    crate::room_tools::joined(client, id)
}
pub(crate) async fn state_room(
    client: &Client,
    id: &str,
    kind: StateEventType,
) -> Result<Room, &'static str> {
    let r = joined(client, id)?;
    if !r
        .power_levels()
        .await
        .map_err(|_| "organization-failed")?
        .user_can_send_state(r.own_user_id(), kind)
    {
        return Err("room-action-forbidden");
    }
    Ok(r)
}
