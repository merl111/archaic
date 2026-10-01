use super::super::{next, server};
use archaic_matrix::{
    Command, EventKind, channels,
    security::{Action, Secret},
    transaction_id,
};
use matrix_sdk::ruma::{
    device_id, events::room::message::RoomMessageEventContent, room_id, user_id,
};
use matrix_sdk_crypto::{EncryptionSettings, OlmMachine, encrypt_room_key_export};
use serde_json::json;
use wiremock::{
    Mock, ResponseTemplate,
    matchers::{method, path_regex},
};

#[tokio::test]
async fn importing_real_megolm_keys_unlocks_loaded_history_without_refetching() {
    let server = server().await;
    let room = room_id!("!studio:local");
    let machine = OlmMachine::new(user_id!("@bob:local"), device_id!("SOURCE")).await;
    machine
        .share_room_key(room, std::iter::empty(), EncryptionSettings::default())
        .await
        .unwrap();
    let encrypted = machine
        .encrypt_room_event(
            room,
            RoomMessageEventContent::text_plain("Recovered history"),
        )
        .await
        .unwrap();
    let content: serde_json::Value = serde_json::from_str(encrypted.content.json().get()).unwrap();
    let event = json!({"type":"m.room.encrypted","sender":"@bob:local","event_id":"$encrypted","origin_server_ts":10,"content":content});
    Mock::given(method("GET"))
        .and(path_regex(".*/messages$"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"start":"history","end":"older","chunk":[event],"state":[]})),
        )
        .with_priority(1)
        .expect(1)
        .mount(&server)
        .await;
    let (sender, commands, events, mut receiver) = channels();
    let worker = tokio::spawn(archaic_matrix::run(commands, events));
    sender
        .send(Command::Login {
            epoch: 41,
            server: server.uri(),
            username: "alice".into(),
            password: "test".into(),
        })
        .await
        .unwrap();
    next(
        &mut receiver,
        |e| matches!(e,EventKind::Rooms(rooms) if !rooms.is_empty()),
    )
    .await;
    sender
        .send(Command::Select(room.to_string()))
        .await
        .unwrap();
    let EventKind::History {
        messages,
        info: before,
        ..
    } = next(&mut receiver, |e| matches!(e, EventKind::History { .. }))
        .await
        .kind
    else {
        unreachable!()
    };
    assert_eq!(messages.len(), 1);
    assert!(messages[0].undecrypted);
    assert!(messages[0].body.is_none());
    let keys = machine.store().export_room_keys(|_| true).await.unwrap();
    assert!(!keys.is_empty());
    let export = encrypt_room_key_export(&keys, "test-only", 1).unwrap();
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), export).unwrap();
    let id = transaction_id();
    sender
        .send(Command::Security {
            id: id.clone(),
            action: Action::Import {
                path: file.path().to_owned(),
                passphrase: Secret::new("test-only".into()),
            },
        })
        .await
        .unwrap();
    let result = next(
        &mut receiver,
        |e| matches!(e,EventKind::SecurityResult{id:other,..} if *other==id),
    )
    .await;
    assert!(matches!(
        result.kind,
        EventKind::SecurityResult { result: Ok(_), .. }
    ));
    let EventKind::History {messages,info,..} = next(&mut receiver, |e| matches!(e,EventKind::History{messages,..} if messages.iter().any(|m| m.body.is_some()))).await.kind else {unreachable!()};
    assert_eq!(info, before);
    assert_eq!(messages[0].id, "$encrypted");
    assert!(!messages[0].undecrypted);
    assert_eq!(messages[0].body.as_deref(), Some("Recovered history"));
    worker.abort();
    let _ = worker.await;
    server.verify().await;
}
