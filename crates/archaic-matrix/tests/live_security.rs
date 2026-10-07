//! Opt-in tests against a disposable loopback Synapse; never reads real credentials.
use archaic_matrix::{
    Command, EventKind, Receiver, Sender, channels,
    security::{Action, Secret, Snapshot},
    transaction_id,
};
use matrix_sdk::{
    Client,
    config::{RequestConfig, SyncSettings},
    encryption::verification::VerificationRequestState,
};
use serde_json::json;
use std::time::Duration;
async fn next(receiver: &mut Receiver, predicate: impl Fn(&EventKind) -> bool) -> EventKind {
    tokio::time::timeout(Duration::from_secs(45), async {
        loop {
            let event = receiver.recv().await.expect("worker running");
            if predicate(&event.kind) {
                return event.kind;
            }
            if let EventKind::Error(key) = event.kind {
                panic!("worker error {key}");
            }
        }
    })
    .await
    .expect("event deadline")
}
async fn action(
    sender: &Sender,
    receiver: &mut Receiver,
    action: Action,
) -> Result<Option<Secret>, &'static str> {
    let id = transaction_id();
    sender
        .send(Command::Security {
            id: id.clone(),
            action,
        })
        .await
        .unwrap();
    let EventKind::SecurityResult { result, .. } = next(
        receiver,
        |e| matches!(e,EventKind::SecurityResult{id:i,..} if *i==id),
    )
    .await
    else {
        unreachable!()
    };
    result
}
async fn snapshot(receiver: &mut Receiver, predicate: impl Fn(&Snapshot) -> bool) -> Snapshot {
    let EventKind::SecuritySnapshot(s) = next(
        receiver,
        |e| matches!(e,EventKind::SecuritySnapshot(s) if predicate(s)),
    )
    .await
    else {
        unreachable!()
    };
    s
}
async fn wait<T, F, Fut>(mut f: F) -> T
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Option<T>>,
{
    tokio::time::timeout(Duration::from_secs(45), async {
        loop {
            if let Some(v) = f().await {
                return v;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("SDK state deadline")
}
#[tokio::test]
#[ignore = "requires disposable loopback Synapse via ARCHAIC_TEST_SERVER"]
async fn real_device_verification_recovery_and_encrypted_key_export() {
    let server = std::env::var("ARCHAIC_TEST_SERVER").expect("explicit test server");
    let url = url::Url::parse(&server).unwrap();
    assert_eq!(url.host_str(), Some("127.0.0.1"));
    let username = format!("archaic{}", transaction_id());
    let password = "disposable-test-password";
    let response=matrix_sdk::reqwest::Client::new().post(format!("{server}/_matrix/client/v3/register")).header("Content-Type","application/json").body(json!({"username":username,"password":password,"auth":{"type":"m.login.dummy"},"inhibit_login":true}).to_string()).send().await.unwrap();
    assert!(
        response.status().is_success(),
        "registration status {}",
        response.status()
    );
    let peer = Client::builder()
        .homeserver_url(&server)
        .request_config(
            RequestConfig::default()
                .timeout(Duration::from_secs(10))
                .retry_limit(0),
        )
        .build()
        .await
        .unwrap();
    peer.matrix_auth()
        .login_username(&username, password)
        .initial_device_display_name("Archaic test peer")
        .await
        .unwrap();
    let syncing = peer.clone();
    let peer_task = tokio::spawn(async move {
        loop {
            syncing
                .sync_once(SyncSettings::default().timeout(Duration::from_secs(2)))
                .await
                .unwrap();
        }
    });
    let (sender, commands, events, mut receiver) = channels();
    let worker = tokio::spawn(archaic_matrix::run(commands, events));
    sender
        .send(Command::Login {
            epoch: 1,
            server: server.clone(),
            username: username.clone(),
            password: password.into(),
        })
        .await
        .unwrap();
    next(&mut receiver, |e| matches!(e, EventKind::Connected(_))).await;
    action(&sender, &mut receiver, Action::Refresh)
        .await
        .unwrap();
    let initial = snapshot(&mut receiver, |s| s.devices.len() == 2).await;
    assert!(!initial.identity);
    action(
        &sender,
        &mut receiver,
        Action::Bootstrap(Secret::new(password.into())),
    )
    .await
    .unwrap();
    snapshot(&mut receiver, |s| s.identity).await;
    let key = action(&sender, &mut receiver, Action::EnableRecovery)
        .await
        .unwrap()
        .expect("new recovery key");
    assert!(!key.expose().is_empty());
    assert_eq!(format!("{key:?}"), "[redacted]");
    action(
        &sender,
        &mut receiver,
        Action::VerifyDevice(peer.device_id().unwrap().to_string()),
    )
    .await
    .unwrap();
    let initiated = snapshot(&mut receiver, |s| s.flow.is_some()).await;
    let flow = initiated.flow.unwrap();
    let request = wait(|| async {
        peer.encryption()
            .get_verification_request(peer.user_id().unwrap(), &flow.id)
            .await
    })
    .await;
    request.accept().await.unwrap();
    snapshot(&mut receiver, |s| {
        s.flow
            .as_ref()
            .is_some_and(|f| f.state == "security-verification-ready")
    })
    .await;
    action(&sender, &mut receiver, Action::Start(flow.id.clone()))
        .await
        .unwrap();
    let sas = wait(|| async {
        peer.encryption()
            .get_verification(peer.user_id().unwrap(), &flow.id)
            .await
            .and_then(|v| v.sas())
    })
    .await;
    sas.accept().await.unwrap();
    let compared = snapshot(&mut receiver, |s| {
        s.flow
            .as_ref()
            .is_some_and(|f| f.state == "security-verification-compare")
    })
    .await;
    let codes = wait(|| async { sas.decimals() }).await;
    assert!(
        compared
            .flow
            .unwrap()
            .codes
            .contains(&format!("{}  {}  {}", codes.0, codes.1, codes.2))
    );
    sas.confirm().await.unwrap();
    action(&sender, &mut receiver, Action::Confirm(flow.id.clone()))
        .await
        .unwrap();
    snapshot(&mut receiver, |s| {
        s.flow
            .as_ref()
            .is_some_and(|f| f.state == "security-verification-done")
    })
    .await;
    wait(|| async { sas.is_done().then_some(()) }).await;
    assert!(matches!(request.state(), VerificationRequestState::Done));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("keys.txt");
    action(
        &sender,
        &mut receiver,
        Action::Export {
            path: path.clone(),
            passphrase: Secret::new("long-test-passphrase".into()),
        },
    )
    .await
    .unwrap();
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .starts_with("-----BEGIN MEGOLM SESSION DATA-----")
    );
    assert!(
        action(
            &sender,
            &mut receiver,
            Action::Import {
                path: path.clone(),
                passphrase: Secret::new("wrong".into())
            }
        )
        .await
        .is_err()
    );
    action(
        &sender,
        &mut receiver,
        Action::Import {
            path,
            passphrase: Secret::new("long-test-passphrase".into()),
        },
    )
    .await
    .unwrap();
    // Recovery on another device obtains private cross-signing and backup secrets without resets.
    peer.encryption()
        .recovery()
        .recover(key.expose())
        .await
        .unwrap();
    assert!(
        peer.encryption()
            .cross_signing_status()
            .await
            .unwrap()
            .has_master
    );
    assert!(peer.encryption().backups().are_enabled().await);
    assert!(
        action(&sender, &mut receiver, Action::EnableRecovery)
            .await
            .is_err()
    );
    sender
        .send(Command::NewConversation(
            archaic_matrix::NewConversation::Group {
                name: "Encrypted verification test".into(),
                topic: String::new(),
                invitees: String::new(),
            },
        ))
        .await
        .unwrap();
    let EventKind::ConversationOpened { room, .. } = next(&mut receiver, |e| {
        matches!(e, EventKind::ConversationOpened { .. })
    })
    .await
    else {
        unreachable!()
    };
    let id = matrix_sdk::ruma::RoomId::parse(&room.id).unwrap();
    let peer_room = wait(|| async { peer.get_room(&id) }).await;
    assert!(
        peer_room
            .latest_encryption_state()
            .await
            .unwrap()
            .is_encrypted()
    );
    peer_room
        .send(
            matrix_sdk::ruma::events::room::message::RoomMessageEventContent::text_plain(
                "Encrypted from the second device",
            ),
        )
        .await
        .unwrap();
    next(&mut receiver,|e|matches!(e,EventKind::History{messages,..} if messages.iter().any(|m|m.body.as_deref()==Some("Encrypted from the second device")))).await;
    sender
        .send(Command::Enqueue {
            room_id: room.id.clone(),
            body: "Encrypted queued response".into(),
            transaction: transaction_id(),
        })
        .await
        .unwrap();
    next(&mut receiver,|e|matches!(e,EventKind::History{messages,..} if messages.iter().any(|m|m.body.as_deref()==Some("Encrypted queued response")))).await;
    let page = peer_room
        .messages(matrix_sdk::room::MessagesOptions::backward())
        .await
        .unwrap();
    assert!(page.chunk.iter().any(|e| {
        e.raw()
            .get_field::<serde_json::Value>("content")
            .ok()
            .flatten()
            .is_some_and(|c| c["body"] == "Encrypted queued response")
    }));
    // Exercise the new durable media queue through actual encrypted media on Synapse.
    let upload_path = dir.path().join("encrypted-attachment.txt");
    std::fs::write(&upload_path, b"encrypted attachment bytes").unwrap();
    let upload = archaic_matrix::ToolRequest {
        room_id: room.id.clone(),
        id: transaction_id(),
        action: archaic_matrix::ToolAction::QueueUpload { path: upload_path },
    };
    sender
        .send(Command::RoomTool(upload.clone()))
        .await
        .unwrap();
    let uploaded = next(
        &mut receiver,
        |e| matches!(e,EventKind::RoomToolResult{request,..} if request.id==upload.id),
    )
    .await;
    assert!(matches!(
        uploaded,
        EventKind::RoomToolResult {
            result: Ok(archaic_matrix::ToolData::QueuedUpload),
            ..
        }
    ));
    let event_id = wait(|| async {
        let page = peer_room
            .messages(matrix_sdk::room::MessagesOptions::backward())
            .await
            .ok()?;
        page.chunk.iter().find_map(|e| {
            let content = e
                .raw()
                .get_field::<serde_json::Value>("content")
                .ok()
                .flatten()?;
            if content["body"] != "encrypted-attachment.txt" {
                return None;
            }
            assert!(
                content.get("file").is_some(),
                "media uses an encrypted file descriptor"
            );
            assert!(
                content.get("url").is_none(),
                "no plaintext media source in encrypted message"
            );
            e.raw().get_field::<String>("event_id").ok().flatten()
        })
    })
    .await;
    let saved_path = dir.path().join("decrypted-attachment.txt");
    let download = archaic_matrix::ToolRequest {
        room_id: room.id.clone(),
        id: transaction_id(),
        action: archaic_matrix::ToolAction::Download {
            event_id,
            path: saved_path.clone(),
        },
    };
    sender
        .send(Command::RoomTool(download.clone()))
        .await
        .unwrap();
    let result = next(
        &mut receiver,
        |e| matches!(e,EventKind::RoomToolResult{request,..} if request.id==download.id),
    )
    .await;
    assert!(matches!(
        result,
        EventKind::RoomToolResult {
            result: Ok(archaic_matrix::ToolData::Downloaded),
            ..
        }
    ));
    assert_eq!(
        std::fs::read(saved_path).unwrap(),
        b"encrypted attachment bytes"
    );
    action(&sender, &mut receiver, Action::RecoverRoom(room.id))
        .await
        .unwrap();

    sender.send(Command::Logout).await.unwrap();
    next(&mut receiver, |e| matches!(e, EventKind::SignedOut)).await;
    peer_task.abort();
    peer.matrix_auth().logout().await.unwrap();
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}
