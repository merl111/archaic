mod recovery;
use super::{next, server};
use archaic_matrix::{Command, EventKind, HistoryLoad, Receiver, Sender, channels};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use wiremock::{
    Mock, ResponseTemplate,
    matchers::{method, path_regex},
};

fn message(id: &str) -> Value {
    json!({"type":"m.room.message","sender":"@alice:local","event_id":format!("${id}"),
        "origin_server_ts":10,"content":{"msgtype":"m.text","body":id}})
}
async fn request(sender: &Sender, receiver: &mut Receiver, kind: HistoryLoad) -> EventKind {
    sender
        .send(Command::LoadHistory {
            room_id: "!studio:local".into(),
            kind,
        })
        .await
        .unwrap();
    next(receiver, |event| matches!(event,
        EventKind::History {kind:received,..} | EventKind::HistoryFailed {kind:received,..} if *received == kind)).await.kind
}
fn bodies(event: EventKind, expected: &[&str]) -> archaic_matrix::HistoryInfo {
    let EventKind::History { messages, info, .. } = event else {
        panic!("expected history")
    };
    assert_eq!(
        messages
            .iter()
            .map(|m| m.body.as_deref().unwrap())
            .collect::<Vec<_>>(),
        expected
    );
    info
}
#[tokio::test]
async fn pagination_retries_cursors_merges_sync_and_resets_latest_using_real_sdk() {
    let server = server().await;
    let older_calls = Arc::new(AtomicUsize::new(0));
    let oldest_calls = Arc::new(AtomicUsize::new(0));
    let advanced = Arc::new(AtomicBool::new(false));
    let counts = (older_calls.clone(), oldest_calls.clone(), advanced.clone());
    Mock::given(method("GET")).and(path_regex(".*/messages$"))
        .respond_with(move |req: &wiremock::Request| {
            assert_eq!(req.headers.get("authorization").unwrap(), "Bearer test-access-token");
            let query: std::collections::HashMap<_,_> = req.url.query_pairs().into_owned().collect();
            assert_eq!(query.get("dir").map(String::as_str), Some("b"));
            assert_eq!(query.get("limit").map(String::as_str), Some("50"));
            let (chunk, end) = match query.get("from").map(String::as_str) {
                None => (if counts.2.load(Ordering::SeqCst) { vec![message("third"),message("second")] }
                    else { vec![message("second"),message("first")] },Some("older")),
                Some("older") => {
                    match counts.0.fetch_add(1,Ordering::SeqCst) {
                        0 => return ResponseTemplate::new(503).set_body_json(json!({"errcode":"M_UNKNOWN","error":"temporary failure"})),
                        1 => (vec![message("oversized");51],Some("must-not-advance")),
                        2 => (vec![json!({"type":"m.room.message","content":{}})],Some("must-not-advance")),
                        _ => (vec![message("first"),message("zero")],Some("empty")),
                    }
                },
                Some("empty") => (vec![],Some("oldest")),
                Some("oldest") => {
                    if counts.1.fetch_add(1,Ordering::SeqCst)==0 { (vec![message("discarded")],Some("oldest")) }
                    else { (vec![message("start")],None) }
                },
                _ => panic!("unexpected pagination cursor"),
            };
            let mut body=json!({"start":query.get("from").map(String::as_str).unwrap_or("latest"),"chunk":chunk,"state":[]});
            if let Some(end)=end {body["end"]=json!(end);}
            ResponseTemplate::new(200).set_body_json(body)
        }).with_priority(1).mount(&server).await;
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
        |e| matches!(e,EventKind::Rooms(rooms) if !rooms.is_empty()),
    )
    .await;
    sender
        .send(Command::Select("!studio:local".into()))
        .await
        .unwrap();
    let initial = next(&mut receiver, |e| matches!(e, EventKind::History { .. }))
        .await
        .kind;
    assert!(bodies(initial, &["first", "second"]).can_load_more);
    assert!(matches!(
        request(&sender, &mut receiver, HistoryLoad::Older).await,
        EventKind::HistoryFailed {
            kind: HistoryLoad::Older,
            key: "history-load-failed",
            ..
        }
    ));
    for _ in 0..2 {
        assert!(matches!(
            request(&sender, &mut receiver, HistoryLoad::Older).await,
            EventKind::HistoryFailed {
                key: "history-invalid-page",
                ..
            }
        ));
    }
    assert!(
        bodies(
            request(&sender, &mut receiver, HistoryLoad::Older).await,
            &["zero", "first", "second"]
        )
        .can_load_more
    );
    assert_eq!(older_calls.load(Ordering::SeqCst), 4);
    advanced.store(true, Ordering::SeqCst);
    sender.send(Command::Refresh).await.unwrap();
    let updated=next(&mut receiver,|e|matches!(e,EventKind::History{messages,..} if messages.last().is_some_and(|m|m.body.as_deref()==Some("third")))).await.kind;
    bodies(updated, &["zero", "first", "second", "third"]);
    assert!(
        bodies(
            request(&sender, &mut receiver, HistoryLoad::Older).await,
            &["zero", "first", "second", "third"]
        )
        .can_load_more
    );
    assert!(matches!(
        request(&sender, &mut receiver, HistoryLoad::Older).await,
        EventKind::HistoryFailed {
            key: "history-stalled",
            ..
        }
    ));
    assert!(
        !bodies(
            request(&sender, &mut receiver, HistoryLoad::Older).await,
            &["start", "zero", "first", "second", "third"]
        )
        .can_load_more
    );
    assert!(
        !bodies(
            request(&sender, &mut receiver, HistoryLoad::Older).await,
            &["start", "zero", "first", "second", "third"]
        )
        .can_load_more
    );
    assert_eq!(oldest_calls.load(Ordering::SeqCst), 2);
    assert!(
        bodies(
            request(&sender, &mut receiver, HistoryLoad::Latest).await,
            &["second", "third"]
        )
        .can_load_more
    );
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}
