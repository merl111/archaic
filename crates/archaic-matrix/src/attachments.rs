use crate::{
    ToolRequest,
    message_actions::{failure, target},
};
use matrix_sdk::{
    Room,
    attachment::AttachmentConfig,
    ruma::{
        EventId,
        events::{AnySyncMessageLikeEvent, AnySyncTimelineEvent, SyncMessageLikeEvent},
    },
};
use std::{io::Read, path::Path};
#[cfg(test)]
use std::{io::Write, path::PathBuf};
const MAX_FILE: u64 = 25 * 1024 * 1024;
#[derive(Default)]
pub(crate) struct Attachments {
    pending: Option<Prepared>,
}
struct Prepared {
    request: ToolRequest,
    name: String,
    bytes: Vec<u8>,
    mime: mime::Mime,
}
impl Attachments {
    pub async fn upload(
        &mut self,
        room: &Room,
        request: &ToolRequest,
        path: &Path,
    ) -> Result<(), &'static str> {
        if self.pending.as_ref().is_none_or(|p| &p.request != request) {
            let path = path.to_owned();
            let (name, bytes) = tokio::task::spawn_blocking(move || read_file(&path))
                .await
                .map_err(|_| "file-read-failed")??;
            let mime = detect_mime(&bytes);
            self.pending = Some(Prepared {
                request: request.clone(),
                name,
                bytes,
                mime,
            });
        }
        let file = self.pending.as_ref().ok_or("file-read-failed")?;
        room.send_attachment(
            file.name.clone(),
            &file.mime,
            file.bytes.clone(),
            AttachmentConfig::new().txn_id(request.id.clone()),
        )
        .await
        .map_err(failure)?;
        self.pending = None;
        Ok(())
    }
}
pub(crate) fn read_file(path: &Path) -> Result<(String, Vec<u8>), &'static str> {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .ok_or("file-read-failed")?
        .to_owned();
    if !std::fs::metadata(path)
        .map_err(|_| "file-read-failed")?
        .is_file()
    {
        return Err("file-size-limit");
    }
    let file = std::fs::File::open(path).map_err(|_| "file-read-failed")?;
    let metadata = file.metadata().map_err(|_| "file-read-failed")?;
    if !metadata.is_file() || metadata.len() > MAX_FILE {
        return Err("file-size-limit");
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "file-read-failed")?;
    if bytes.len() as u64 > MAX_FILE {
        return Err("file-size-limit");
    }
    Ok((name, bytes))
}
pub(crate) fn detect_mime(bytes: &[u8]) -> mime::Mime {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        mime::IMAGE_PNG
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        mime::IMAGE_JPEG
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        mime::IMAGE_GIF
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        "image/webp".parse().expect("static MIME type")
    } else {
        mime::APPLICATION_OCTET_STREAM
    }
}
pub(crate) async fn download(room: &Room, event_id: &str, path: &Path) -> Result<(), &'static str> {
    download_progress(room, event_id, path, None).await
}
pub(crate) async fn download_progress(
    room: &Room,
    event_id: &str,
    path: &Path,
    progress: Option<crate::transfers::Progress>,
) -> Result<(), &'static str> {
    if path.exists() {
        return Err("file-exists");
    }
    let id = EventId::parse(event_id).map_err(|_| "message-unavailable")?;
    let event = target(room, &id).await?;
    let AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::RoomMessage(
        SyncMessageLikeEvent::Original(event),
    )) = event
    else {
        return Err("attachment-unavailable");
    };
    if event.event_id != id {
        return Err("attachment-unavailable");
    }
    let value = serde_json::to_value(&event.content).map_err(|_| "attachment-unavailable")?;
    if value["info"]["size"].as_u64().is_some_and(|n| n > MAX_FILE) {
        return Err("file-size-limit");
    }
    if !matches!(
        value["msgtype"].as_str(),
        Some("m.file" | "m.image" | "m.video" | "m.audio")
    ) {
        return Err("attachment-unavailable");
    }
    use matrix_sdk::{media::MediaEventContent, ruma::events::room::message::MessageType};
    let source = match &event.content.msgtype {
        MessageType::File(c) => c.source(),
        MessageType::Image(c) => c.source(),
        MessageType::Video(c) => c.source(),
        MessageType::Audio(c) => c.source(),
        _ => None,
    }
    .ok_or("attachment-unavailable")?;
    crate::transfers::download_source(&room.client(), source, path, progress).await
}
#[cfg(test)]
fn save_file(path: PathBuf, bytes: &[u8]) -> Result<(), &'static str> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or("file-save-failed")?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|_| "file-save-failed")?;
    file.write_all(bytes)
        .and_then(|_| file.as_file().sync_all())
        .map_err(|_| "file-save-failed")?;
    file.persist_noclobber(path).map_err(|e| {
        if e.error.kind() == std::io::ErrorKind::AlreadyExists {
            "file-exists"
        } else {
            "file-save-failed"
        }
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_files_are_bounded_regular_and_saved_without_clobbering() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report");
        save_file(path.clone(), b"original").unwrap();
        assert_eq!(
            read_file(&path).unwrap(),
            ("report".into(), b"original".to_vec())
        );
        assert_eq!(save_file(path.clone(), b"overwrite"), Err("file-exists"));
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
        assert_eq!(read_file(dir.path()), Err("file-size-limit"));
        std::fs::File::create(&path)
            .unwrap()
            .set_len(MAX_FILE + 1)
            .unwrap();
        assert_eq!(read_file(&path), Err("file-size-limit"));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
    #[test]
    fn content_detection_does_not_trust_extensions() {
        assert_eq!(detect_mime(b"plain text"), mime::APPLICATION_OCTET_STREAM);
        assert_eq!(detect_mime(b"\x89PNG\r\n\x1a\nimage"), mime::IMAGE_PNG);
        assert_eq!(detect_mime(b"\xff\xd8\xffimage"), mime::IMAGE_JPEG);
        assert_eq!(detect_mime(b"GIF89a"), mime::IMAGE_GIF);
    }
}
