use super::*;
fn search(query: &str, mode: SearchMode, more: bool) -> ToolRequest {
    request(ToolAction::Search {
        query: query.into(),
        mode,
        more,
    })
}
fn page(data: Result<ToolData, &'static str>) -> archaic_matrix::SearchPage {
    let Ok(ToolData::Search(page)) = data else {
        panic!("expected search page")
    };
    page
}
#[tokio::test]
async fn server_search_scopes_query_and_retries_same_cursor_without_losing_results() {
    let server = server().await;
    let first = event(
        "$found1",
        "m.room.message",
        json!({"msgtype":"m.text","body":"Needle one"}),
        10,
    );
    let second = event(
        "$found2",
        "m.room.message",
        json!({"msgtype":"m.text","body":"Needle two"}),
        20,
    );
    Mock::given(method("POST")).and(path_regex(".*/search$")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"search_categories":{"room_events":{"results":[{"result":first}],"next_batch":"next-page"}}}))).mount(&server).await;
    Mock::given(method("POST")).and(path_regex(".*/search$")).and(query_param("next_batch","next-page")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"search_categories":{"room_events":{"results":[{"result":first},{"result":second}]}}}))).with_priority(2).mount(&server).await;
    Mock::given(method("POST"))
        .and(path_regex(".*/search$"))
        .and(query_param("next_batch", "next-page"))
        .respond_with(
            ResponseTemplate::new(503)
                .set_body_json(json!({"errcode":"M_UNKNOWN","error":"retry"})),
        )
        .with_priority(1)
        .up_to_n_times(1)
        .mount(&server)
        .await;
    let (sender, mut receiver, worker) = login(&server).await;
    let p = page(
        perform(
            &sender,
            &mut receiver,
            &search("Needle", SearchMode::Server, false),
        )
        .await,
    );
    assert_eq!(p.messages.len(), 1);
    assert!(p.more);
    assert_error(
        perform(
            &sender,
            &mut receiver,
            &search("Changed", SearchMode::Server, true),
        )
        .await,
        "search-restart",
    );
    let more = search("Needle", SearchMode::Server, true);
    assert_error(
        perform(&sender, &mut receiver, &more).await,
        "message-action-failed",
    );
    let p = page(perform(&sender, &mut receiver, &more).await);
    assert_eq!(p.messages.len(), 2);
    assert!(!p.more);
    let requests = server.received_requests().await.unwrap();
    let searches: Vec<_> = requests
        .iter()
        .filter(|r| r.url.path().ends_with("/search"))
        .collect();
    assert_eq!(searches.len(), 3);
    assert_eq!(searches[1].url, searches[2].url);
    for search in searches {
        let body: Value = search.body_json().unwrap();
        assert_eq!(
            body["search_categories"]["room_events"]["search_term"],
            "Needle"
        );
        assert_eq!(
            body["search_categories"]["room_events"]["filter"]["rooms"],
            json!(["!studio:local"])
        );
    }
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}
#[tokio::test]
async fn local_search_scans_history_without_sending_query_and_detects_stalled_pagination() {
    let server = server().await;
    Mock::given(method("GET")).and(path_regex(".*/messages$")).and(query_param("from","older")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"start":"older","end":"older","chunk":[event("$stale","m.room.message",json!({"msgtype":"m.text","body":"FIRST older"}),1)]}))).with_priority(1).up_to_n_times(1).mount(&server).await;
    Mock::given(method("GET")).and(path_regex(".*/messages$")).and(query_param("from","older")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"start":"older","chunk":[event("$old","m.room.message",json!({"msgtype":"m.text","body":"FIRST older"}),1)]}))).with_priority(2).mount(&server).await;
    let (sender, mut receiver, worker) = login(&server).await;
    let p = page(
        perform(
            &sender,
            &mut receiver,
            &search("FIRST", SearchMode::Local, false),
        )
        .await,
    );
    assert_eq!(p.messages.len(), 1);
    assert_eq!(p.scanned, 2);
    assert!(p.more);
    let more = search("FIRST", SearchMode::Local, true);
    assert_error(perform(&sender, &mut receiver, &more).await, "page-stalled");
    let p = page(perform(&sender, &mut receiver, &more).await);
    assert_eq!(p.messages.len(), 2);
    assert_eq!(p.scanned, 3);
    assert!(!p.more);
    assert!(!p.messages.iter().any(|m| m.id == "$stale"));
    assert!(
        !server.received_requests().await.unwrap().iter().any(|r| r
            .url
            .path()
            .ends_with("/search")
            || r.url.as_str().contains("FIRST"))
    );
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}
#[tokio::test]
async fn encrypted_server_search_is_blocked_before_disclosing_query() {
    let server = server().await;
    Mock::given(method("GET"))
        .and(path_regex(".*/state/m.room.encryption/?$"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"algorithm":"m.megolm.v1.aes-sha2"})),
        )
        .with_priority(1)
        .mount(&server)
        .await;
    let (sender, mut receiver, worker) = login(&server).await;
    assert_error(
        perform(
            &sender,
            &mut receiver,
            &search("private query", SearchMode::Server, false),
        )
        .await,
        "search-encrypted",
    );
    assert!(
        !server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .any(|r| r.url.path().ends_with("/search"))
    );
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}
#[tokio::test]
async fn cross_room_search_results_are_rejected() {
    let server = server().await;
    let mut wrong = event(
        "$wrong",
        "m.room.message",
        json!({"msgtype":"m.text","body":"wrong room"}),
        1,
    );
    wrong["room_id"] = json!("!other:local");
    Mock::given(method("POST"))
        .and(path_regex(".*/search$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"search_categories":{"room_events":{"results":[{"result":wrong}]}}}),
        ))
        .mount(&server)
        .await;
    let (sender, mut receiver, worker) = login(&server).await;
    assert_error(
        perform(
            &sender,
            &mut receiver,
            &search("wrong", SearchMode::Server, false),
        )
        .await,
        "history-invalid-page",
    );
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}

#[tokio::test]
async fn local_search_reports_undecryptable_events_instead_of_silent_completeness() {
    let server = server().await;
    Mock::given(method("GET")).and(path_regex(".*/messages$")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"start":"start","chunk":[event("$encrypted","m.room.encrypted",json!({"algorithm":"m.megolm.v1.aes-sha2","ciphertext":"invalid","session_id":"unknown","sender_key":"unknown","device_id":"OTHER"}),1)]}))).with_priority(1).mount(&server).await;
    let (sender, mut receiver, worker) = login(&server).await;
    let p = page(
        perform(
            &sender,
            &mut receiver,
            &search("secret", SearchMode::Local, false),
        )
        .await,
    );
    assert_eq!(p.unavailable, 1);
    assert_eq!(p.scanned, 1);
    assert!(p.messages.is_empty());
    assert!(!p.more);
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}
