//! Account activity, search persistence and native notification events derived from one SDK sync.
use crate::{Event, EventKind, connection::Session, worker::emit};
use matrix_sdk::sync::SyncResponse;
use tokio::sync::mpsc;
pub(crate) async fn update(
    active: &mut Session,
    response: &SyncResponse,
    events: &mpsc::Sender<Event>,
) {
    blocked(active, response, events).await;
    index_history(active, response, events).await;
    activity(active, response, events).await;
    notifications(active, response, events).await;
}
async fn blocked(active: &mut Session, response: &SyncResponse, events: &mpsc::Sender<Event>) {
    if (!active.initial_sync_done
        || response.account_data.iter().any(|e| {
            e.get_field::<String>("type").ok().flatten().as_deref() == Some("m.ignored_user_list")
        }))
        && let Ok(Some(raw))=active.client.account().account_data::<matrix_sdk::ruma::events::ignored_user_list::IgnoredUserListEventContent>().await
            && let Ok(content)=raw.deserialize() {
                active.blocked=content.ignored_users.keys().map(ToString::to_string).collect();
                emit(events,active.epoch,EventKind::Blocked(active.blocked.clone())).await;
            }
}
async fn index_history(
    active: &mut Session,
    response: &SyncResponse,
    events: &mpsc::Sender<Event>,
) {
    for (id, update) in &response.rooms.joined {
        if let Some(room) = active.client.get_room(id) {
            if update.timeline.limited
                && let Some(cursor) = &update.timeline.prev_batch
                && let Err(key) = active.local.index.gap(id.as_str(), cursor)
            {
                emit(events, active.epoch, EventKind::Error(key)).await;
            }
            let values = update
                .timeline
                .events
                .iter()
                .filter_map(|e| crate::details::value(&room, e).ok())
                .collect::<Vec<_>>();
            match active.local.index.merge(
                id.as_str(),
                values,
                room.clone_info()
                    .room_version_rules_or_default()
                    .redaction
                    .keep_room_redaction_redacts,
            ) {
                Ok(_) => {}
                Err(key) => emit(events, active.epoch, EventKind::Error(key)).await,
            }
        }
    }
}

async fn activity(active: &mut Session, response: &SyncResponse, events: &mpsc::Sender<Event>) {
    for (id, update) in &response.rooms.joined {
        let own = active.client.user_id().map(|u| u.as_str());
        let typing = update
            .ephemeral
            .iter()
            .filter_map(|e| serde_json::from_str::<serde_json::Value>(e.json().get()).ok())
            .find(|e| e["type"] == "m.typing")
            .map(|e| typing_users(&e, own));
        let unread = active
            .client
            .get_room(id)
            .map(|r| r.unread_notification_counts().notification_count)
            .unwrap_or_default();
        emit(
            events,
            active.epoch,
            EventKind::Activity {
                room_id: id.to_string(),
                unread,
                latest: update
                    .timeline
                    .events
                    .iter()
                    .rev()
                    .find_map(|e| e.raw().get_field::<String>("event_id").ok().flatten()),
                typing,
            },
        )
        .await;
    }
}
fn typing_users(event: &serde_json::Value, own: Option<&str>) -> Vec<String> {
    event["content"]["user_ids"]
        .as_array()
        .map(|users| {
            users
                .iter()
                .filter_map(|v| v.as_str())
                .filter(|u| Some(*u) != own)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}
async fn notifications(
    active: &mut Session,
    response: &SyncResponse,
    events: &mpsc::Sender<Event>,
) {
    for (room_id, notifications) in &response.notifications {
        let mut notify = None;
        for notification in notifications {
            if !notification
                .actions
                .iter()
                .any(|a| matches!(a, matrix_sdk::ruma::push::Action::Notify))
            {
                continue;
            }
            if let matrix_sdk::deserialized_responses::RawAnySyncOrStrippedTimelineEvent::Sync(
                raw,
            ) = &notification.event
            {
                let own = raw
                    .get_field::<String>("sender")
                    .ok()
                    .flatten()
                    .is_some_and(|u| {
                        active.blocked.contains(&u)
                            || Some(u.as_str()) == active.client.user_id().map(|u| u.as_str())
                    });
                if let Some(id) = raw.get_field::<String>("event_id").ok().flatten()
                    && !own
                    && active.notified.insert(id.clone())
                    && active.initial_sync_done
                {
                    notify = Some(id);
                }
            }
        }
        if let Some(event_id) = notify {
            let name = active
                .client
                .get_room(room_id)
                .map(|r| r.name().unwrap_or_else(|| room_id.to_string()))
                .unwrap_or_else(|| room_id.to_string());
            emit(
                events,
                active.epoch,
                EventKind::Notification {
                    room_id: room_id.to_string(),
                    name,
                    event_id,
                },
            )
            .await;
        }
    }
    if active.notified.len() > 4096 {
        active.notified.clear();
    }
    active.initial_sync_done = true;
}
