use super::*;
use archaic_matrix::storage::Profile;
#[tokio::test]
async fn queued_message_survives_restart_and_retries_with_same_sdk_transaction() {
    let server = server().await;
    Mock::given(method("PUT"))
        .and(path_regex(r".*/send/m.room.message/.*"))
        .respond_with(
            ResponseTemplate::new(403)
                .set_body_json(json!({"errcode":"M_FORBIDDEN","error":"blocked for test"})),
        )
        .with_priority(2)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let vault = MemoryVault::default();
    let profile = Profile::open(dir.path().into(), &vault).unwrap();
    let (sender, commands, events, mut receiver) = channels();
    let worker = tokio::spawn(archaic_matrix::run_with_profile(commands, events, profile));
    next(&mut receiver, |e| matches!(e, EventKind::SignedOut)).await;
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
    let transaction = transaction_id();
    sender
        .send(Command::Enqueue {
            room_id: "!studio:local".into(),
            body: "durable draft".into(),
            transaction: transaction.clone(),
        })
        .await
        .unwrap();
    next(
        &mut receiver,
        |e| matches!(e,EventKind::Queued{transaction:t} if *t==transaction),
    )
    .await;
    let EventKind::Outbox(items) = next(
        &mut receiver,
        |e| matches!(e,EventKind::Outbox(v) if v.len()==1&&v[0].failed),
    )
    .await
    .kind
    else {
        unreachable!()
    };
    let id = items[0].id.clone();
    assert_eq!(items[0].body, "durable draft");
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
    let profile = Profile::open(dir.path().into(), &vault).unwrap();
    let (sender, commands, events, mut receiver) = channels();
    let worker = tokio::spawn(archaic_matrix::run_with_profile(commands, events, profile));
    next(
        &mut receiver,
        |e| matches!(e,EventKind::Outbox(v) if v.iter().any(|i|i.id==id)),
    )
    .await;
    Mock::given(method("PUT"))
        .and(path_regex(r".*/send/m.room.message/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"event_id":"$delivered"})))
        .with_priority(1)
        .mount(&server)
        .await;
    sender
        .send(Command::OutboxAction {
            room_id: "!studio:local".into(),
            id: id.clone(),
            retry: true,
        })
        .await
        .unwrap();
    next(
        &mut receiver,
        |e| matches!(e,EventKind::Outbox(v) if v.is_empty()),
    )
    .await;
    let requests = server.received_requests().await.unwrap();
    let sends = requests
        .iter()
        .filter(|r| r.method == "PUT" && r.url.path().contains("/send/"))
        .collect::<Vec<_>>();
    assert!(sends.len() >= 2);
    assert!(sends.iter().all(|r| r.url.path().ends_with(&id)));
    sender.send(Command::Logout).await.unwrap();
    next(&mut receiver, |e| matches!(e, EventKind::SignedOut)).await;
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}

#[tokio::test]
async fn refreshed_tokens_are_encrypted_and_restored_without_new_login() {
    let server = server().await;
    Mock::given(method("POST")).and(path_regex(r".*/login$")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"user_id":"@alice:local","device_id":"ARCHAIC_TEST","access_token":"expired-access","refresh_token":"first-refresh"}))).with_priority(1).mount(&server).await;
    Mock::given(wiremock::matchers::header(
        "authorization",
        "Bearer expired-access",
    ))
    .and(path_regex(r".*/sync$"))
    .respond_with(
        ResponseTemplate::new(401).set_body_json(
            json!({"errcode":"M_UNKNOWN_TOKEN","error":"expired","soft_logout":true}),
        ),
    )
    .with_priority(1)
    .mount(&server)
    .await;
    Mock::given(method("POST"))
        .and(path_regex(r".*/refresh$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"access_token":"rotated-access","refresh_token":"rotated-refresh"}),
        ))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let vault = MemoryVault::default();
    let profile = Profile::open(dir.path().into(), &vault).unwrap();
    let (sender, commands, events, mut receiver) = channels();
    let worker = tokio::spawn(archaic_matrix::run_with_profile(commands, events, profile));
    next(&mut receiver, |e| matches!(e, EventKind::SignedOut)).await;
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
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
    let bytes = std::fs::read(dir.path().join("session.enc")).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("rotated"));
    let before = server.received_requests().await.unwrap().len();
    let profile = Profile::open(dir.path().into(), &vault).unwrap();
    let (sender, commands, events, mut receiver) = channels();
    let worker = tokio::spawn(archaic_matrix::run_with_profile(commands, events, profile));
    next(
        &mut receiver,
        |e| matches!(e,EventKind::Rooms(r) if !r.is_empty()),
    )
    .await;
    let requests = server.received_requests().await.unwrap();
    assert!(requests[before..].iter().any(|r| {
        r.url.path().ends_with("/sync")
            && r.headers
                .get("authorization")
                .is_some_and(|v| v == "Bearer rotated-access")
    }));
    assert_eq!(
        requests
            .iter()
            .filter(|r| r.method == "POST" && r.url.path().ends_with("/login"))
            .count(),
        1
    );
    let login = requests
        .iter()
        .find(|r| r.method == "POST" && r.url.path().ends_with("/login"))
        .unwrap();
    assert_eq!(
        login.body_json::<serde_json::Value>().unwrap()["refresh_token"],
        true
    );
    sender.send(Command::Logout).await.unwrap();
    next(&mut receiver, |e| matches!(e, EventKind::SignedOut)).await;
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}

#[tokio::test]
async fn browser_sso_uses_loopback_callback_and_cancellation_closes_listener() {
    let server = server().await;
    Mock::given(method("GET"))
        .and(path_regex(r".*/login$"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"flows":[{"type":"m.login.sso"},{"type":"m.login.token"}]})),
        )
        .mount(&server)
        .await;

    let (sender, commands, events, mut receiver) = channels();
    let worker = tokio::spawn(archaic_matrix::run(commands, events));
    sender
        .send(Command::BrowserLogin {
            epoch: 1,
            server: server.uri(),
        })
        .await
        .unwrap();
    let EventKind::BrowserUrl(url) = next(&mut receiver, |e| matches!(e, EventKind::BrowserUrl(_)))
        .await
        .kind
    else {
        unreachable!()
    };
    let url = url::Url::parse(&url).unwrap();
    let redirect = url
        .query_pairs()
        .find(|(k, _)| k == "redirectUrl")
        .unwrap()
        .1
        .into_owned();
    let mut redirect = url::Url::parse(&redirect).unwrap();
    assert!(matches!(
        redirect.host_str(),
        Some("127.0.0.1" | "[::1]" | "localhost")
    ));
    redirect
        .query_pairs_mut()
        .append_pair("loginToken", "disposable-sso-token");
    let response = matrix_sdk::reqwest::get(redirect).await.unwrap();
    assert!(response.status().is_success());
    next(&mut receiver, |e| matches!(e, EventKind::Connected(_))).await;
    let requests = server.received_requests().await.unwrap();
    let login = requests
        .iter()
        .find(|r| r.method == "POST" && r.url.path().ends_with("/login"))
        .unwrap()
        .body_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(login["type"], "m.login.token");
    assert_eq!(login["token"], "disposable-sso-token");
    assert!(login.get("password").is_none());
    sender.send(Command::Logout).await.unwrap();
    next(&mut receiver, |e| matches!(e, EventKind::SignedOut)).await;
    sender
        .send(Command::BrowserLogin {
            epoch: 2,
            server: server.uri(),
        })
        .await
        .unwrap();
    let EventKind::BrowserUrl(url) = next(&mut receiver, |e| matches!(e, EventKind::BrowserUrl(_)))
        .await
        .kind
    else {
        unreachable!()
    };
    let url = url::Url::parse(&url).unwrap();
    let redirect = url
        .query_pairs()
        .find(|(k, _)| k == "redirectUrl")
        .unwrap()
        .1
        .into_owned();
    sender.send(Command::CancelLogin).await.unwrap();
    next(&mut receiver, |e| {
        matches!(e, EventKind::Error("sso-cancelled"))
    })
    .await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(matrix_sdk::reqwest::get(redirect).await.is_err());
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}

#[tokio::test]
async fn sync_updates_relations_without_refetching_and_context_pages_forward() {
    let server = server().await;
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
    sender
        .send(Command::Select("!studio:local".into()))
        .await
        .unwrap();
    next(
        &mut receiver,
        |e| matches!(e,EventKind::History{messages,..} if messages.len()==2),
    )
    .await;
    let message = |id: &str, body: &str| json!({"type":"m.room.message","sender":"@alice:local","event_id":id,"origin_server_ts":100,"content":{"msgtype":"m.text","body":body}});
    let edit = json!({"type":"m.room.message","sender":"@alice:local","event_id":"$edit","origin_server_ts":200,"content":{"msgtype":"m.text","body":"* updated","m.new_content":{"msgtype":"m.text","body":"updated"},"m.relates_to":{"rel_type":"m.replace","event_id":"$second"}}});
    Mock::given(method("GET")).and(path_regex(r".*/sync$")).respond_with(ResponseTemplate::new(200).set_delay(Duration::from_millis(50)).set_body_json(json!({"next_batch":"new","rooms":{"join":{"!studio:local":{"timeline":{"events":[edit,message("$new","new message")],"limited":false,"prev_batch":"ignored"}}}}}))).with_priority(1).mount(&server).await;
    next(&mut receiver,|e|matches!(e,EventKind::History{messages,..} if messages.len()==3&&messages[1].body.as_deref()==Some("updated"))).await;
    assert_eq!(
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .filter(|r| r.url.path().ends_with("/messages"))
            .count(),
        1
    );
    Mock::given(method("GET")).and(path_regex(r".*/context/.*anchor$")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"event":message("$anchor","anchor"),"events_before":[message("$old","old")],"events_after":[message("$after","after")],"start":"back-context","end":"forward-context","state":[]}))).mount(&server).await;
    sender
        .send(Command::Context {
            room_id: "!studio:local".into(),
            event_id: "$anchor".into(),
        })
        .await
        .unwrap();
    let EventKind::History { messages, info, .. } = next(
        &mut receiver,
        |e| matches!(e,EventKind::History{messages,..} if messages.iter().any(|m|m.id=="$anchor")),
    )
    .await
    .kind
    else {
        unreachable!()
    };
    assert_eq!(
        messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ["$old", "$anchor", "$after"]
    );
    assert!(info.can_load_newer && info.can_load_more);
    Mock::given(method("GET"))
        .and(path_regex(r".*/messages$"))
        .and(wiremock::matchers::query_param("dir", "f"))
        .and(wiremock::matchers::query_param("from", "forward-context"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"start":"forward-context","chunk":[message("$later","later")],"state":[]}),
        ))
        .with_priority(1)
        .mount(&server)
        .await;
    sender
        .send(Command::LoadHistory {
            room_id: "!studio:local".into(),
            kind: archaic_matrix::HistoryLoad::Newer,
        })
        .await
        .unwrap();
    let EventKind::History { messages, info, .. } = next(&mut receiver, |e| {
        matches!(
            e,
            EventKind::History {
                kind: archaic_matrix::HistoryLoad::Newer,
                ..
            }
        )
    })
    .await
    .kind
    else {
        unreachable!()
    };
    assert_eq!(messages.last().unwrap().id, "$later");
    assert!(!info.can_load_newer);
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}
