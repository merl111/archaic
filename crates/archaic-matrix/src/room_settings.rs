//! Extended, validated room operations. Server policy remains authoritative.
use crate::{
    message_actions::failure,
    organization::{Page, Row, state_room},
};
use matrix_sdk::{
    Client,
    ruma::{
        EventId, RoomAliasId, RoomId,
        events::{StateEventType, room::power_levels::RoomPowerLevelsEventContent},
    },
};
use serde_json::json;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operation {
    AliasAdd,
    AliasRemove,
    CanonicalAlias,
    Publish,
    Unpublish,
    LowPriority,
    NormalPriority,
    Avatar,
    ClearAvatar,
    Restricted,
    EventPower,
    Retention,
    Upgrade,
    Report,
    Redact,
    Knock,
    ParentAdd,
    ParentRemove,
    Threads,
    ParticipatedThreads,
    SubscribeThread,
    UnsubscribeThread,
}
pub(crate) async fn perform(
    client: &Client,
    room: &str,
    operation: &Operation,
    value: &str,
    extra: &str,
    next: Option<String>,
) -> Result<Page, &'static str> {
    use Operation::*;
    if value.len() > 4096 || extra.len() > 4096 || next.as_ref().is_some_and(|n| n.len() > 4096) {
        return Err("organization-invalid");
    }
    if *operation == Knock {
        let room =
            matrix_sdk::ruma::RoomOrAliasId::parse(room).map_err(|_| "organization-invalid")?;
        let result = client
            .knock(room, (!value.is_empty()).then(|| value.to_owned()), vec![])
            .await
            .map_err(failure)?;
        return Ok(Page {
            text: result.room_id().to_string(),
            ..Default::default()
        });
    }
    let joined = crate::room_tools::joined(client, room)?;
    match operation {
        AliasAdd | AliasRemove | CanonicalAlias => {
            let alias = RoomAliasId::parse(value).map_err(|_| "organization-invalid")?;
            if *operation == AliasAdd {
                client
                    .create_room_alias(&alias, joined.room_id())
                    .await
                    .map_err(|e| failure(e.into()))?;
            } else {
                let resolved = client
                    .resolve_room_alias(&alias)
                    .await
                    .map_err(|e| failure(e.into()))?;
                if resolved.room_id != joined.room_id() {
                    return Err("organization-invalid");
                }
                if *operation == AliasRemove {
                    client
                        .remove_room_alias(&alias)
                        .await
                        .map_err(|e| failure(e.into()))?;
                } else {
                    state_room(client, room, StateEventType::RoomCanonicalAlias).await?;
                    state(
                        &joined,
                        "m.room.canonical_alias",
                        "",
                        json!({"alias":alias,"alt_aliases":joined.alt_aliases()}),
                    )
                    .await?;
                }
            }
        }
        Publish | Unpublish => {
            use matrix_sdk::ruma::api::client::{
                directory::set_room_visibility::v3::Request, room::Visibility,
            };
            client
                .send(Request::new(
                    joined.room_id().to_owned(),
                    if *operation == Publish {
                        Visibility::Public
                    } else {
                        Visibility::Private
                    },
                ))
                .await
                .map_err(|e| failure(e.into()))?;
        }
        LowPriority | NormalPriority => {
            joined
                .set_is_low_priority(*operation == LowPriority, None)
                .await
                .map_err(failure)?;
        }
        Avatar | ClearAvatar => {
            state_room(client, room, StateEventType::RoomAvatar).await?;
            if *operation == ClearAvatar {
                joined.remove_avatar().await.map_err(failure)?;
            } else {
                let path = std::path::PathBuf::from(value);
                let (_, bytes) =
                    tokio::task::spawn_blocking(move || crate::attachments::read_file(&path))
                        .await
                        .map_err(|_| "file-read-failed")??;
                let mime = crate::attachments::detect_mime(&bytes);
                if mime.type_() != mime::IMAGE {
                    return Err("preview-invalid");
                }
                joined
                    .upload_avatar(&mime, bytes, None)
                    .await
                    .map_err(failure)?;
            }
        }
        Restricted => {
            state_room(client, room, StateEventType::RoomJoinRules).await?;
            let ids = value
                .split_whitespace()
                .map(RoomId::parse)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| "organization-invalid")?;
            if ids.is_empty() || ids.len() > 20 {
                return Err("organization-invalid");
            }
            state(&joined,"m.room.join_rules","",json!({"join_rule":"restricted","allow":ids.iter().map(|id|json!({"type":"m.room_membership","room_id":id})).collect::<Vec<_>>()})).await?;
        }
        EventPower => {
            state_room(client, room, StateEventType::RoomPowerLevels).await?;
            let level = extra
                .parse::<i64>()
                .ok()
                .and_then(|n| matrix_sdk::ruma::Int::try_from(n).ok())
                .ok_or("organization-invalid")?;
            let power = joined
                .power_levels()
                .await
                .map_err(|_| "organization-failed")?;
            if value.is_empty() || value.len() > 255 || level > power.for_user(joined.own_user_id())
            {
                return Err("room-action-forbidden");
            }
            let old = power
                .events
                .get(&matrix_sdk::ruma::events::TimelineEventType::from(value))
                .copied()
                .unwrap_or(power.state_default);
            if old > power.for_user(joined.own_user_id()) {
                return Err("room-action-forbidden");
            }
            let mut content =
                RoomPowerLevelsEventContent::try_from(power).map_err(|_| "organization-failed")?;
            content.events.insert(value.into(), level);
            joined.send_state_event(content).await.map_err(failure)?;
        }
        Retention => {
            state_room(client, room, StateEventType::from("m.room.retention")).await?;
            let min = value.parse::<u64>().map_err(|_| "organization-invalid")?;
            let max = extra.parse::<u64>().map_err(|_| "organization-invalid")?;
            if min > max || max > 9_007_199_254_740_991 {
                return Err("organization-invalid");
            }
            state(
                &joined,
                "m.room.retention",
                "",
                json!({"min_lifetime":min,"max_lifetime":max}),
            )
            .await?;
        }
        Upgrade => {
            state_room(client, room, StateEventType::RoomTombstone).await?;
            let version = matrix_sdk::ruma::RoomVersionId::try_from(value)
                .map_err(|_| "organization-invalid")?;
            let result = client
                .send(
                    matrix_sdk::ruma::api::client::room::upgrade_room::v3::Request::new(
                        joined.room_id().to_owned(),
                        version,
                    ),
                )
                .await
                .map_err(|e| failure(e.into()))?;
            return Ok(Page {
                rows: vec![Row {
                    id: result.replacement_room.to_string(),
                    title: result.replacement_room.to_string(),
                    detail: String::new(),
                }],
                ..Default::default()
            });
        }
        Report | Redact => {
            let event = EventId::parse(value).map_err(|_| "organization-invalid")?;
            if *operation == Report {
                joined
                    .report_content(event, Some(extra.to_owned()))
                    .await
                    .map_err(failure)?;
            } else {
                let target = crate::message_actions::target(&joined, &event).await?;
                let power = joined
                    .power_levels()
                    .await
                    .map_err(|_| "organization-failed")?;
                let allowed = if target.sender() == joined.own_user_id() {
                    power.user_can_redact_own_event(joined.own_user_id())
                } else {
                    power.user_can_redact_event_of_other(joined.own_user_id())
                };
                if !allowed {
                    return Err("room-action-forbidden");
                }
                joined
                    .redact(&event, (!extra.is_empty()).then_some(extra), None)
                    .await
                    .map_err(|e| failure(e.into()))?;
            }
        }
        ParentAdd | ParentRemove => {
            state_room(client, room, StateEventType::SpaceParent).await?;
            let parent = RoomId::parse(value).map_err(|_| "organization-invalid")?;
            if parent == joined.room_id() {
                return Err("organization-invalid");
            }
            if *operation == ParentAdd {
                let parent_room = crate::room_tools::joined(client, parent.as_str())?;
                if !parent_room.is_space() {
                    return Err("organization-invalid");
                }
            }
            state(
                &joined,
                "m.space.parent",
                parent.as_str(),
                if *operation == ParentRemove {
                    json!({})
                } else {
                    json!({"via":[joined.own_user_id().server_name()],"canonical":true})
                },
            )
            .await?;
        }
        Threads | ParticipatedThreads => {
            return crate::room_threads::list(
                client,
                &joined,
                *operation == ParticipatedThreads,
                next,
            )
            .await;
        }
        SubscribeThread | UnsubscribeThread => {
            let event = EventId::parse(value).map_err(|_| "organization-invalid")?;
            if *operation == SubscribeThread {
                joined
                    .subscribe_thread(event, None)
                    .await
                    .map_err(failure)?;
            } else {
                joined.unsubscribe_thread(event).await.map_err(failure)?;
            }
        }
        Knock => unreachable!(),
    }
    Ok(Page::default())
}
async fn state(
    room: &matrix_sdk::Room,
    kind: &str,
    key: &str,
    value: serde_json::Value,
) -> Result<(), &'static str> {
    room.send_state_event_raw(
        kind,
        key,
        matrix_sdk::ruma::serde::Raw::from_json(
            serde_json::value::to_raw_value(&value).map_err(|_| "organization-invalid")?,
        ),
    )
    .await
    .map_err(failure)?;
    Ok(())
}
