#![recursion_limit = "256"]
mod protocol_conversations;
mod protocol_history;
mod protocol_membership;
mod protocol_messages;
mod protocol_reliability;
mod protocol_tools;
use archaic_matrix::{Command, Event, EventKind, Receiver, channels, transaction_id};
use serde_json::json;
use std::time::Duration;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path, path_regex},
};

async fn next(events: &mut Receiver, predicate: impl Fn(&EventKind) -> bool) -> Event {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let event = events.recv().await.expect("worker still running");
            if predicate(&event.kind) {
                return event;
            }
            if let EventKind::Error(key) = event.kind {
                panic!("unexpected worker error: {key}");
            }
        }
    })
    .await
    .expect("expected Matrix event before deadline")
}

async fn server() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/_matrix/client/versions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"versions":["v1.11"],"unstable_features":{}})),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST")).and(path_regex(r"/_matrix/client/(v3|r0)/login$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"user_id":"@alice:local","access_token":"test-access-token","device_id":"ARCHAIC_TEST"}))).mount(&server).await;
    Mock::given(method("POST"))
        .and(path_regex(r"/_matrix/client/(v3|r0)/keys/upload$"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"one_time_key_counts":{"signed_curve25519":50}})),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path_regex(r"/_matrix/client/(v3|r0)/keys/query$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"device_keys":{}})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(
            r"/_matrix/client/(v3|r0)/rooms/.*/state/m.room.encryption/?$",
        ))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_json(json!({"errcode":"M_NOT_FOUND","error":"Room is not encrypted"})),
        )
        .mount(&server)
        .await;
    let sync = json!({"next_batch":"batch-1", "rooms":{"join":{"!studio:local":{
        "state":{"events":[
            {"type":"m.room.create","state_key":"","sender":"@alice:local","event_id":"$create","origin_server_ts":1,"content":{"creator":"@alice:local","room_version":"10"}},
            {"type":"m.room.member","state_key":"@alice:local","sender":"@alice:local","event_id":"$member","origin_server_ts":2,"content":{"membership":"join","displayname":"Alice"}},
            {"type":"m.room.name","state_key":"","sender":"@alice:local","event_id":"$name","origin_server_ts":3,"content":{"name":"Studio"}}
        ]},
        "timeline":{"events":[],"limited":false,"prev_batch":"history"},
        "unread_notifications":{},"summary":{}
    }}}, "device_one_time_keys_count":{"signed_curve25519":50}});
    Mock::given(method("GET"))
        .and(path_regex(r"/_matrix/client/(v3|r0)/sync$"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(80))
                .set_body_json(sync),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET")).and(path_regex(r"/_matrix/client/(v3|r0)/rooms/.*/messages$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"start":"history", "end":"older", "chunk":[
            {"type":"m.room.message","sender":"@alice:local","event_id":"$second","origin_server_ts":20,"content":{"msgtype":"m.text","body":"Second"}},
            {"type":"m.room.message","sender":"@bob:local","event_id":"$first","origin_server_ts":10,"content":{"msgtype":"m.text","body":"First"}}
        ],"state":[]}))).mount(&server).await;
    Mock::given(method("PUT"))
        .and(path_regex(
            r"/_matrix/client/(v3|r0)/rooms/.*/send/m.room.message/.*",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"event_id":"$sent"})))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path_regex(r"/_matrix/client/(v3|r0)/logout$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;
    server
}

#[tokio::test]
async fn password_login_sync_history_send_and_logout_use_real_sdk_requests() {
    let server = server().await;
    let (sender, commands, events, mut receiver) = channels();
    let worker = tokio::spawn(archaic_matrix::run(commands, events));
    sender
        .send(Command::Login {
            epoch: 7,
            server: server.uri(),
            username: "alice".into(),
            password: "test-password".into(),
        })
        .await
        .unwrap();
    let connected = next(&mut receiver, |event| {
        matches!(event, EventKind::Connected(_))
    })
    .await;
    assert_eq!(connected.epoch, 7);
    let rooms = next(
        &mut receiver,
        |event| matches!(event, EventKind::Rooms(rooms) if !rooms.is_empty()),
    )
    .await;
    let EventKind::Rooms(rooms) = rooms.kind else {
        unreachable!()
    };
    assert_eq!(rooms[0].name, "Studio");
    sender
        .send(Command::Select(rooms[0].id.clone()))
        .await
        .unwrap();
    let history = next(&mut receiver, |event| {
        matches!(event, EventKind::History { .. })
    })
    .await;
    let EventKind::History { messages, .. } = history.kind else {
        unreachable!()
    };
    assert_eq!(
        messages
            .iter()
            .map(|message| message.body.as_deref().unwrap())
            .collect::<Vec<_>>(),
        ["First", "Second"]
    );
    let transaction = transaction_id();
    sender
        .send(Command::Send {
            room_id: rooms[0].id.clone(),
            body: "Hello Matrix".into(),
            transaction: transaction.clone(),
        })
        .await
        .unwrap();
    next(
        &mut receiver,
        |event| matches!(event, EventKind::Sent { transaction: id } if id == &transaction),
    )
    .await;
    sender.send(Command::Logout).await.unwrap();
    next(&mut receiver, |event| matches!(event, EventKind::SignedOut)).await;
    drop(sender);
    tokio::time::timeout(Duration::from_secs(2), worker)
        .await
        .unwrap()
        .unwrap();
    let requests = server.received_requests().await.unwrap();
    let send = requests
        .iter()
        .find(|request| request.method == "PUT" && request.url.path().contains("/send/"))
        .unwrap();
    assert!(send.url.path().ends_with(transaction.as_str()));
    assert_eq!(
        send.headers.get("authorization").unwrap(),
        "Bearer test-access-token"
    );
    assert_eq!(
        send.body_json::<serde_json::Value>().unwrap()["body"],
        "Hello Matrix"
    );
    let login = requests
        .iter()
        .find(|request| request.method == "POST" && request.url.path().ends_with("/login"))
        .unwrap();
    assert_eq!(
        login.body_json::<serde_json::Value>().unwrap()["password"],
        "test-password"
    );
    assert!(
        requests
            .iter()
            .any(|request| request.url.path().ends_with("/logout"))
    );
}

#[tokio::test]
async fn rejected_credentials_report_safe_error_and_allow_retry() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/_matrix/client/versions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"versions":["v1.11"]})))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path_regex(r".*/login$"))
        .respond_with(
            ResponseTemplate::new(403)
                .set_body_json(json!({"errcode":"M_FORBIDDEN", "error":"sensitive server error"})),
        )
        .mount(&server)
        .await;
    let (sender, commands, events, mut receiver) = channels();
    let worker = tokio::spawn(archaic_matrix::run(commands, events));
    for epoch in [1, 2] {
        sender
            .send(Command::Login {
                epoch,
                server: server.uri(),
                username: "alice".into(),
                password: "do-not-log".into(),
            })
            .await
            .unwrap();
        let event = tokio::time::timeout(Duration::from_secs(10), receiver.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(event.epoch, epoch);
        assert!(matches!(event.kind, EventKind::Error("login-failed")));
        assert!(!format!("{event:?}").contains("sensitive"));
    }
    drop(sender);
    worker.await.unwrap();
}

#[derive(Default)]
struct MemoryVault(std::sync::Mutex<Option<Vec<u8>>>);
impl archaic_matrix::storage::Vault for MemoryVault {
    fn load(&self) -> Result<Option<Vec<u8>>, &'static str> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn save(&self, key: &[u8]) -> Result<(), &'static str> {
        *self.0.lock().unwrap() = Some(key.to_vec());
        Ok(())
    }
}

#[tokio::test]
async fn restart_restores_the_same_device_and_logout_removes_saved_login() {
    use archaic_matrix::storage::Profile;
    let server = server().await;
    let directory = tempfile::tempdir().unwrap();
    let vault = MemoryVault::default();
    let profile = Profile::open(directory.path().into(), &vault).unwrap();
    let (sender, commands, events, mut receiver) = channels();
    let worker = tokio::spawn(archaic_matrix::run_with_profile(commands, events, profile));
    next(&mut receiver, |event| matches!(event, EventKind::SignedOut)).await;
    sender
        .send(Command::Login {
            epoch: 1,
            server: server.uri(),
            username: "alice".into(),
            password: "test-password".into(),
        })
        .await
        .unwrap();
    next(&mut receiver, |event| {
        matches!(event, EventKind::SessionSaved(true))
    })
    .await;
    next(
        &mut receiver,
        |event| matches!(event, EventKind::Rooms(rooms) if !rooms.is_empty()),
    )
    .await;
    drop(sender);
    worker.await.unwrap();
    let envelope = std::fs::read(directory.path().join("session.enc")).unwrap();
    assert!(!String::from_utf8_lossy(&envelope).contains("test-access-token"));
    let profile = Profile::open(directory.path().into(), &vault).unwrap();
    let (sender, commands, events, mut receiver) = channels();
    let worker = tokio::spawn(archaic_matrix::run_with_profile(commands, events, profile));
    let restored = next(&mut receiver, |event| {
        matches!(event, EventKind::Connected(_))
    })
    .await;
    assert_eq!(restored.epoch, 0);
    next(&mut receiver, |event| {
        matches!(event, EventKind::SessionSaved(true))
    })
    .await;
    next(
        &mut receiver,
        |event| matches!(event, EventKind::Rooms(rooms) if !rooms.is_empty()),
    )
    .await;
    sender.send(Command::Logout).await.unwrap();
    next(&mut receiver, |event| matches!(event, EventKind::SignedOut)).await;
    drop(sender);
    worker.await.unwrap();
    let profile = Profile::open(directory.path().into(), &vault).unwrap();
    let (sender, commands, events, mut receiver) = channels();
    let worker = tokio::spawn(archaic_matrix::run_with_profile(commands, events, profile));
    next(&mut receiver, |event| matches!(event, EventKind::SignedOut)).await;
    drop(sender);
    worker.await.unwrap();
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests
            .iter()
            .filter(|r| r.method == "POST" && r.url.path().ends_with("/login"))
            .count(),
        1
    );
    assert_eq!(
        requests
            .iter()
            .filter(|r| r.url.path().ends_with("/logout"))
            .count(),
        1
    );
}

#[tokio::test]
async fn corrupt_saved_login_is_preserved_and_temporary_mode_is_explicit() {
    let directory = tempfile::tempdir().unwrap();
    let profile =
        archaic_matrix::storage::Profile::open(directory.path().into(), &MemoryVault::default())
            .unwrap();
    let path = directory.path().join("session.enc");
    std::fs::write(&path, b"corrupt").unwrap();
    let (sender, commands, events, mut receiver) = channels();
    let worker = tokio::spawn(archaic_matrix::run_with_profile(commands, events, profile));
    next(&mut receiver, |event| {
        matches!(event, EventKind::RestoreBlocked("session-invalid"))
    })
    .await;
    sender.send(Command::Restore { epoch: 4 }).await.unwrap();
    assert_eq!(
        next(&mut receiver, |event| matches!(
            event,
            EventKind::RestoreBlocked(_)
        ))
        .await
        .epoch,
        4
    );
    sender.send(Command::Temporary { epoch: 5 }).await.unwrap();
    assert_eq!(
        next(&mut receiver, |event| matches!(event, EventKind::SignedOut))
            .await
            .epoch,
        5
    );
    assert_eq!(std::fs::read(path).unwrap(), b"corrupt");
    drop(sender);
    worker.await.unwrap();
}

#[tokio::test]
async fn failed_local_logout_cleanup_can_retry_without_revoking_twice() {
    let server = server().await;
    let directory = tempfile::tempdir().unwrap();
    let vault = MemoryVault::default();
    let profile = archaic_matrix::storage::Profile::open(directory.path().into(), &vault).unwrap();
    let (sender, commands, events, mut receiver) = channels();
    let worker = tokio::spawn(archaic_matrix::run_with_profile(commands, events, profile));
    next(&mut receiver, |event| matches!(event, EventKind::SignedOut)).await;
    sender
        .send(Command::Login {
            epoch: 1,
            server: server.uri(),
            username: "alice".into(),
            password: "test-password".into(),
        })
        .await
        .unwrap();
    next(&mut receiver, |event| {
        matches!(event, EventKind::SessionSaved(true))
    })
    .await;
    let record = directory.path().join("session.enc");
    let backup = directory.path().join("record-backup");
    std::fs::rename(&record, &backup).unwrap();
    std::fs::create_dir(&record).unwrap();
    sender.send(Command::Logout).await.unwrap();
    next(&mut receiver, |event| {
        matches!(event, EventKind::Error("logout-cleanup-failed"))
    })
    .await;
    std::fs::remove_dir(&record).unwrap();
    std::fs::rename(backup, record).unwrap();
    sender.send(Command::Logout).await.unwrap();
    next(&mut receiver, |event| matches!(event, EventKind::SignedOut)).await;
    drop(sender);
    worker.await.unwrap();
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests
            .iter()
            .filter(|r| r.url.path().ends_with("/logout"))
            .count(),
        1
    );
}

#[tokio::test]
async fn revoked_token_stops_sync_and_can_be_signed_out_locally() {
    let server = server().await;
    Mock::given(method("GET"))
        .and(path_regex(r".*/sync$"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_json(json!({"errcode":"M_UNKNOWN_TOKEN","error":"revoked"})),
        )
        .with_priority(1)
        .mount(&server)
        .await;
    let (sender, commands, events, mut receiver) = channels();
    let worker = tokio::spawn(archaic_matrix::run(commands, events));
    sender
        .send(Command::Login {
            epoch: 1,
            server: server.uri(),
            username: "alice".into(),
            password: "test-password".into(),
        })
        .await
        .unwrap();
    next(&mut receiver, |event| {
        matches!(event, EventKind::Error("session-expired"))
    })
    .await;
    sender.send(Command::Logout).await.unwrap();
    next(&mut receiver, |event| matches!(event, EventKind::SignedOut)).await;
    drop(sender);
    worker.await.unwrap();
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests
            .iter()
            .filter(|r| r.url.path().ends_with("/sync"))
            .count(),
        1
    );
    assert!(!requests.iter().any(|r| r.url.path().ends_with("/logout")));
}

#[tokio::test]
async fn joining_validates_retries_and_opens_the_resolved_room() {
    let server = server().await;
    Mock::given(method("GET"))
        .and(path_regex(r"/_matrix/client/(v3|r0)/directory/room/.*"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"room_id":"!new:local", "servers":["local"]})),
        )
        .mount(&server)
        .await;
    let denied = Mock::given(method("POST"))
        .and(path_regex(r"/_matrix/client/(v3|r0)/join/.*"))
        .respond_with(
            ResponseTemplate::new(403)
                .set_body_json(json!({"errcode":"M_FORBIDDEN","error":"Private room"})),
        )
        .mount_as_scoped(&server)
        .await;
    let (sender, commands, events, mut receiver) = channels();
    let worker = tokio::spawn(archaic_matrix::run(commands, events));
    sender
        .send(Command::Login {
            epoch: 9,
            server: server.uri(),
            username: "alice".into(),
            password: "test-password".into(),
        })
        .await
        .unwrap();
    next(&mut receiver, |event| matches!(event, EventKind::Rooms(_))).await;
    sender
        .send(Command::Join("not a Matrix room".into()))
        .await
        .unwrap();
    next(&mut receiver, |event| {
        matches!(event, EventKind::JoinFailed("join-invalid"))
    })
    .await;
    assert!(
        !server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .any(|request| request.url.path().contains("/join/"))
    );
    sender
        .send(Command::Join("#new:local".into()))
        .await
        .unwrap();
    next(&mut receiver, |event| {
        matches!(event, EventKind::JoinFailed("join-forbidden"))
    })
    .await;
    drop(denied);
    Mock::given(method("POST"))
        .and(path_regex(r"/_matrix/client/(v3|r0)/join/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"room_id":"!new:local"})))
        .mount(&server)
        .await;
    for address in ["  #new:local  ", "!new:local"] {
        sender.send(Command::Join(address.into())).await.unwrap();
        let joined = next(
            &mut receiver,
            |event| matches!(event, EventKind::Joined(room) if room.id == "!new:local"),
        )
        .await;
        assert_eq!(joined.epoch, 9);
        next(&mut receiver, |event| matches!(event, EventKind::Rooms(rooms) if rooms.iter().any(|room| room.id == "!new:local"))).await;
        next(
            &mut receiver,
            |event| matches!(event, EventKind::History { room_id, .. } if room_id == "!new:local"),
        )
        .await;
    }
    let requests = server.received_requests().await.unwrap();
    let joins: Vec<_> = requests
        .iter()
        .filter(|request| request.url.path().contains("/join/"))
        .collect();
    assert_eq!(joins.len(), 3);
    assert!(
        joins
            .iter()
            .all(|request| request.headers.get("authorization").unwrap()
                == "Bearer test-access-token")
    );
    worker.abort();
}

#[tokio::test]
async fn cached_rooms_and_recent_history_restore_before_network_sync() {
    use archaic_matrix::storage::Profile;
    let server = server().await;
    let directory = tempfile::tempdir().unwrap();
    let vault = MemoryVault::default();
    let profile = Profile::open(directory.path().into(), &vault).unwrap();
    let (sender, commands, events, mut receiver) = channels();
    let worker = tokio::spawn(archaic_matrix::run_with_profile(commands, events, profile));
    next(&mut receiver, |e| matches!(e, EventKind::SignedOut)).await;
    sender
        .send(Command::Login {
            epoch: 1,
            server: server.uri(),
            username: "alice".into(),
            password: "test-password".into(),
        })
        .await
        .unwrap();
    next(
        &mut receiver,
        |e| matches!(e,EventKind::Rooms(r) if !r.is_empty()),
    )
    .await;
    sender
        .send(Command::Select("!studio:local".into()))
        .await
        .unwrap();
    let history = next(
        &mut receiver,
        |e| matches!(e,EventKind::History {messages,..} if !messages.is_empty()),
    )
    .await;
    let EventKind::History {
        messages: before, ..
    } = history.kind
    else {
        unreachable!()
    };
    drop(sender);
    worker.await.unwrap();
    // A stalled network must not gate restoration or a cached viewport.
    server.reset().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503).set_delay(Duration::from_secs(30)))
        .mount(&server)
        .await;
    let profile = Profile::open(directory.path().into(), &vault).unwrap();
    let (sender, commands, events, mut receiver) = channels();
    let worker = tokio::spawn(archaic_matrix::run_with_profile(commands, events, profile));
    tokio::time::timeout(Duration::from_secs(10), async {
        next(&mut receiver, |e| matches!(e, EventKind::Connected(_))).await;
        next(
            &mut receiver,
            |e| matches!(e,EventKind::Rooms(r) if !r.is_empty()),
        )
        .await;
        sender
            .send(Command::Select("!studio:local".into()))
            .await
            .unwrap();
        let event = next(&mut receiver, |e| matches!(e, EventKind::History { .. })).await;
        let EventKind::History { messages, .. } = event.kind else {
            unreachable!()
        };
        assert_eq!(messages, before);
    })
    .await
    .expect("cached UI must not wait for the stalled network request");
    drop(sender);
    worker.await.unwrap();
}

#[tokio::test]
async fn account_and_room_display_names_arrive_without_avatar_downloads() {
    let server = server().await;
    Mock::given(method("GET"))
        .and(path_regex(
            r"/_matrix/client/(v3|r0)/profile/.*/displayname$",
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"displayname":"Alice Example"})),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(r"/_matrix/client/(v3|r0)/rooms/.*/members$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"chunk":[
            {"type":"m.room.member","state_key":"@alice:local","sender":"@alice:local","event_id":"$member","origin_server_ts":2,"content":{"membership":"join","displayname":"Alice"}}
        ]}))).mount(&server).await;
    let (sender, commands, events, mut receiver) = channels();
    let worker = tokio::spawn(archaic_matrix::run(commands, events));
    sender
        .send(Command::Login {
            epoch: 1,
            server: server.uri(),
            username: "alice".into(),
            password: "test".into(),
        })
        .await
        .unwrap();
    next(&mut receiver, |e| matches!(e, EventKind::DisplayName { key, name } if key == "account" && name == "Alice Example")).await;
    sender
        .send(Command::Select("!studio:local".into()))
        .await
        .unwrap();
    next(&mut receiver, |e| matches!(e, EventKind::DisplayName { key, name } if key == "user:!studio:local:@alice:local" && name == "Alice")).await;
    drop(sender);
    tokio::time::timeout(Duration::from_secs(2), worker)
        .await
        .unwrap()
        .unwrap();
}
