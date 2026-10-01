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
