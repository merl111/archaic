//! Cancellable background IO. Downloads are bounded while streaming and published only after integrity verification.
use crate::{Event, EventKind, ToolAction, ToolData, ToolRequest, TransactionId};
use matrix_sdk::{Client, attachment::AttachmentConfig, ruma::events::room::MediaSource};
use std::{
    io::{Read, Seek, Write},
    path::Path,
};
use tokio::sync::mpsc;
const LIMIT: u64 = 25 * 1024 * 1024;
#[derive(Default)]
pub(crate) struct Transfers {
    task: Option<(ToolRequest, tokio::task::JoinHandle<()>)>,
}
impl Drop for Transfers {
    fn drop(&mut self) {
        if let Some((_, t)) = &self.task {
            t.abort();
        }
    }
}
impl Transfers {
    pub async fn start(
        &mut self,
        client: Client,
        epoch: u64,
        request: ToolRequest,
        events: mpsc::Sender<Event>,
    ) {
        if self.task.as_ref().is_some_and(|(_, t)| !t.is_finished()) {
            let _ = events
                .send(Event {
                    epoch,
                    kind: EventKind::RoomToolResult {
                        request,
                        result: Err("transfer-busy"),
                    },
                })
                .await;
            return;
        }
        let target = request.clone();
        let task = tokio::spawn(async move {
            let result = perform(&client, &target, epoch, &events).await;
            let _ = events
                .send(Event {
                    epoch,
                    kind: EventKind::RoomToolResult {
                        request: target,
                        result,
                    },
                })
                .await;
        });
        self.task = Some((request, task));
    }
    pub async fn cancel(&mut self, id: &TransactionId, epoch: u64, events: &mpsc::Sender<Event>) {
        if self
            .task
            .as_ref()
            .is_some_and(|(r, t)| &r.id == id && !t.is_finished())
        {
            let (request, task) = self.task.take().unwrap();
            task.abort();
            let _ = task.await;
            let _ = events
                .send(Event {
                    epoch,
                    kind: EventKind::RoomToolResult {
                        request,
                        result: Err("transfer-cancelled"),
                    },
                })
                .await;
        }
    }
}
async fn perform(
    client: &Client,
    request: &ToolRequest,
    epoch: u64,
    events: &mpsc::Sender<Event>,
) -> Result<ToolData, &'static str> {
    let room = crate::room_tools::joined(client, &request.room_id)?;
    match &request.action {
        ToolAction::QueueUpload { path } => {
            let path = path.clone();
            let (name, bytes) =
                tokio::task::spawn_blocking(move || crate::attachments::read_file(&path))
                    .await
                    .map_err(|_| "file-read-failed")??;
            let mime = crate::attachments::detect_mime(&bytes);
            let preview = if mime.type_() == mime::IMAGE {
                let preview_bytes = bytes.clone();
                tokio::task::spawn_blocking(move || {
                    decode_preview(
                        image::ImageReader::new(std::io::Cursor::new(preview_bytes)),
                        512,
                        384,
                    )
                })
                .await
                .ok()
                .and_then(Result::ok)
            } else {
                None
            };
            room.send_queue()
                .send_attachment(
                    name,
                    mime,
                    bytes,
                    AttachmentConfig::new().txn_id(request.id.clone()),
                )
                .await
                .map_err(|_| "outbox-failed")?;
            if let Some(data) = preview {
                crate::worker::emit(
                    events,
                    epoch,
                    EventKind::ImagePreview {
                        room_id: request.room_id.clone(),
                        event_id: format!("~{}", request.id),
                        result: Ok(data),
                    },
                )
                .await;
            }
            Ok(ToolData::QueuedUpload)
        }
        ToolAction::Download { event_id, path } => {
            crate::attachments::download_progress(
                &room,
                event_id,
                path,
                Some(Progress {
                    epoch,
                    request: request.clone(),
                    events: events.clone(),
                }),
            )
            .await?;
            Ok(ToolData::Downloaded)
        }
        ToolAction::Preview { event_id } => {
            let image = preview_image(
                &room,
                event_id,
                Some(Progress {
                    epoch,
                    request: request.clone(),
                    events: events.clone(),
                }),
                1024,
                768,
            )
            .await?;
            Ok(ToolData::Preview {
                pixels: image.pixels,
                width: image.width,
                height: image.height,
            })
        }
        _ => Err("attachment-unavailable"),
    }
}
pub(crate) struct Progress {
    pub epoch: u64,
    pub request: ToolRequest,
    pub events: mpsc::Sender<Event>,
}
impl Progress {
    fn report(&self, current: u64, total: u64) {
        let _ = self.events.try_send(Event {
            epoch: self.epoch,
            kind: EventKind::TransferProgress {
                room_id: self.request.room_id.clone(),
                id: self.request.id.clone(),
                current,
                total,
            },
        });
    }
}
pub(crate) async fn download_source(
    client: &Client,
    source: MediaSource,
    path: &Path,
    progress: Option<Progress>,
) -> Result<(), &'static str> {
    if path.exists() {
        return Err("file-exists");
    }
    let (uri, info) = match source {
        MediaSource::Plain(uri) => (uri, None),
        MediaSource::Encrypted(file) => {
            let uri = file.url.clone();
            (
                uri,
                Some(matrix_sdk_crypto::MediaEncryptionInfo::from(*file)),
            )
        }
    };
    let (server, media) = uri.parts().map_err(|_| "attachment-unavailable")?;
    let mut url = client.homeserver();
    url.path_segments_mut()
        .map_err(|_| "attachment-unavailable")?
        .pop_if_empty()
        .extend([
            "_matrix",
            "client",
            "v1",
            "media",
            "download",
            server.as_str(),
            media,
        ]);
    // Reject redirects: never forward account credentials to a media-provided origin.
    let http = matrix_sdk::reqwest::Client::builder()
        .redirect(matrix_sdk::reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|_| "file-save-failed")?;
    let token = client.access_token().ok_or("session-expired")?;
    let mut response = http
        .get(url.clone())
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|_| "message-action-failed")?;
    if response.status().as_u16() == 401 {
        // The SDK serializes refresh and invokes the encrypted token persistence callback.
        client
            .refresh_access_token()
            .await
            .map_err(|_| "session-expired")?;
        response = http
            .get(url)
            .bearer_auth(client.access_token().ok_or("session-expired")?)
            .send()
            .await
            .map_err(|_| "message-action-failed")?;
        if response.status().as_u16() == 401 {
            return Err("session-expired");
        }
    }
    if !response.status().is_success() {
        return Err("message-action-failed");
    }
    let total = response.content_length().unwrap_or(0);
    if total > LIMIT {
        return Err("file-size-limit");
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or("file-save-failed")?;
    let mut incoming = tempfile::NamedTempFile::new_in(parent).map_err(|_| "file-save-failed")?;
    let mut received = 0u64;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "message-action-failed")?
    {
        received = received
            .checked_add(chunk.len() as u64)
            .ok_or("file-size-limit")?;
        if received > LIMIT {
            return Err("file-size-limit");
        }
        incoming.write_all(&chunk).map_err(|_| "file-save-failed")?;
        if let Some(p) = &progress {
            p.report(received, total);
        }
    }
    let output = if let Some(info) = info {
        incoming.rewind().map_err(|_| "file-save-failed")?;
        let mut verified =
            tempfile::NamedTempFile::new_in(parent).map_err(|_| "file-save-failed")?;
        let mut decryptor = matrix_sdk_crypto::AttachmentDecryptor::new(&mut incoming, info)
            .map_err(|_| "message-action-failed")?;
        let mut chunk = [0; 64 * 1024];
        loop {
            let count = decryptor
                .read(&mut chunk)
                .map_err(|_| "message-action-failed")?;
            if count == 0 {
                break;
            }
            verified
                .write_all(&chunk[..count])
                .map_err(|_| "file-save-failed")?;
            tokio::task::yield_now().await;
        }
        verified
    } else {
        incoming
    };
    output
        .as_file()
        .sync_all()
        .map_err(|_| "file-save-failed")?;
    output.persist_noclobber(path).map_err(|e| {
        if e.error.kind() == std::io::ErrorKind::AlreadyExists {
            "file-exists"
        } else {
            "file-save-failed"
        }
    })?;
    if let Some(p) = &progress {
        p.report(received, received);
    }
    Ok(())
}

/// Shared by automatic thumbnails and the explicit larger viewer. Downloads
/// remain size-bounded, authenticated and verified before pixels are exposed.
pub(crate) async fn preview_image(
    room: &matrix_sdk::Room,
    event: &str,
    progress: Option<Progress>,
    max_width: u32,
    max_height: u32,
) -> Result<crate::ImagePreview, &'static str> {
    let dir = tempfile::tempdir().map_err(|_| "file-save-failed")?;
    let path = dir.path().join("verified-image");
    crate::attachments::download_progress(room, event, &path, progress).await?;
    tokio::task::spawn_blocking(move || {
        let _keep = dir;
        decode_preview(
            image::ImageReader::open(path).map_err(|_| "preview-invalid")?,
            max_width,
            max_height,
        )
    })
    .await
    .map_err(|_| "preview-invalid")?
}
fn decode_preview<R: std::io::BufRead + Seek>(
    reader: image::ImageReader<R>,
    max_width: u32,
    max_height: u32,
) -> Result<crate::ImagePreview, &'static str> {
    let mut reader = reader
        .with_guessed_format()
        .map_err(|_| "preview-invalid")?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().map_err(|_| "preview-invalid")?;
    let image = if image.width() > max_width || image.height() > max_height {
        image.thumbnail(max_width, max_height)
    } else {
        image
    }
    .into_rgba8();
    let (width, height) = image.dimensions();
    Ok(crate::ImagePreview {
        pixels: image.into_raw(),
        width,
        height,
    })
}

#[cfg(test)]
mod preview_tests {
    use super::*;
    #[test]
    fn decode_fits_large_images_without_upscaling_and_rejects_invalid_data() {
        for (w, h, expected) in [
            (2, 3, (2, 3)),
            (1600, 900, (512, 288)),
            (900, 1600, (216, 384)),
        ] {
            let mut png = std::io::Cursor::new(Vec::new());
            image::RgbaImage::from_pixel(w, h, image::Rgba([15, 100, 200, 255]))
                .write_to(&mut png, image::ImageFormat::Png)
                .unwrap();
            png.set_position(0);
            let preview = decode_preview(image::ImageReader::new(png), 512, 384).unwrap();
            assert_eq!((preview.width, preview.height), expected);
            assert_eq!(
                preview.pixels.len(),
                (preview.width * preview.height * 4) as usize
            );
        }
        assert!(
            decode_preview(
                image::ImageReader::new(std::io::Cursor::new(b"not an image")),
                512,
                384
            )
            .is_err()
        );
    }
}
