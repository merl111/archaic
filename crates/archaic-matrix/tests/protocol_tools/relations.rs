use super::*;
fn details(more: bool) -> ToolRequest {
    request(ToolAction::Details {
        event_id: "$original".into(),
        more,
    })
}
fn data(result: Result<ToolData, &'static str>) -> Box<archaic_matrix::MessageDetails> {
    let Ok(ToolData::Details(d)) = result else {
        panic!("expected details")
    };
    d
}
#[tokio::test]
async fn details_resolve_old_reply_and_all_relation_pages_with_atomic_failure_retry() {
    let server = server().await;
    target(&server,event("$original","m.room.message",json!({"msgtype":"m.text","body":"Original","m.relates_to":{"m.in_reply_to":{"event_id":"$oldreply"}}}),10)).await;
    target(
        &server,
        event(
            "$oldreply",
            "m.room.message",
            json!({"msgtype":"m.text","body":"Outside loaded history"}),
            1,
        ),
    )
    .await;
    let edit = event(
        "$edit",
        "m.room.message",
        json!({"msgtype":"m.text","body":"* Edited","m.new_content":{"msgtype":"m.text","body":"Edited"},"m.relates_to":{"rel_type":"m.replace","event_id":"$original"}}),
        20,
    );
    let reaction = event(
        "$reaction",
        "m.reaction",
        json!({"m.relates_to":{"rel_type":"m.annotation","event_id":"$original","key":"like"}}),
        30,
    );
    Mock::given(method("GET"))
        .and(path_regex(".*/relations/.*original$"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"chunk":[reaction],"next_batch":"r2"})),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(".*/relations/.*original$"))
        .and(query_param("from", "r2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"chunk":[edit]})))
        .with_priority(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(".*/relations/.*original$"))
        .and(query_param("from", "r2"))
        .respond_with(
            ResponseTemplate::new(503)
                .set_body_json(json!({"errcode":"M_UNKNOWN","error":"retry"})),
        )
        .with_priority(1)
        .up_to_n_times(1)
        .mount(&server)
        .await;
    let (sender, mut receiver, worker) = login(&server).await;
    let d = data(perform(&sender, &mut receiver, &details(false)).await);
    assert!(d.more);
    assert_eq!(d.loaded, 1);
    assert_eq!(d.message.reactions.len(), 1);
    assert_eq!(
        d.reply.unwrap().body.as_deref(),
        Some("Outside loaded history")
    );
    let more = details(true);
    assert_error(
        perform(&sender, &mut receiver, &more).await,
        "message-action-failed",
    );
    let d = data(perform(&sender, &mut receiver, &more).await);
    assert!(!d.more);
    assert_eq!(d.loaded, 2);
    assert_eq!(d.message.body.as_deref(), Some("Edited"));
    assert_eq!(d.message.reactions.len(), 1);
    assert_eq!(d.revisions.len(), 1);
    let requests = server.received_requests().await.unwrap();
    let retries: Vec<_> = requests
        .iter()
        .filter(|r| {
            r.url.path().contains("/relations/")
                && r.url.query_pairs().any(|(k, v)| k == "from" && v == "r2")
        })
        .collect();
    assert_eq!(retries.len(), 2);
    assert_eq!(retries[0].url, retries[1].url);
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}
#[tokio::test]
async fn relations_refresh_drops_stale_reactions_and_handles_missing_reply() {
    let server = server().await;
    target(&server,event("$original","m.room.message",json!({"msgtype":"m.text","body":"Original","m.relates_to":{"m.in_reply_to":{"event_id":"$missing"}}}),10)).await;
    Mock::given(method("GET"))
        .and(path_regex(".*/event/.*missing$"))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_json(json!({"errcode":"M_NOT_FOUND","error":"gone"})),
        )
        .mount(&server)
        .await;
    let reaction = event(
        "$reaction",
        "m.reaction",
        json!({"m.relates_to":{"rel_type":"m.annotation","event_id":"$original","key":"like"}}),
        20,
    );
    Mock::given(method("GET"))
        .and(path_regex(".*/relations/.*original$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"chunk":[reaction]})))
        .with_priority(1)
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(".*/relations/.*original$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"chunk":[]})))
        .mount(&server)
        .await;
    let (sender, mut receiver, worker) = login(&server).await;
    let d = data(perform(&sender, &mut receiver, &details(false)).await);
    assert!(d.reply_unavailable);
    assert_eq!(d.message.reactions.len(), 1);
    let d = data(perform(&sender, &mut receiver, &details(false)).await);
    assert!(d.message.reactions.is_empty());
    assert!(!d.more);
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}

#[tokio::test]
async fn details_show_only_public_receipts_anchored_on_selected_event() {
    let server = server().await;
    let state = vec![
        json!({"type":"m.room.create","state_key":"","sender":"@alice:local","event_id":"$create","origin_server_ts":1,"content":{"creator":"@alice:local","room_version":"10"}}),
        json!({"type":"m.room.member","state_key":"@alice:local","sender":"@alice:local","event_id":"$member","origin_server_ts":2,"content":{"membership":"join"}}),
    ];
    Mock::given(method("GET")).and(path_regex(".*/sync$")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"next_batch":"r1","rooms":{"join":{"!studio:local":{"state":{"events":state},"timeline":{"events":[],"limited":false,"prev_batch":"history"},"ephemeral":{"events":[{"type":"m.receipt","content":{"$original":{"m.read":{"@bob:local":{"ts":40}},"m.read.private":{"@alice:local":{"ts":41}}},"$other":{"m.read":{"@carol:local":{"ts":42}}}}}]}}}},"device_one_time_keys_count":{"signed_curve25519":50}}))).with_priority(1).mount(&server).await;
    target(
        &server,
        event(
            "$original",
            "m.room.message",
            json!({"msgtype":"m.text","body":"Selected"}),
            10,
        ),
    )
    .await;
    Mock::given(method("GET"))
        .and(path_regex(".*/relations/.*original$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"chunk":[]})))
        .mount(&server)
        .await;
    let (sender, mut receiver, worker) = login(&server).await;
    let d = data(perform(&sender, &mut receiver, &details(false)).await);
    assert_eq!(d.read_by, vec!["@bob:local"]);
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}

#[tokio::test]
async fn open_details_refresh_automatically_when_sync_adds_relations_and_thread_receipts() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::time::Duration;
    let server = server().await;
    let changed = Arc::new(AtomicBool::new(false));
    let state = changed.clone();
    let root = event(
        "$original",
        "m.room.message",
        json!({"msgtype":"m.text","body":"Before"}),
        10,
    );
    target(&server, root.clone()).await;
    Mock::given(method("GET"))
        .and(path_regex(".*/relations/.*original$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"chunk":[]})))
        .mount(&server)
        .await;
    Mock::given(method("GET")).and(path_regex(".*/sync$")).respond_with(move|_:&wiremock::Request| {
        let updated=state.load(Ordering::Acquire);
        let events=if updated {vec![
            event("$edit","m.room.message",json!({"msgtype":"m.text","body":"* After","m.relates_to":{"rel_type":"m.replace","event_id":"$original"},"m.new_content":{"msgtype":"m.text","body":"After"}}),20),
            event("$thread","m.room.message",json!({"msgtype":"m.text","body":"Thread reply","m.relates_to":{"rel_type":"m.thread","event_id":"$original","is_falling_back":true,"m.in_reply_to":{"event_id":"$original"}}}),21)
        ]}else{vec![root.clone()]};
        let receipts=if updated {vec![json!({"type":"m.receipt","content":{"$original":{"m.read":{"@bob:local":{"ts":40,"thread_id":"$original"},"@carol:local":{"ts":41}}}}})]}else{vec![]};
        ResponseTemplate::new(200).set_delay(Duration::from_millis(80)).set_body_json(json!({"next_batch":if updated{"changed"}else{"initial"},"rooms":{"join":{"!studio:local":{"state":{"events":[
            {"type":"m.room.create","event_id":"$create","state_key":"","sender":"@alice:local","origin_server_ts":1,"content":{"creator":"@alice:local","room_version":"10"}},
            {"type":"m.room.member","event_id":"$member","state_key":"@alice:local","sender":"@alice:local","origin_server_ts":2,"content":{"membership":"join"}}
        ]},"timeline":{"events":events,"limited":false,"prev_batch":"history"},"ephemeral":{"events":receipts}}}},"device_one_time_keys_count":{"signed_curve25519":50}}))
    }).with_priority(1).mount(&server).await;
    let (sender, mut receiver, worker) = login(&server).await;
    sender
        .send(Command::Select("!studio:local".into()))
        .await
        .unwrap();
    next(&mut receiver, |e| matches!(e, EventKind::History { .. })).await;
    let initial = data(perform(&sender, &mut receiver, &details(false)).await);
    assert_eq!(initial.message.body.as_deref(), Some("Before"));
    changed.store(true, Ordering::Release);
    let EventKind::LiveDetails { details, .. } = next(
        &mut receiver,
        |e| matches!(e,EventKind::LiveDetails{details,..} if details.thread.len()==1),
    )
    .await
    .kind
    else {
        unreachable!()
    };
    assert_eq!(details.message.body.as_deref(), Some("After"));
    assert_eq!(details.thread_read_by, vec!["@bob:local"]);
    assert!(
        details
            .message
            .thread_read_by
            .contains(&"@bob:local".to_owned())
    );
    assert_eq!(details.message.read_by, vec!["@carol:local"]);
    assert_eq!(details.read_by, vec!["@carol:local"]);
    assert_eq!(
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .filter(|r| r.url.path().contains("/relations/"))
            .count(),
        1,
        "live update needs no relation refetch without a gap"
    );
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}

#[tokio::test]
async fn timeline_receipts_move_without_new_messages_and_keep_threads_private() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::time::Duration;
    let server = server().await;
    let changed = Arc::new(AtomicBool::new(false));
    let state = changed.clone();
    let messages = vec![
        event(
            "$one",
            "m.room.message",
            json!({"msgtype":"m.text","body":"One"}),
            10,
        ),
        event(
            "$two",
            "m.room.message",
            json!({"msgtype":"m.text","body":"Two"}),
            20,
        ),
        event(
            "$hidden",
            "m.reaction",
            json!({"m.relates_to":{"rel_type":"m.annotation","event_id":"$one","key":"👍"}}),
            25,
        ),
        event(
            "$reply",
            "m.room.message",
            json!({"msgtype":"m.text","body":"Thread","m.relates_to":{"rel_type":"m.thread","event_id":"$one"}}),
            30,
        ),
    ];
    Mock::given(method("GET"))
        .and(path_regex(".*/messages$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"start":"history","chunk":messages.into_iter().rev().collect::<Vec<_>>()}),
        ))
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("GET")).and(path_regex(".*/sync$")).respond_with(move|_: &wiremock::Request| {
        let updated=state.load(Ordering::Acquire);
        let target=if updated {"$hidden"}else{"$one"};
        let mut receipts=json!({target:{"m.read":{"@bob:local":{"ts":if updated {50}else{40},"thread_id":if updated {serde_json::Value::Null}else{json!("main")}},"@alice:local":{"ts":40}},"m.read.private":{"@private:local":{"ts":40}}},"$reply":{"m.read":{"@carol:local":{"ts":41,"thread_id":"$one"}}}});
        if updated { receipts[target]["m.read"]["@bob:local"].as_object_mut().unwrap().remove("thread_id"); }
        ResponseTemplate::new(200).set_delay(Duration::from_millis(80)).set_body_json(json!({"next_batch":if updated{"r2"}else{"r1"},"rooms":{"join":{"!studio:local":{"state":{"events":[
            {"type":"m.room.create","state_key":"","sender":"@alice:local","event_id":"$create","origin_server_ts":1,"content":{"creator":"@alice:local","room_version":"10"}},
            {"type":"m.room.member","state_key":"@alice:local","sender":"@alice:local","event_id":"$member","origin_server_ts":2,"content":{"membership":"join"}}
        ]},"timeline":{"events":[],"limited":false},"ephemeral":{"events":[{"type":"m.receipt","content":receipts}]}}}},"device_one_time_keys_count":{"signed_curve25519":50}}))
    }).with_priority(1).mount(&server).await;
    let (sender, mut receiver, worker) = login(&server).await;
    sender
        .send(Command::Select("!studio:local".into()))
        .await
        .unwrap();
    let initial=next(&mut receiver,|e| matches!(e,EventKind::History{messages,..} if messages.len()==3 && !messages[0].read_by.is_empty())).await;
    let EventKind::History { messages, .. } = initial.kind else {
        unreachable!()
    };
    assert_eq!(messages[0].read_by, vec!["@alice:local", "@bob:local"]);
    assert!(messages[1].read_by.is_empty());
    assert!(messages[2].read_by.is_empty());
    assert_eq!(messages[2].thread_read_by, vec!["@carol:local"]);
    changed.store(true, Ordering::Release);
    let updated=next(&mut receiver,|e| matches!(e,EventKind::History{messages,..} if messages.len()==3 && !messages[1].read_by.is_empty())).await;
    let EventKind::History { messages, .. } = updated.kind else {
        unreachable!()
    };
    assert!(messages[0].read_by.is_empty());
    assert_eq!(messages[1].read_by, vec!["@alice:local", "@bob:local"]);
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}

#[tokio::test]
async fn queued_thread_send_never_fetches_root_and_keeps_worker_responsive() {
    use std::time::Duration;
    let server = server().await;
    Mock::given(method("GET"))
        .and(path_regex(".*/event/.*|.*/relations/.*"))
        .respond_with(ResponseTemplate::new(500).set_delay(Duration::from_secs(10)))
        .expect(0)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path_regex(".*/send/m.room.message/.*"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_secs(2))
                .set_body_json(json!({"event_id":"$reply"})),
        )
        .with_priority(1)
        .mount(&server)
        .await;
    let (sender, mut receiver, worker) = login(&server).await;
    sender
        .send(Command::EnqueueThread {
            room_id: "!studio:local".into(),
            root: "$root".into(),
            latest: "$latest".into(),
            body: "Fast thread reply".into(),
            transaction: "local-submit".into(),
        })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1),next(&mut receiver,|e|matches!(e,EventKind::ThreadQueued { transaction,result:Ok(()),.. } if transaction=="local-submit"))).await.expect("queue acceptance must not wait for HTTP");
    let (tx, rx) = tokio::sync::oneshot::channel();
    sender.send(Command::Barrier(tx)).await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), rx)
        .await
        .expect("worker remains responsive during upload")
        .unwrap();
    next(
        &mut receiver,
        |e| matches!(e,EventKind::Delivery { event_id:Some(id),.. } if id=="$reply"),
    )
    .await;
    let requests = server.received_requests().await.unwrap();
    let sent = requests
        .iter()
        .find(|r| r.url.path().contains("/send/m.room.message/"))
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&sent.body).unwrap();
    assert_eq!(body["m.relates_to"]["rel_type"], "m.thread");
    assert_eq!(body["m.relates_to"]["event_id"], "$root");
    assert_eq!(body["m.relates_to"]["m.in_reply_to"]["event_id"], "$latest");
    assert_eq!(body["m.relates_to"]["is_falling_back"], true);
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}
