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
    assert_eq!(
        items[0].message.as_ref().unwrap().body.as_deref(),
        Some("durable draft")
    );
    assert_eq!(
        items[0].message.as_ref().unwrap().transaction_id.as_deref(),
        Some(id.as_str())
    );
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
        |e| matches!(e,EventKind::Delivery { transaction, event_id:Some(event), .. } if transaction == &id && event == "$delivered"),
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
    // Restored Rooms now precede the network; wait for a successful sync explicitly.
    next(&mut receiver, |e| matches!(e, EventKind::Status("ready"))).await;
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

#[tokio::test]
async fn drafts_are_encrypted_and_soft_logout_resumes_the_same_device() {
    let server = server().await;
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
        |e| matches!(e, EventKind::Rooms(r) if !r.is_empty()),
    )
    .await;
    sender
        .send(Command::SaveDraft {
            room_id: "!studio:local".into(),
            body: "private unsent phrase".into(),
        })
        .await
        .unwrap();
    // A following command is a barrier: the worker has saved the draft before handling it.
    sender.send(Command::Refresh).await.unwrap();
    next(&mut receiver, |e| matches!(e, EventKind::Rooms(_))).await;
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
    let store = std::fs::read_dir(dir.path().join("stores"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let bytes = std::fs::read(store.join("drafts.enc")).unwrap();
    assert!(!bytes.windows(20).any(|b| b == b"private unsent phrase"));
    let profile = Profile::open(dir.path().into(), &vault).unwrap();
    let (sender, commands, events, mut receiver) = channels();
    let worker = tokio::spawn(archaic_matrix::run_with_profile(commands, events, profile));
    let EventKind::Drafts(drafts) = next(&mut receiver, |e| matches!(e, EventKind::Drafts(_)))
        .await
        .kind
    else {
        unreachable!()
    };
    assert_eq!(drafts["!studio:local"], "private unsent phrase");
    Mock::given(method("GET"))
        .and(path_regex(r".*/sync$"))
        .and(wiremock::matchers::header(
            "authorization",
            "Bearer test-access-token",
        ))
        .respond_with(ResponseTemplate::new(401).set_body_json(
            json!({"errcode":"M_UNKNOWN_TOKEN","error":"expired","soft_logout":true}),
        ))
        .with_priority(1)
        .mount(&server)
        .await;
    next(&mut receiver, |e| {
        matches!(e, EventKind::Error("session-expired"))
    })
    .await;
    next(&mut receiver, |e| {
        matches!(e, EventKind::ReauthenticationRequired)
    })
    .await;
    Mock::given(method("POST")).and(path_regex(r".*/login$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"user_id":"@alice:local","device_id":"ARCHAIC_TEST","access_token":"resumed-access"})))
        .with_priority(1).mount(&server).await;
    sender
        .send(Command::Reauthenticate {
            password: archaic_matrix::security::Secret::new("test".into()),
        })
        .await
        .unwrap();
    next(&mut receiver, |e| matches!(e, EventKind::Connected(_))).await;
    let requests = server.received_requests().await.unwrap();
    let login = requests
        .iter()
        .rev()
        .find(|r| r.url.path().ends_with("/login") && r.method == "POST")
        .unwrap()
        .body_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(login["device_id"], "ARCHAIC_TEST");
    let EventKind::Drafts(drafts) = next(&mut receiver, |e| matches!(e, EventKind::Drafts(_)))
        .await
        .kind
    else {
        unreachable!()
    };
    assert_eq!(drafts["!studio:local"], "private unsent phrase");
    assert_eq!(
        std::fs::read_dir(dir.path().join("stores"))
            .unwrap()
            .count(),
        1
    );
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}

#[tokio::test]
async fn directory_space_pagination_and_admin_permissions_use_real_endpoints() {
    use archaic_matrix::organization::Action;
    let server = server().await;
    Mock::given(method("POST")).and(path_regex(r".*/publicRooms$")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"chunk":[{"room_id":"!public:local","name":"Public room","num_joined_members":2,"world_readable":false,"guest_can_join":false}],"next_batch":"next-directory"}))).mount(&server).await;
    Mock::given(method("GET")).and(path_regex(r".*/hierarchy$")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"rooms":[{"room_id":"!space:local","name":"Space","num_joined_members":2,"world_readable":false,"guest_can_join":false,"children_state":[]}]}))).mount(&server).await;
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
    let id = transaction_id();
    sender
        .send(Command::Organization {
            id: id.clone(),
            action: Action::Directory {
                query: "Public".into(),
                next: None,
            },
        })
        .await
        .unwrap();
    let EventKind::Organization {
        result: Ok(page), ..
    } = next(
        &mut receiver,
        |e| matches!(e,EventKind::Organization{id:i,..} if i==&id),
    )
    .await
    .kind
    else {
        panic!("directory failed")
    };
    assert_eq!(page.rows[0].id, "!public:local");
    assert_eq!(page.next.as_deref(), Some("next-directory"));
    let id = transaction_id();
    sender
        .send(Command::Organization {
            id: id.clone(),
            action: Action::Hierarchy {
                room: "!space:local".into(),
                next: None,
            },
        })
        .await
        .unwrap();
    let EventKind::Organization {
        result: Ok(page), ..
    } = next(
        &mut receiver,
        |e| matches!(e,EventKind::Organization{id:i,..} if i==&id),
    )
    .await
    .kind
    else {
        panic!("hierarchy failed")
    };
    assert_eq!(page.rows[0].title, "Space");
    let id = transaction_id();
    sender
        .send(Command::Organization {
            id: id.clone(),
            action: Action::SetName {
                room: "!studio:local".into(),
                value: "Forbidden change".into(),
            },
        })
        .await
        .unwrap();
    let EventKind::Organization { result, .. } = next(
        &mut receiver,
        |e| matches!(e,EventKind::Organization{id:i,..} if i==&id),
    )
    .await
    .kind
    else {
        unreachable!()
    };
    assert!(result.is_err());
    assert!(
        !server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .any(|r| r.method == "PUT" && r.url.path().contains("/state/m.room.name"))
    );
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}

#[tokio::test]
async fn oauth_login_uses_pkce_validates_state_and_restores_its_session() {
    let server = server().await;
    let base = server.uri();
    Mock::given(method("GET")).and(path_regex(r".*/auth_metadata$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issuer":base,"authorization_endpoint":format!("{base}/authorize"),
            "token_endpoint":format!("{base}/token"),"registration_endpoint":format!("{base}/register"),
            "revocation_endpoint":format!("{base}/revoke"),"response_types_supported":["code"],
            "response_modes_supported":["query","fragment"],"grant_types_supported":["authorization_code","refresh_token"],
            "code_challenge_methods_supported":["S256"]
        }))).mount(&server).await;
    Mock::given(method("POST"))
        .and(path("/register"))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(json!({"client_id":"archaic-test-client"})),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"access_token":"oauth-token","token_type":"Bearer","expires_in":3600}),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(r".*/account/whoami$"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"user_id":"@alice:local","device_id":"OAUTH_DEVICE"})),
        )
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let vault = MemoryVault::default();
    let profile = Profile::open(dir.path().into(), &vault).unwrap();
    let (sender, commands, events, mut receiver) = channels();
    let worker = tokio::spawn(archaic_matrix::run_with_profile(commands, events, profile));
    next(&mut receiver, |e| matches!(e, EventKind::SignedOut)).await;
    sender
        .send(Command::OAuthLogin {
            epoch: 1,
            server: base,
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
    let params = url
        .query_pairs()
        .into_owned()
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(params["code_challenge_method"], "S256");
    assert!(!params["code_challenge"].is_empty());
    assert_eq!(params["client_id"], "archaic-test-client");
    let device = params["scope"]
        .split_whitespace()
        .find_map(|s| s.strip_prefix("urn:matrix:org.matrix.msc2967.client:device:"))
        .unwrap()
        .to_owned();
    Mock::given(method("GET"))
        .and(path_regex(r".*/account/whoami$"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"user_id":"@alice:local","device_id":device})),
        )
        .with_priority(2)
        .mount(&server)
        .await;
    let callback = &params["redirect_uri"];
    let http = matrix_sdk::reqwest::Client::new();
    assert_eq!(
        http.get(format!("{callback}?state=wrong&code=test-code"))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    assert!(
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .all(|r| r.url.path() != "/token")
    );
    assert_eq!(
        http.get(format!(
            "{callback}?{}",
            url::form_urlencoded::Serializer::new(String::new())
                .append_pair("state", &params["state"])
                .append_pair("code", "test-code")
                .finish()
        ))
        .send()
        .await
        .unwrap()
        .status(),
        200
    );
    next(&mut receiver, |e| matches!(e, EventKind::Connected(_))).await;
    let requests = server.received_requests().await.unwrap();
    let exchange = requests.iter().find(|r| r.url.path() == "/token").unwrap();
    let form = url::form_urlencoded::parse(&exchange.body)
        .into_owned()
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(form["grant_type"], "authorization_code");
    assert_eq!(form["code"], "test-code");
    assert!(!form["code_verifier"].is_empty());
    next(
        &mut receiver,
        |e| matches!(e, EventKind::Rooms(r) if !r.is_empty()),
    )
    .await;
    Mock::given(method("GET"))
        .and(path_regex(r".*/sync$"))
        .and(wiremock::matchers::header(
            "authorization",
            "Bearer oauth-token",
        ))
        .respond_with(ResponseTemplate::new(401).set_body_json(
            json!({"errcode":"M_UNKNOWN_TOKEN","error":"expired","soft_logout":true}),
        ))
        .with_priority(1)
        .mount(&server)
        .await;
    next(&mut receiver, |e| {
        matches!(e, EventKind::Error("session-expired"))
    })
    .await;
    next(&mut receiver, |e| {
        matches!(e, EventKind::ReauthenticationRequired)
    })
    .await;
    sender
        .send(Command::Reauthorize { oauth: true })
        .await
        .unwrap();
    let EventKind::BrowserUrl(renew) =
        next(&mut receiver, |e| matches!(e, EventKind::BrowserUrl(_)))
            .await
            .kind
    else {
        unreachable!()
    };
    let renew = url::Url::parse(&renew)
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(renew["client_id"], params["client_id"]);
    assert_eq!(renew["scope"], params["scope"]);
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"access_token":"renewed-oauth","token_type":"Bearer","expires_in":3600}),
        ))
        .with_priority(1)
        .mount(&server)
        .await;
    let saved_before_renewal = std::fs::read(dir.path().join("session.enc")).unwrap();
    Mock::given(method("GET"))
        .and(path_regex(r".*/account/whoami$"))
        .and(wiremock::matchers::header(
            "authorization",
            "Bearer renewed-oauth",
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"user_id":"@alice:local","device_id":"OTHER_DEVICE"})),
        )
        .up_to_n_times(2)
        .with_priority(1)
        .mount(&server)
        .await;
    let mut renewed = url::Url::parse(&renew["redirect_uri"]).unwrap();
    renewed
        .query_pairs_mut()
        .append_pair("state", &renew["state"])
        .append_pair("code", "renewal");
    assert_eq!(http.get(renewed).send().await.unwrap().status(), 200);
    next(&mut receiver, |e| {
        matches!(e, EventKind::Error("session-key-mismatch"))
    })
    .await;
    assert_eq!(
        std::fs::read(dir.path().join("session.enc")).unwrap(),
        saved_before_renewal
    );
    next(&mut receiver, |e| {
        matches!(e, EventKind::Error("session-expired"))
    })
    .await;
    next(&mut receiver, |e| {
        matches!(e, EventKind::ReauthenticationRequired)
    })
    .await;
    sender
        .send(Command::Reauthorize { oauth: true })
        .await
        .unwrap();
    let EventKind::BrowserUrl(renew) =
        next(&mut receiver, |e| matches!(e, EventKind::BrowserUrl(_)))
            .await
            .kind
    else {
        unreachable!()
    };
    let renew = url::Url::parse(&renew)
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect::<std::collections::HashMap<_, _>>();
    let mut renewed = url::Url::parse(&renew["redirect_uri"]).unwrap();
    renewed
        .query_pairs_mut()
        .append_pair("state", &renew["state"])
        .append_pair("code", "renewal");
    assert_eq!(http.get(renewed).send().await.unwrap().status(), 200);
    next(&mut receiver, |e| matches!(e, EventKind::Connected(_))).await;
    next(
        &mut receiver,
        |e| matches!(e, EventKind::Rooms(r) if !r.is_empty()),
    )
    .await;
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
    assert!(
        http.get(callback).send().await.is_err(),
        "loopback callback closes after login"
    );
    let profile = Profile::open(dir.path().into(), &vault).unwrap();
    let (sender, commands, events, mut receiver) = channels();
    let worker = tokio::spawn(archaic_matrix::run_with_profile(commands, events, profile));
    next(&mut receiver, |e| matches!(e, EventKind::Connected(_))).await;
    assert_eq!(
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .filter(|r| r.url.path() == "/register")
            .count(),
        1
    );
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}

#[tokio::test]
async fn organization_writes_preserve_settings_and_reject_power_escalation() {
    use archaic_matrix::organization::Action;
    let server = server().await;
    let sync = json!({"next_batch":"admin","rooms":{"join":{"!studio:local":{
        "state":{"events":[
            {"type":"m.room.create","state_key":"","sender":"@alice:local","event_id":"$create","origin_server_ts":1,"content":{"creator":"@alice:local","room_version":"10","type":"m.space"}},
            {"type":"m.room.member","state_key":"@alice:local","sender":"@alice:local","event_id":"$member","origin_server_ts":2,"content":{"membership":"join"}},
            {"type":"m.room.power_levels","state_key":"","sender":"@alice:local","event_id":"$power","origin_server_ts":3,"content":{"users":{"@alice:local":100},"state_default":50,"ban":50,"kick":50}}
        ]},"timeline":{"events":[],"limited":false},"summary":{},"unread_notifications":{}
    }}},"device_one_time_keys_count":{"signed_curve25519":50}});
    Mock::given(method("GET"))
        .and(path_regex(r".*/sync$"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(80))
                .set_body_json(sync),
        )
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path_regex(r".*/state/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"event_id":"$updated"})))
        .mount(&server)
        .await;
    for pattern in [
        r".*/tags/m.favourite$",
        r".*/account_data/m.ignored_user_list$",
    ] {
        Mock::given(method("PUT"))
            .and(path_regex(pattern))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .mount(&server)
            .await;
    }
    Mock::given(method("POST"))
        .and(path_regex(r".*/(invite|kick|ban|unban)$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;
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
    for action in [
        Action::SetName {
            room: "!studio:local".into(),
            value: "Renamed".into(),
        },
        Action::SetTopic {
            room: "!studio:local".into(),
            value: "A topic".into(),
        },
        Action::JoinRule {
            room: "!studio:local".into(),
            value: "knock".into(),
        },
        Action::HistoryVisibility {
            room: "!studio:local".into(),
            value: "invited".into(),
        },
        Action::GuestAccess {
            room: "!studio:local".into(),
            value: "forbidden".into(),
        },
        Action::Power {
            room: "!studio:local".into(),
            user: "@bob:local".into(),
            level: 50,
        },
        Action::Moderate {
            room: "!studio:local".into(),
            user: "@bob:local".into(),
            kind: "ban".into(),
            reason: "Test moderation".into(),
        },
        Action::Favourite {
            room: "!studio:local".into(),
            enabled: true,
        },
        Action::Ignore {
            user: "@bob:local".into(),
            enabled: true,
        },
        Action::Advanced {
            room: "!studio:local".into(),
            operation: archaic_matrix::room_settings::Operation::Restricted,
            value: "!members:local".into(),
            extra: String::new(),
            next: None,
        },
        Action::Advanced {
            room: "!studio:local".into(),
            operation: archaic_matrix::room_settings::Operation::EventPower,
            value: "m.room.message".into(),
            extra: "25".into(),
            next: None,
        },
        Action::Advanced {
            room: "!studio:local".into(),
            operation: archaic_matrix::room_settings::Operation::Retention,
            value: "1000".into(),
            extra: "2000".into(),
            next: None,
        },
        Action::SpaceChild {
            room: "!studio:local".into(),
            child: "!child:local".into(),
            remove: false,
        },
    ] {
        let id = transaction_id();
        sender
            .send(Command::Organization {
                id: id.clone(),
                action,
            })
            .await
            .unwrap();
        let EventKind::Organization { result, .. } = next(
            &mut receiver,
            |e| matches!(e,EventKind::Organization{id:i,..} if *i==id),
        )
        .await
        .kind
        else {
            unreachable!()
        };
        assert!(result.is_ok(), "{result:?}");
    }
    let id = transaction_id();
    sender
        .send(Command::Organization {
            id: id.clone(),
            action: Action::Power {
                room: "!studio:local".into(),
                user: "@bob:local".into(),
                level: 101,
            },
        })
        .await
        .unwrap();
    let EventKind::Organization { result, .. } = next(
        &mut receiver,
        |e| matches!(e,EventKind::Organization{id:i,..} if *i==id),
    )
    .await
    .kind
    else {
        unreachable!()
    };
    assert_eq!(result.unwrap_err(), "room-action-forbidden");
    for (operation, value, extra, error) in [
        (
            archaic_matrix::room_settings::Operation::EventPower,
            "m.room.message",
            "101",
            "room-action-forbidden",
        ),
        (
            archaic_matrix::room_settings::Operation::Retention,
            "2000",
            "1000",
            "organization-invalid",
        ),
        (
            archaic_matrix::room_settings::Operation::Restricted,
            "bad-id",
            "",
            "organization-invalid",
        ),
        (
            archaic_matrix::room_settings::Operation::ParentAdd,
            "!studio:local",
            "",
            "organization-invalid",
        ),
    ] {
        let id = transaction_id();
        sender
            .send(Command::Organization {
                id: id.clone(),
                action: Action::Advanced {
                    room: "!studio:local".into(),
                    operation,
                    value: value.into(),
                    extra: extra.into(),
                    next: None,
                },
            })
            .await
            .unwrap();
        let EventKind::Organization { result, .. } = next(
            &mut receiver,
            |e| matches!(e,EventKind::Organization{id:i,..} if *i==id),
        )
        .await
        .kind
        else {
            unreachable!()
        };
        assert_eq!(result.unwrap_err(), error);
    }
    let requests = server.received_requests().await.unwrap();
    for (suffix, key, value) in [
        ("m.room.name/", "name", json!("Renamed")),
        ("m.room.topic/", "topic", json!("A topic")),
        ("m.room.join_rules/", "join_rule", json!("knock")),
        (
            "m.room.history_visibility/",
            "history_visibility",
            json!("invited"),
        ),
        ("m.room.guest_access/", "guest_access", json!("forbidden")),
        ("/ban", "reason", json!("Test moderation")),
        (
            "m.ignored_user_list",
            "ignored_users",
            json!({"@bob:local":{}}),
        ),
    ] {
        let request = requests
            .iter()
            .find(|r| r.method != "GET" && r.url.path().ends_with(suffix))
            .unwrap_or_else(|| panic!("missing {suffix}"));
        assert_eq!(
            request.body_json::<serde_json::Value>().unwrap()[key],
            value
        );
    }
    let power = requests
        .iter()
        .filter(|r| r.method == "PUT" && r.url.path().contains("/state/m.room.power_levels"))
        .collect::<Vec<_>>();
    assert_eq!(
        power.len(),
        2,
        "only the authorized user and event-level changes may write state"
    );
    assert_eq!(
        power[0].body_json::<serde_json::Value>().unwrap()["users"]["@bob:local"],
        50
    );
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}

mod closure;
