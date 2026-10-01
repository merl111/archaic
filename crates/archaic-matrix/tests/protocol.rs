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
            if let EventKind::Error(key) = event.kind {
                panic!("unexpected worker error: {key}");
            }
            if predicate(&event.kind) {
                return event;
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
