use super::*;

async fn expired(events: &mut Receiver) {
    next(events, |e| matches!(e, EventKind::Error("session-expired"))).await;
    next(events, |e| matches!(e, EventKind::ReauthenticationRequired)).await;
}
async fn sso_url(sender: &archaic_matrix::Sender, events: &mut Receiver) -> String {
    sender
        .send(Command::Reauthorize { oauth: false })
        .await
        .unwrap();
    let EventKind::BrowserUrl(url) = next(events, |e| matches!(e, EventKind::BrowserUrl(_)))
        .await
        .kind
    else {
        unreachable!()
    };
    url::Url::parse(&url)
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == "redirectUrl")
        .unwrap()
        .1
        .into_owned()
}
#[tokio::test]
async fn sso_reauthorization_cancels_rejects_identity_changes_and_retains_device() {
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
    expired(&mut receiver).await;
    let original = std::fs::read(dir.path().join("session.enc")).unwrap();
    let callback = sso_url(&sender, &mut receiver).await;
    let http = matrix_sdk::reqwest::Client::new();
    assert_eq!(
        http.get(format!("{callback}?loginToken=one&loginToken=two"))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        http.get(format!("{callback}?loginToken=one"))
            .header("Host", "attacker.invalid")
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    // A worker barrier must not cancel an active browser flow.
    let (tx, rx) = tokio::sync::oneshot::channel();
    sender.send(Command::Barrier(tx)).await.unwrap();
    rx.await.unwrap();
    sender.send(Command::CancelLogin).await.unwrap();
    next(&mut receiver, |e| {
        matches!(e, EventKind::Error("sso-cancelled"))
    })
    .await;
    assert!(http.get(&callback).send().await.is_err());
    assert_eq!(
        std::fs::read(dir.path().join("session.enc")).unwrap(),
        original
    );
    expired(&mut receiver).await;
    Mock::given(method("POST")).and(path_regex(r".*/login$"))
        .and(wiremock::matchers::body_partial_json(json!({"token":"wrong"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"user_id":"@mallory:local","device_id":"ARCHAIC_TEST","access_token":"wrong-account"})))
        .with_priority(1).mount(&server).await;
    let callback = sso_url(&sender, &mut receiver).await;
    assert_eq!(
        http.get(format!("{callback}?loginToken=wrong"))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    next(&mut receiver, |e| {
        matches!(e, EventKind::Error("session-key-mismatch"))
    })
    .await;
    assert_eq!(
        std::fs::read(dir.path().join("session.enc")).unwrap(),
        original
    );
    expired(&mut receiver).await;
    Mock::given(method("POST")).and(path_regex(r".*/login$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"user_id":"@alice:local","device_id":"ARCHAIC_TEST","access_token":"renewed-sso"})))
        .with_priority(1).mount(&server).await;
    let callback = sso_url(&sender, &mut receiver).await;
    assert_eq!(
        http.get(format!("{callback}?loginToken=correct"))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    next(&mut receiver, |e| matches!(e, EventKind::Connected(_))).await;
    next(
        &mut receiver,
        |e| matches!(e, EventKind::Rooms(r) if !r.is_empty()),
    )
    .await;
    let requests = server.received_requests().await.unwrap();
    for request in requests
        .iter()
        .filter(|r| r.url.path().ends_with("/login") && r.method == "POST")
        .skip(1)
    {
        let body = request.body_json::<serde_json::Value>().unwrap();
        assert_eq!(body["device_id"], "ARCHAIC_TEST");
        assert_eq!(body["type"], "m.login.token");
    }
    assert!(!requests.iter().any(|r| {
        r.headers
            .get("authorization")
            .is_some_and(|h| h == "Bearer wrong-account")
    }));
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}

#[tokio::test]
async fn threads_use_scoped_pagination_and_aliases_cannot_cross_rooms() {
    use archaic_matrix::{organization::Action, room_settings::Operation};
    let server = server().await;
    Mock::given(method("GET")).and(path_regex(r".*/rooms/.*/threads$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"chunk":[{"type":"m.room.message","event_id":"$root","sender":"@bob:local","origin_server_ts":10,"room_id":"!studio:local","content":{"msgtype":"m.text","body":"Thread subject"},"unsigned":{"m.relations":{"m.thread":{"count":2,"current_user_participated":true,"latest_event":{"type":"m.room.message","event_id":"$latest","sender":"@bob:local","origin_server_ts":12,"content":{"msgtype":"m.text","body":"Reply"}}}}}}],"next_batch":"thread-next"})))
        .mount(&server).await;
    Mock::given(method("GET"))
        .and(path_regex(r".*/directory/room/.*"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"room_id":"!other:local","servers":["local"]})),
        )
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
    for (operation, value, cursor, error) in [
        (Operation::Threads, "", None, None),
        (
            Operation::ParticipatedThreads,
            "",
            Some("thread-next".into()),
            Some("history-invalid-page"),
        ),
        (
            Operation::AliasRemove,
            "#other:local",
            None,
            Some("organization-invalid"),
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
                    extra: String::new(),
                    next: cursor,
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
        if let Some(error) = error {
            assert_eq!(result.unwrap_err(), error);
        } else {
            let page = result.unwrap();
            assert_eq!(page.rows[0].id, "$root");
            assert_eq!(page.rows[0].title, "Thread subject");
            assert_eq!(page.next.as_deref(), Some("thread-next"));
        }
    }
    let requests = server.received_requests().await.unwrap();
    assert!(!requests.iter().any(|r| r.method == "DELETE"));
    assert!(requests.iter().any(|r| {
        r.url.path().ends_with("/threads")
            && r.url
                .query_pairs()
                .any(|(k, v)| k == "include" && v == "participated")
    }));
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}
