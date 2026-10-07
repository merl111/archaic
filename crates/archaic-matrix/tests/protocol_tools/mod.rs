use super::{next, server};
use archaic_matrix::{
    Command, EventKind, Receiver, SearchMode, Sender, ToolAction, ToolData, ToolRequest,
    transaction_id,
};
use serde_json::{Value, json};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path_regex, query_param},
};
mod composer;
mod media;
mod relations;
mod search;
async fn login(server: &MockServer) -> (Sender, Receiver, tokio::task::JoinHandle<()>) {
    let (sender, commands, events, mut receiver) = archaic_matrix::channels();
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
fn request(action: ToolAction) -> ToolRequest {
    ToolRequest {
        room_id: "!studio:local".into(),
        id: transaction_id(),
        action,
    }
}
async fn perform(
    sender: &Sender,
    receiver: &mut Receiver,
    request: &ToolRequest,
) -> Result<ToolData, &'static str> {
    sender
        .send(Command::RoomTool(request.clone()))
        .await
        .unwrap();
    let event = next(
        receiver,
        |e| matches!(e,EventKind::RoomToolResult{request:r,..} if r==request),
    )
    .await;
    let EventKind::RoomToolResult { result, .. } = event.kind else {
        unreachable!()
    };
    result
}
fn event(id: &str, kind: &str, content: Value, time: u64) -> Value {
    json!({"event_id":id,"room_id":"!studio:local","sender":"@alice:local","type":kind,"content":content,"origin_server_ts":time})
}
async fn target(server: &MockServer, value: Value) {
    Mock::given(method("GET"))
        .and(path_regex(format!(
            ".*/event/.*{}$",
            &value["event_id"].as_str().unwrap()[1..]
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(value))
        .mount(server)
        .await;
}
fn assert_error(result: Result<ToolData, &'static str>, expected: &str) {
    assert!(
        matches!(result,Err(key) if key==expected),
        "unexpected tool result, expected {expected}"
    );
}
