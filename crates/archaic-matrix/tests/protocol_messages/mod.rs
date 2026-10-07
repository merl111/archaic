use super::{next, server};
use archaic_matrix::{
    Command, EventKind, MessageAction, MessageRequest, Receiver, Sender, channels, transaction_id,
};
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
            epoch: 1,
            server: server.uri(),
            username: "alice".into(),
            password: "test".into(),
        })
        .await
        .unwrap();
    next(
        &mut receiver,
        |e| matches!(e,EventKind::Rooms(r) if !r.is_empty()),
    )
    .await;
    (sender, receiver, worker)
}
fn request(id: &str, action: MessageAction) -> MessageRequest {
    MessageRequest {
        room_id: "!studio:local".into(),
        event_id: id.into(),
        action,
        transaction: transaction_id(),
    }
}
async fn perform(
    sender: &Sender,
    receiver: &mut Receiver,
    request: &MessageRequest,
) -> Result<(), &'static str> {
    sender
        .send(Command::MessageAction(request.clone()))
        .await
        .unwrap();
    let event = next(
        receiver,
        |e| matches!(e,EventKind::MessageActionResult{request:r,..} if r==request),
    )
    .await;
    let EventKind::MessageActionResult { result, .. } = event.kind else {
        unreachable!()
    };
    result
}
async fn target(server: &MockServer, id: &str, sender: &str, kind: &str, content: Value) {
    Mock::given(method("GET")).and(path_regex(format!(".*/event/.*{}$",&id[1..]))).respond_with(ResponseTemplate::new(200).set_body_json(json!({"event_id":id,"room_id":"!studio:local","sender":sender,"type":kind,"content":content,"origin_server_ts":10}))).mount(server).await;
}
#[tokio::test]
async fn replies_edits_reactions_and_redactions_use_sdk_payloads_and_stable_retry_ids() {
    let server = server().await;
    target(
        &server,
        "$own",
        "@alice:local",
        "m.room.message",
        json!({"msgtype":"m.text","body":"Original"}),
    )
    .await;
    target(
        &server,
        "$reaction",
        "@alice:local",
        "m.reaction",
        json!({"m.relates_to":{"rel_type":"m.annotation","event_id":"$own","key":"👍"}}),
    )
    .await;
    for pattern in [".*/send/m.reaction/.*", ".*/redact/.*"] {
        Mock::given(method("PUT"))
            .and(path_regex(pattern))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"event_id":"$result"})))
            .mount(&server)
            .await;
    }
    Mock::given(method("PUT"))
        .and(path_regex(".*/send/m.room.message/.*"))
        .respond_with(
            ResponseTemplate::new(503)
                .set_body_json(json!({"errcode":"M_UNKNOWN","error":"retry"})),
        )
        .with_priority(1)
        .up_to_n_times(1)
        .mount(&server)
        .await;
    let (sender, mut receiver, worker) = login(&server).await;
    let reply = request("$own", MessageAction::Reply("Hello reply".into()));
    assert_eq!(
        perform(&sender, &mut receiver, &reply).await,
        Err("message-action-failed")
    );
    assert_eq!(perform(&sender, &mut receiver, &reply).await, Ok(()));
    for action in [
        MessageAction::Edit("Edited text".into()),
        MessageAction::React("👍".into()),
        MessageAction::RemoveReactions {
            key: "👍".into(),
            ids: vec!["$reaction".into()],
        },
        MessageAction::Delete,
    ] {
        assert_eq!(
            perform(&sender, &mut receiver, &request("$own", action)).await,
            Ok(())
        );
    }
    let requests = server.received_requests().await.unwrap();
    let writes: Vec<_> = requests.iter().filter(|r| r.method == "PUT").collect();
    assert_eq!(writes.len(), 6);
    assert_eq!(writes[0].url.path(), writes[1].url.path());
    for write in &writes {
        assert_eq!(
            write.headers.get("authorization").unwrap(),
            "Bearer test-access-token"
        );
    }
    let body: Value = writes[1].body_json().unwrap();
    assert_eq!(body["m.relates_to"]["m.in_reply_to"]["event_id"], "$own");
    let body: Value = writes[2].body_json().unwrap();
    assert_eq!(body["m.relates_to"]["rel_type"], "m.replace");
    assert_eq!(body["m.new_content"]["body"], "Edited text");
    let body: Value = writes[3].body_json().unwrap();
    assert_eq!(body["m.relates_to"]["key"], "👍");
    assert!(writes[4].url.path().contains("/redact/"));
    assert!(writes[5].url.path().contains("/redact/"));
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}
#[tokio::test]
async fn author_type_target_and_input_guards_prevent_unauthorized_writes() {
    let server = server().await;
    target(
        &server,
        "$foreign",
        "@bob:local",
        "m.room.message",
        json!({"msgtype":"m.text","body":"Other"}),
    )
    .await;
    target(
        &server,
        "$own",
        "@alice:local",
        "m.room.message",
        json!({"msgtype":"m.text","body":"Own"}),
    )
    .await;
    target(
        &server,
        "$image",
        "@alice:local",
        "m.room.message",
        json!({"msgtype":"m.image","body":"Photo","url":"mxc://local/photo"}),
    )
    .await;
    target(
        &server,
        "$reaction",
        "@bob:local",
        "m.reaction",
        json!({"m.relates_to":{"rel_type":"m.annotation","event_id":"$own","key":"👍"}}),
    )
    .await;
    let (sender, mut receiver, worker) = login(&server).await;
    for (id, action, error) in [
        (
            "$foreign",
            MessageAction::Edit("forged".into()),
            "message-forbidden",
        ),
        ("$foreign", MessageAction::Delete, "message-forbidden"),
        (
            "$image",
            MessageAction::Edit("caption".into()),
            "message-forbidden",
        ),
        (
            "$own",
            MessageAction::Reply(" ".into()),
            "message-body-invalid",
        ),
        (
            "$own",
            MessageAction::React("bad\nkey".into()),
            "reaction-invalid",
        ),
        (
            "$own",
            MessageAction::RemoveReactions {
                key: "👍".into(),
                ids: vec!["$reaction".into()],
            },
            "message-forbidden",
        ),
    ] {
        assert_eq!(
            perform(&sender, &mut receiver, &request(id, action)).await,
            Err(error)
        );
    }
    assert!(
        !server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .any(|r| r.method == "PUT")
    );
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}

#[tokio::test]
async fn already_redacted_own_message_accepts_same_delete_transaction_after_lost_ack() {
    let server = server().await;
    Mock::given(method("GET")).and(path_regex(".*/event/.*own$")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"event_id":"$own","room_id":"!studio:local","sender":"@alice:local","type":"m.room.message","content":{},"origin_server_ts":10,"unsigned":{"redacted_because":{"type":"m.room.redaction","event_id":"$redact","sender":"@alice:local","origin_server_ts":20,"content":{},"redacts":"$own"}}}))).mount(&server).await;
    Mock::given(method("PUT"))
        .and(path_regex(".*/redact/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"event_id":"$redact"})))
        .expect(2)
        .mount(&server)
        .await;
    let (sender, mut receiver, worker) = login(&server).await;
    let delete = request("$own", MessageAction::Delete);
    assert_eq!(perform(&sender, &mut receiver, &delete).await, Ok(()));
    assert_eq!(perform(&sender, &mut receiver, &delete).await, Ok(()));
    let paths: Vec<_> = server
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .filter(|r| r.method == "PUT")
        .map(|r| r.url.path().to_owned())
        .collect();
    assert_eq!(paths[0], paths[1]);
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}
#[tokio::test]
async fn expired_action_reports_expiry_and_stops_further_mutations() {
    let server = server().await;
    target(
        &server,
        "$own",
        "@alice:local",
        "m.room.message",
        json!({"msgtype":"m.text","body":"Own"}),
    )
    .await;
    Mock::given(method("PUT"))
        .and(path_regex(".*/send/m.reaction/.*"))
        .respond_with(ResponseTemplate::new(401).set_body_json(
            json!({"errcode":"M_UNKNOWN_TOKEN","error":"Expired","soft_logout":false}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let (sender, mut receiver, worker) = login(&server).await;
    let action = request("$own", MessageAction::React("like".into()));
    assert_eq!(
        perform(&sender, &mut receiver, &action).await,
        Err("session-expired")
    );
    next(&mut receiver, |e| {
        matches!(e, EventKind::Error("session-expired"))
    })
    .await;
    sender.send(Command::MessageAction(action)).await.unwrap();
    sender.send(Command::Logout).await.unwrap();
    next(&mut receiver, |e| matches!(e, EventKind::SignedOut)).await;
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}

#[tokio::test]
async fn reaction_removal_validates_all_targets_before_writing_and_retries_each_same_id() {
    let server = server().await;
    target(
        &server,
        "$own",
        "@alice:local",
        "m.room.message",
        json!({"msgtype":"m.text","body":"Own"}),
    )
    .await;
    for (id, owner) in [
        ("$r1", "@alice:local"),
        ("$r2", "@alice:local"),
        ("$foreign", "@bob:local"),
    ] {
        target(
            &server,
            id,
            owner,
            "m.reaction",
            json!({"m.relates_to":{"rel_type":"m.annotation","event_id":"$own","key":"like"}}),
        )
        .await;
    }
    Mock::given(method("PUT"))
        .and(path_regex(".*/redact/.*r2/.*"))
        .respond_with(
            ResponseTemplate::new(503)
                .set_body_json(json!({"errcode":"M_UNKNOWN","error":"retry"})),
        )
        .with_priority(1)
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path_regex(".*/redact/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"event_id":"$redact"})))
        .mount(&server)
        .await;
    let (sender, mut receiver, worker) = login(&server).await;
    let invalid = request(
        "$own",
        MessageAction::RemoveReactions {
            key: "like".into(),
            ids: vec!["$r1".into(), "$foreign".into()],
        },
    );
    assert_eq!(
        perform(&sender, &mut receiver, &invalid).await,
        Err("message-forbidden")
    );
    assert!(
        !server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .any(|r| r.method == "PUT")
    );
    let action = request(
        "$own",
        MessageAction::RemoveReactions {
            key: "like".into(),
            ids: vec!["$r1".into(), "$r2".into()],
        },
    );
    assert_eq!(
        perform(&sender, &mut receiver, &action).await,
        Err("message-action-failed")
    );
    assert_eq!(perform(&sender, &mut receiver, &action).await, Ok(()));
    let paths: Vec<_> = server
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .filter(|r| r.method == "PUT")
        .map(|r| r.url.path().to_owned())
        .collect();
    assert_eq!(paths.len(), 4);
    assert_eq!(paths[0], paths[2]);
    assert_eq!(paths[1], paths[3]);
    assert_ne!(paths[0], paths[1]);
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}
#[tokio::test]
async fn mismatched_room_in_fetched_event_is_not_mutated() {
    let server = server().await;
    Mock::given(method("GET")).and(path_regex(".*/event/.*own$")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"event_id":"$own","room_id":"!different:local","sender":"@alice:local","type":"m.room.message","content":{"msgtype":"m.text","body":"Wrong room"},"origin_server_ts":10}))).mount(&server).await;
    let (sender, mut receiver, worker) = login(&server).await;
    assert_eq!(
        perform(
            &sender,
            &mut receiver,
            &request("$own", MessageAction::Delete)
        )
        .await,
        Err("message-unavailable")
    );
    assert!(
        !server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .any(|r| r.method == "PUT")
    );
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}

#[tokio::test]
async fn thread_reply_and_text_forward_use_sdk_relations_and_target_guards() {
    let server = server().await;
    target(
        &server,
        "$root",
        "@bob:local",
        "m.room.message",
        json!({"msgtype":"m.text","body":"Original"}),
    )
    .await;
    let (sender, mut receiver, worker) = login(&server).await;
    let thread = request("$root", MessageAction::Thread("Thread response".into()));
    assert_eq!(perform(&sender, &mut receiver, &thread).await, Ok(()));
    let forward = request("$root", MessageAction::Forward("!studio:local".into()));
    assert_eq!(perform(&sender, &mut receiver, &forward).await, Ok(()));
    let invalid = request("$root", MessageAction::Forward("!notjoined:local".into()));
    assert_eq!(
        perform(&sender, &mut receiver, &invalid).await,
        Err("room-unavailable")
    );
    let requests = server.received_requests().await.unwrap();
    let sent = requests
        .iter()
        .filter(|r| r.method == "PUT" && r.url.path().contains("/send/"))
        .collect::<Vec<_>>();
    assert_eq!(sent.len(), 2);
    let content = sent[0].body_json::<Value>().unwrap();
    assert_eq!(content["m.relates_to"]["rel_type"], "m.thread");
    assert_eq!(content["m.relates_to"]["event_id"], "$root");
    assert_eq!(content["body"], "Thread response");
    let content = sent[1].body_json::<Value>().unwrap();
    assert_eq!(content["body"], "@bob:local:\nOriginal");
    assert!(content.get("m.relates_to").is_none());
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}
