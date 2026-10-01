use super::{next, server};
use archaic_matrix::{Command, EventKind, NewConversation, Receiver, Sender, channels};
use serde_json::{Value, json};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path_regex},
};

async fn login(server: &MockServer) -> (Sender, Receiver, tokio::task::JoinHandle<()>) {
    let (sender, commands, events, mut receiver) = channels();
    let worker = tokio::spawn(archaic_matrix::run(commands, events));
    sender
        .send(Command::Login {
            epoch: 3,
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
    (sender, receiver, worker)
}
async fn open(sender: &Sender, receiver: &mut Receiver, input: NewConversation) -> EventKind {
    sender.send(Command::NewConversation(input)).await.unwrap();
    next(receiver, |e| {
        matches!(
            e,
            EventKind::ConversationOpened { .. } | EventKind::CreateFailed(_)
        )
    })
    .await
    .kind
}
fn group() -> NewConversation {
    NewConversation::Group {
        name: "  Native team  ".into(),
        topic: "Roadmap".into(),
        invitees: " @bob:local, @bob:local; @carol:local ".into(),
    }
}
fn opened(event: EventKind, expected: &str, expected_warning: Option<&str>) {
    let EventKind::ConversationOpened { room, warning } = event else {
        panic!("expected opened room")
    };
    assert_eq!(room.id, expected);
    assert_eq!(room.membership, archaic_matrix::Membership::Joined);
    assert_eq!(warning, expected_warning);
}
async fn creations(server: &MockServer) -> Vec<Value> {
    server
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .filter(|req| req.url.path().ends_with("/createRoom"))
        .map(|req| {
            assert_eq!(
                req.headers.get("authorization").unwrap(),
                "Bearer test-access-token"
            );
            req.body_json().unwrap()
        })
        .collect()
}
fn encrypted_private(body: &Value) {
    assert_eq!(body["preset"], "private_chat");
    assert_eq!(
        body.get("visibility")
            .and_then(Value::as_str)
            .unwrap_or("private"),
        "private"
    );
    assert_eq!(body["initial_state"][0]["type"], "m.room.encryption");
    assert_eq!(
        body["initial_state"][0]["content"]["algorithm"],
        "m.megolm.v1.aes-sha2"
    );
    assert!(body.get("room_alias_name").is_none());
}
#[tokio::test]
async fn group_creation_validates_retries_explicitly_and_sends_private_encrypted_payload() {
    let server = server().await;
    Mock::given(method("POST"))
        .and(path_regex(".*/createRoom$"))
        .respond_with(
            ResponseTemplate::new(403)
                .set_body_json(json!({"errcode":"M_FORBIDDEN","error":"Denied"})),
        )
        .with_priority(1)
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path_regex(".*/createRoom$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"room_id":"!group:local"})))
        .mount(&server)
        .await;
    let (sender, mut receiver, worker) = login(&server).await;
    assert!(matches!(
        open(
            &sender,
            &mut receiver,
            NewConversation::Direct("invalid".into())
        )
        .await,
        EventKind::CreateFailed("create-invalid-user")
    ));
    assert!(creations(&server).await.is_empty());
    assert!(matches!(
        open(&sender, &mut receiver, group()).await,
        EventKind::CreateFailed("create-forbidden")
    ));
    assert_eq!(creations(&server).await.len(), 1);
    opened(
        open(&sender, &mut receiver, group()).await,
        "!group:local",
        None,
    );
    next(
        &mut receiver,
        |e| matches!(e,EventKind::History{room_id,..} if room_id=="!group:local"),
    )
    .await;
    let requests = creations(&server).await;
    assert_eq!(requests.len(), 2);
    let body = &requests[1];
    encrypted_private(body);
    assert_eq!(body["name"], "Native team");
    assert_eq!(body["topic"], "Roadmap");
    assert_eq!(body["invite"], json!(["@bob:local", "@carol:local"]));
    assert!(
        !body
            .get("is_direct")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    );
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}
#[tokio::test]
async fn failed_direct_label_keeps_created_room_and_reopening_does_not_duplicate_it() {
    let server = server().await;
    Mock::given(method("POST"))
        .and(path_regex(".*/createRoom$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"room_id":"!dm:local"})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(".*/account_data/m.direct$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path_regex(".*/account_data/m.direct$"))
        .respond_with(
            ResponseTemplate::new(503)
                .set_body_json(json!({"errcode":"M_UNKNOWN","error":"Unavailable"})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let (sender, mut receiver, worker) = login(&server).await;
    for _ in 0..2 {
        opened(
            open(
                &sender,
                &mut receiver,
                NewConversation::Direct(" @bob:local ".into()),
            )
            .await,
            "!dm:local",
            Some("create-direct-tag-failed"),
        );
    }
    let requests = creations(&server).await;
    assert_eq!(requests.len(), 1);
    encrypted_private(&requests[0]);
    assert_eq!(requests[0]["is_direct"], true);
    assert_eq!(requests[0]["invite"], json!(["@bob:local"]));
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}
#[tokio::test]
async fn an_existing_joined_direct_room_is_opened_without_creation() {
    let server = server().await;
    Mock::given(method("GET")).and(path_regex(".*/sync$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "next_batch":"dm-sync", "account_data":{"events":[{"type":"m.direct","content":{"@bob:local":["!studio:local"]}}]},
            "rooms":{"join":{"!studio:local":{"state":{"events":[
                {"type":"m.room.create","state_key":"","sender":"@alice:local","event_id":"$c","origin_server_ts":1,"content":{"creator":"@alice:local","room_version":"10"}},
                {"type":"m.room.member","state_key":"@alice:local","sender":"@alice:local","event_id":"$a","origin_server_ts":2,"content":{"membership":"join"}},
                {"type":"m.room.member","state_key":"@bob:local","sender":"@alice:local","event_id":"$b","origin_server_ts":3,"content":{"membership":"invite"}}
            ]},"timeline":{"events":[],"limited":false,"prev_batch":"old"}}}},
            "device_one_time_keys_count":{"signed_curve25519":50}
        }))).with_priority(1).mount(&server).await;
    let (sender, mut receiver, worker) = login(&server).await;
    opened(
        open(
            &sender,
            &mut receiver,
            NewConversation::Direct("@bob:local".into()),
        )
        .await,
        "!studio:local",
        None,
    );
    assert!(creations(&server).await.is_empty());
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}
