//! Session-owned, bounded background avatar and membership requests.
use crate::{Event, EventKind, Message, worker::emit};
use matrix_sdk::{
    Client, Room, RoomMemberships,
    media::{MediaFormat, MediaThumbnailSettings},
};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    sync::{Semaphore, mpsc},
    task::JoinHandle,
};

pub(crate) struct Avatars {
    jobs: HashMap<String, (String, Instant, JoinHandle<()>)>,
    permits: Arc<Semaphore>,
    image_room: String,
}
impl Default for Avatars {
    fn default() -> Self {
        Self {
            jobs: HashMap::new(),
            image_room: String::new(),
            permits: Arc::new(Semaphore::new(4)),
        }
    }
}
impl Drop for Avatars {
    fn drop(&mut self) {
        for (_, _, job) in self.jobs.values() {
            job.abort();
        }
    }
}
impl Avatars {
    fn start(
        &mut self,
        key: String,
        version: String,
        future: impl Future<Output = ()> + Send + 'static,
    ) {
        if self.jobs.get(&key).is_some_and(|(v, at, job)| {
            v == &version && (!job.is_finished() || at.elapsed() < Duration::from_secs(60))
        }) {
            return;
        }
        if self.jobs.len() >= 1024 && !self.jobs.contains_key(&key) {
            self.jobs.retain(|_, (_, _, job)| !job.is_finished());
            if self.jobs.len() >= 1024 {
                return;
            }
        }
        if let Some((_, _, old)) = self.jobs.remove(&key) {
            old.abort();
        }
        let permits = self.permits.clone();
        let job = tokio::spawn(async move {
            let Ok(_permit) = permits.acquire_owned().await else {
                return;
            };
            future.await;
        });
        self.jobs.insert(key, (version, Instant::now(), job));
    }
    pub fn rooms(
        &mut self,
        client: &Client,
        selected: Option<&str>,
        events: &mpsc::Sender<Event>,
        epoch: u64,
    ) {
        let (account_client, account_events) = (client.clone(), events.clone());
        self.start("account".into(), String::new(), async move {
            if let Ok(name) = account_client.account().get_display_name().await {
                emit(
                    &account_events,
                    epoch,
                    EventKind::DisplayName {
                        key: "account".into(),
                        name: name.unwrap_or_default(),
                    },
                )
                .await;
            }
            if let Ok(url) = account_client.account().get_avatar_url().await {
                let bytes = match url {
                    Some(url) => account_client
                        .media()
                        .get_media_content(
                            &matrix_sdk::media::MediaRequestParameters {
                                source: matrix_sdk::ruma::events::room::MediaSource::Plain(url),
                                format: format(),
                            },
                            true,
                        )
                        .await
                        .ok(),
                    None => None,
                };
                deliver(&account_events, epoch, "account".into(), bytes).await;
            }
        });
        for room in client.rooms() {
            let key = format!("room:{}", room.room_id());
            let version = room.avatar_url().map(|u| u.to_string()).unwrap_or_default();
            let (image_events, room_copy) = (events.clone(), room.clone());
            self.start(key.clone(), version, async move {
                let bytes = room_avatar(&room_copy).await;
                if let Ok(bytes) = bytes {
                    deliver(&image_events, epoch, key, bytes).await;
                }
            });
            if selected == Some(room.room_id().as_str()) {
                let events = events.clone();
                self.start(
                    format!("members:{}", room.room_id()),
                    String::new(),
                    async move {
                        if let Ok(members) = room.members(RoomMemberships::JOIN).await {
                            emit(
                                &events,
                                epoch,
                                EventKind::MemberCount {
                                    room_id: room.room_id().to_string(),
                                    count: members.len() as u64,
                                },
                            )
                            .await;
                        }
                    },
                );
            }
        }
    }
    pub fn users(
        &mut self,
        room: &Room,
        messages: &[Message],
        events: &mpsc::Sender<Event>,
        epoch: u64,
    ) {
        self.images(room, messages, events, epoch);
        for sender in messages
            .iter()
            .flat_map(|m| {
                std::iter::once(m.sender.clone())
                    .chain(m.reactions.iter().map(|r| r.sender.clone()))
                    .chain(m.read_by.iter().cloned())
                    .chain(m.thread_read_by.iter().cloned())
            })
            .chain(room.client().user_id().map(ToString::to_string))
        {
            let key = format!("user:{}:{sender}", room.room_id());
            let (events, room) = (events.clone(), room.clone());
            self.start(key.clone(), String::new(), async move {
                let Ok(id) = matrix_sdk::ruma::UserId::parse(&sender) else {
                    return;
                };
                if let Ok(Some(member)) = room.get_member(&id).await {
                    emit(
                        &events,
                        epoch,
                        EventKind::DisplayName {
                            key: key.clone(),
                            name: member.display_name().unwrap_or_default().to_owned(),
                        },
                    )
                    .await;
                    if let Ok(bytes) = member.avatar(format()).await {
                        deliver(&events, epoch, key, bytes).await;
                    }
                }
            });
        }
    }
    fn images(
        &mut self,
        room: &Room,
        messages: &[Message],
        events: &mpsc::Sender<Event>,
        epoch: u64,
    ) {
        if self.image_room != room.room_id().as_str() {
            self.jobs.retain(|key, (_, _, job)| {
                if key.starts_with("image:") {
                    job.abort();
                    false
                } else {
                    true
                }
            });
            self.image_room = room.room_id().to_string();
        }
        for message in messages
            .iter()
            .rev()
            .filter(|m| {
                !m.deleted
                    && !m.undecrypted
                    && m.id.starts_with('$')
                    && m.attachment.as_ref().is_some_and(|a| a.kind == "m.image")
            })
            .take(48)
        {
            let key = format!("image:{}:{}", room.room_id(), message.id);
            if self.jobs.contains_key(&key) {
                continue;
            }
            // Finished thumbnail jobs can be evicted as older history is loaded.
            if self.jobs.keys().filter(|k| k.starts_with("image:")).count() >= 48 {
                if let Some(key) = self
                    .jobs
                    .iter()
                    .filter(|(k, (_, _, j))| k.starts_with("image:") && j.is_finished())
                    .min_by_key(|(_, (_, at, _))| *at)
                    .map(|(k, _)| k.clone())
                {
                    self.jobs.remove(&key);
                } else {
                    break;
                }
            }
            let (room, events, event_id) = (room.clone(), events.clone(), message.id.clone());
            self.start(key, String::new(), async move {
                let result =
                    crate::transfers::preview_image(&room, &event_id, None, 512, 384).await;
                emit(
                    &events,
                    epoch,
                    EventKind::ImagePreview {
                        room_id: room.room_id().to_string(),
                        event_id,
                        result,
                    },
                )
                .await;
            });
        }
    }
}
fn format() -> MediaFormat {
    MediaFormat::Thumbnail(MediaThumbnailSettings::new(256u32.into(), 256u32.into()))
}
async fn deliver(events: &mpsc::Sender<Event>, epoch: u64, key: String, bytes: Option<Vec<u8>>) {
    // Decode away from the UI thread, with bounded dimensions and allocation.
    let pixels = match bytes {
        Some(bytes) => tokio::task::spawn_blocking(move || decode(&bytes))
            .await
            .ok()
            .flatten(),
        None => None,
    };
    emit(events, epoch, EventKind::Avatar { key, pixels }).await;
}
fn decode(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() > 2 * 1024 * 1024 {
        return None;
    }
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    Some(
        reader
            .decode()
            .ok()?
            .resize_to_fill(128, 128, image::imageops::FilterType::Triangle)
            .to_rgba8()
            .into_raw(),
    )
}
// Direct rooms often deliberately have no m.room.avatar. Match their peer's
// profile, rather than leaving an initial forever. Group rooms keep room art.
async fn room_avatar(room: &Room) -> matrix_sdk::Result<Option<Vec<u8>>> {
    if let Some(bytes) = room.avatar(format()).await? {
        return Ok(Some(bytes));
    }
    if room.is_direct().await? && room.active_members_count() <= 2 {
        let members = room.members(RoomMemberships::ACTIVE).await?;
        for member in members {
            if Some(member.user_id()) != room.client().user_id() {
                return member.avatar(format()).await;
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    #[test]
    fn bounded_decoder() {
        assert!(super::decode(b"not an image").is_none());
        let mut data = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(80, 40)
            .write_to(&mut data, image::ImageFormat::Png)
            .unwrap();
        assert_eq!(super::decode(data.get_ref()).unwrap().len(), 128 * 128 * 4);
    }
}
