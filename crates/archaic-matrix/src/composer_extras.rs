//! Composer event types use the SDK's encryption and transaction handling.
use crate::{ToolAction, ToolRequest, message_actions::failure};
use matrix_sdk::{
    Room,
    attachment::{AttachmentConfig, AttachmentInfo, BaseAudioInfo},
};
use serde_json::{Value, json};
fn poll(question: &str, answers: &[String]) -> Result<Value, &'static str> {
    let question = question.trim();
    let answers: Vec<_> = answers
        .iter()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .collect();
    if question.is_empty()
        || question.len() > 500
        || !(2..=20).contains(&answers.len())
        || answers.iter().any(|s| s.len() > 200)
        || answers
            .iter()
            .map(|s| s.to_lowercase())
            .collect::<std::collections::HashSet<_>>()
            .len()
            != answers.len()
    {
        return Err("poll-invalid");
    }
    Ok(
        json!({"org.matrix.msc1767.text": format!("{}\n{}", question, answers.join("\n")),
        "org.matrix.msc3381.poll.start": {"question": {"org.matrix.msc1767.text": question},
        "kind": "org.matrix.msc3381.poll.disclosed", "max_selections": 1,
        "answers": answers.iter().enumerate().map(|(i,a)| json!({"id": format!("answer-{i}"), "org.matrix.msc1767.text": a})).collect::<Vec<_>>()}}),
    )
}
pub(crate) async fn send(room: &Room, request: &ToolRequest) -> Result<(), &'static str> {
    if let ToolAction::Poll { question, answers } = &request.action {
        room.send_raw("org.matrix.msc3381.poll.start", poll(question, answers)?)
            .with_transaction_id(&request.id)
            .await
            .map_err(failure)?;
        return Ok(());
    }
    let path = match &request.action {
        ToolAction::Sticker { path } | ToolAction::Voice { path, .. } => path.clone(),
        _ => return Err("message-unavailable"),
    };
    let (name, bytes) = tokio::task::spawn_blocking(move || crate::attachments::read_file(&path))
        .await
        .map_err(|_| "file-read-failed")??;
    if let ToolAction::Voice { duration_ms, .. } = request.action {
        if !bytes.starts_with(b"RIFF")
            || bytes.get(8..12) != Some(b"WAVE")
            || duration_ms == 0
            || duration_ms > 120_000
        {
            return Err("voice-invalid");
        }
        let info = BaseAudioInfo {
            duration: Some(std::time::Duration::from_millis(duration_ms)),
            size: Some((bytes.len() as u32).into()),
            waveform: None,
        };
        room.send_attachment(
            "Voice message.wav",
            &"audio/wav".parse().unwrap(),
            bytes,
            AttachmentConfig::new()
                .txn_id(request.id.clone())
                .info(AttachmentInfo::Voice(info)),
        )
        .await
        .map_err(failure)?;
        return Ok(());
    }
    let reader = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|_| "preview-invalid")?;
    let (w, h) = reader.into_dimensions().map_err(|_| "preview-invalid")?;
    if w == 0 || h == 0 || w > 4096 || h > 4096 {
        return Err("preview-invalid");
    }
    let mime = crate::attachments::detect_mime(&bytes);
    if mime.type_() != mime::IMAGE {
        return Err("preview-invalid");
    }
    let mut content = json!({"body": name, "info": {"w": w, "h": h, "mimetype": mime.to_string(), "size": bytes.len()}});
    if room
        .latest_encryption_state()
        .await
        .map_err(failure)?
        .is_encrypted()
    {
        let file = room
            .client()
            .upload_encrypted_file(&mut bytes.as_slice())
            .await
            .map_err(failure)?;
        content["file"] = serde_json::to_value(file).map_err(|_| "send-failed")?;
    } else {
        let upload = room
            .client()
            .media()
            .upload(&mime, bytes, None)
            .await
            .map_err(failure)?;
        content["url"] = json!(upload.content_uri);
    }
    room.send_raw("m.sticker", content)
        .with_transaction_id(&request.id)
        .await
        .map_err(failure)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn poll_payload_rejects_ambiguous_answers() {
        let value = poll("Lunch?", &["Pizza".into(), "Sushi".into()]).unwrap();
        assert_eq!(
            value["org.matrix.msc3381.poll.start"]["answers"][1]["id"],
            "answer-1"
        );
        assert!(poll("Lunch?", &["Pizza".into(), "pizza".into()]).is_err());
        assert!(poll("", &["a".into(), "b".into()]).is_err());
        assert!(poll("Q", &["a".into()]).is_err());
    }
}
