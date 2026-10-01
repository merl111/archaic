use super::{next, server};
use archaic_matrix::{Command, EventKind, Membership, Receiver, RoomAction, Sender, channels};
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path_regex},
};

async fn invited_server() -> (MockServer, Arc<AtomicUsize>) {
    let server = server().await;
    let state = Arc::new(AtomicUsize::new(0));
    let sync_state = state.clone();
    Mock::given(method("GET")).and(path_regex(".*/sync$"))
        .respond_with(move |_: &wiremock::Request| {
            let state = sync_state.load(Ordering::SeqCst);
            let member = json!({"type":"m.room.member","state_key":"@alice:local","sender":"@bob:local","content":{"membership": if state == 0 { "invite" } else if state == 1 { "join" } else { "leave" }}});
            let rooms = if state == 0 {
                json!({"invite":{"!invite:local":{"invite_state":{"events":[
                    {"type":"m.room.name","state_key":"","sender":"@bob:local","content":{"name":"Invitation test"}}, member
                ]}}}})
            } else {
                let mut member = member;
                member["event_id"] = json!(format!("$member-{state}"));
                member["origin_server_ts"] = json!(10 + state);
                let state_events = json!([
                    {"type":"m.room.create","state_key":"","sender":"@bob:local","event_id":"$create","origin_server_ts":1,"content":{"creator":"@bob:local","room_version":"10"}},
                    {"type":"m.room.name","state_key":"","sender":"@bob:local","event_id":"$name","origin_server_ts":2,"content":{"name":"Invitation test"}},member]);
                json!({if state == 1 { "join" } else { "leave" }: {"!invite:local": {"state":{"events":state_events},"timeline":{"events":[],"limited":false,"prev_batch":"history"}}}})
            };
            ResponseTemplate::new(200).set_delay(std::time::Duration::from_millis(50))
                .set_body_json(json!({"next_batch":format!("membership-{state}"),"rooms":rooms,"device_one_time_keys_count":{"signed_curve25519":50}}))
        }).with_priority(1).mount(&server).await;
    for (endpoint, new_state) in [("join", 1), ("leave", 2)] {
        let state = state.clone();
        Mock::given(method("POST"))
            .and(path_regex(format!(".*/rooms/.*/{endpoint}$")))
            .respond_with(move |_: &wiremock::Request| {
                state.store(new_state, Ordering::SeqCst);
                ResponseTemplate::new(200).set_body_json(json!({"room_id":"!invite:local"}))
            })
            .mount(&server)
            .await;
    }
    for endpoint in ["forget", "invite"] {
        Mock::given(method("POST"))
            .and(path_regex(format!(".*/rooms/.*/{endpoint}$")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .mount(&server)
            .await;
    }
    (server, state)
}

async fn login(server: &MockServer) -> (Sender, Receiver, tokio::task::JoinHandle<()>) {
    let (sender, commands, events, mut receiver) = channels();
    let worker = tokio::spawn(archaic_matrix::run(commands, events));
    sender
        .send(Command::Login {
            epoch: 5,
            server: server.uri(),
            username: "alice".into(),
            password: "test-password".into(),
        })
        .await
        .unwrap();
    let event = next(&mut receiver, |event| matches!(event, EventKind::Rooms(rooms) if rooms.iter().any(|room| room.membership == Membership::Invited))).await;
    let EventKind::Rooms(rooms) = event.kind else {
        unreachable!()
    };
    assert_eq!(rooms[0].name, "Invitation test");
    assert_eq!(rooms[0].inviter.as_deref(), Some("@bob:local"));
    (sender, receiver, worker)
}

async fn action(
    sender: &Sender,
    receiver: &mut Receiver,
    action: RoomAction,
) -> Result<(), &'static str> {
    sender
        .send(Command::RoomAction {
            room_id: "!invite:local".into(),
            action: action.clone(),
        })
        .await
        .unwrap();
    let event = next(receiver, |event| matches!(event, EventKind::RoomActionResult { action: received, .. } if received == &action)).await;
    let EventKind::RoomActionResult {
        room_id, result, ..
    } = event.kind
    else {
        unreachable!()
    };
    assert_eq!(room_id, "!invite:local");
    assert_eq!(event.epoch, 5);
    result
}

#[tokio::test]
async fn invitation_accept_retry_invite_user_and_leave_use_authenticated_sdk_operations() {
    let (server, _) = invited_server().await;
    let (sender, mut receiver, worker) = login(&server).await;
    Mock::given(method("POST"))
        .and(path_regex(".*/rooms/.*/join$"))
        .respond_with(
            ResponseTemplate::new(403)
                .set_body_json(json!({"errcode":"M_FORBIDDEN","error":"Denied"})),
        )
        .with_priority(1)
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(
        action(&sender, &mut receiver, RoomAction::Accept).await,
        Err("room-action-forbidden")
    );
    assert_eq!(
        action(
            &sender,
            &mut receiver,
            RoomAction::Invite("@bob:local".into())
        )
        .await,
        Err("room-membership-changed")
    );
    assert_eq!(
        action(&sender, &mut receiver, RoomAction::Accept).await,
        Ok(())
    );
    next(&mut receiver, |event| matches!(event, EventKind::Rooms(rooms) if rooms.iter().any(|room| room.membership == Membership::Joined))).await;
    assert_eq!(
        action(&sender, &mut receiver, RoomAction::Invite("bob".into())).await,
        Err("invite-invalid-user")
    );
    assert_eq!(
        action(
            &sender,
            &mut receiver,
            RoomAction::Invite(" @carol:local ".into())
        )
        .await,
        Ok(())
    );
    Mock::given(method("POST"))
        .and(path_regex(".*/rooms/.*/leave$"))
        .respond_with(
            ResponseTemplate::new(503)
                .set_body_json(json!({"errcode":"M_UNKNOWN","error":"Unavailable"})),
        )
        .with_priority(1)
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(
        action(&sender, &mut receiver, RoomAction::Leave).await,
        Err("room-action-failed")
    );
    assert_eq!(
        action(&sender, &mut receiver, RoomAction::Leave).await,
        Ok(())
    );
    next(
        &mut receiver,
        |event| matches!(event, EventKind::Rooms(rooms) if rooms.is_empty()),
    )
    .await;
    worker.abort();
    let requests = server.received_requests().await.unwrap();
    let invites: Vec<_> = requests
        .iter()
        .filter(|request| request.url.path().ends_with("/invite"))
        .collect();
    assert_eq!(invites.len(), 1);
    assert_eq!(
        invites[0].body_json::<serde_json::Value>().unwrap()["user_id"],
        "@carol:local"
    );
    assert_eq!(
        invites[0].headers.get("authorization").unwrap(),
        "Bearer test-access-token"
    );
}

#[tokio::test]
async fn decline_removes_invitation_without_loading_history_or_sending() {
    let (server, _) = invited_server().await;
    let (sender, mut receiver, worker) = login(&server).await;
    sender
        .send(Command::Select("!invite:local".into()))
        .await
        .unwrap();
    sender
        .send(Command::Send {
            room_id: "!invite:local".into(),
            body: "Must not send".into(),
            transaction: archaic_matrix::transaction_id(),
        })
        .await
        .unwrap();
    next(&mut receiver, |event| {
        matches!(event, EventKind::Error("send-failed"))
    })
    .await;
    assert_eq!(
        action(&sender, &mut receiver, RoomAction::Decline).await,
        Ok(())
    );
    next(
        &mut receiver,
        |event| matches!(event, EventKind::Rooms(rooms) if rooms.is_empty()),
    )
    .await;
    worker.abort();
    let requests = server.received_requests().await.unwrap();
    assert!(
        requests
            .iter()
            .any(|request| request.url.path().ends_with("/leave"))
    );
    assert!(
        !requests
            .iter()
            .any(|request| request.url.path().ends_with("/messages") || request.method == "PUT")
    );
}

#[tokio::test]
async fn externally_withdrawn_invitation_disappears_and_cannot_be_accepted() {
    let (server, state) = invited_server().await;
    let (sender, mut receiver, worker) = login(&server).await;
    state.store(2, Ordering::SeqCst);
    next(
        &mut receiver,
        |event| matches!(event, EventKind::Rooms(rooms) if rooms.is_empty()),
    )
    .await;
    assert_eq!(
        action(&sender, &mut receiver, RoomAction::Accept).await,
        Err("room-membership-changed")
    );
    worker.abort();
    assert!(
        !server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .any(|request| request.url.path().ends_with("/join"))
    );
}
