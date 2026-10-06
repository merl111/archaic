#![recursion_limit = "256"]
//! Matrix-only application service. No GUI handles or callbacks cross this boundary.
mod archive;
mod archive_sync;
mod attachments;
mod avatars;
mod composer_extras;
mod connection;
mod conversations;
mod details;
mod history;
pub mod links;
mod local_data;
mod member_profile;
mod membership;
pub mod mentions;
mod message_actions;
mod oauth;
pub mod organization;
pub mod outbox;
mod paging;
mod reauthorize;
mod receipts;
pub mod room_settings;
mod room_threads;
mod room_tools;
mod search;
mod search_index;
pub mod security;
pub mod storage;
mod sync_activity;
mod timeline;
mod token_store;
mod transfers;
mod worker;
use matrix_sdk::ruma::OwnedTransactionId;
pub use worker::{run, run_persistent, run_persistent_named, run_with_profile};
pub type TransactionId = OwnedTransactionId;
use tokio::sync::mpsc;
use url::Url;

pub const CHANNEL_CAPACITY: usize = 32;

// Deliberately not Debug: login commands contain a password.
pub enum Command {
    MemberProfile {
        room: String,
        user: String,
    },
    Barrier(tokio::sync::oneshot::Sender<()>),
    Shutdown {
        epoch: u64,
        drafts: std::collections::HashMap<String, String>,
        result: tokio::sync::oneshot::Sender<Result<(), &'static str>>,
    },
    SwitchProfile {
        epoch: u64,
        name: String,
    },
    Organization {
        id: TransactionId,
        action: organization::Action,
    },
    SaveDraft {
        room_id: String,
        body: String,
    },
    Context {
        room_id: String,
        event_id: String,
    },
    Typing {
        room_id: String,
        active: bool,
    },
    BrowserLogin {
        epoch: u64,
        server: String,
    },
    OAuthLogin {
        epoch: u64,
        server: String,
    },
    Reauthorize {
        oauth: bool,
    },
    Reauthenticate {
        password: security::Secret,
    },
    CancelLogin,
    Enqueue {
        room_id: String,
        body: String,
        transaction: TransactionId,
    },
    EnqueueThread {
        room_id: String,
        root: String,
        latest: String,
        body: String,
        transaction: String,
    },
    OutboxAction {
        room_id: String,
        id: String,
        retry: bool,
    },
    Security {
        id: TransactionId,
        action: security::Action,
    },
    DiscardUpload,
    CancelTransfer {
        id: TransactionId,
    },
    Restore {
        epoch: u64,
    },
    Temporary {
        epoch: u64,
    },
    Login {
        epoch: u64,
        server: String,
        username: String,
        password: String,
    },
    Select(String),
    Refresh,
    LoadHistory {
        room_id: String,
        kind: HistoryLoad,
    },
    Join(String),
    JoinVia {
        address: String,
        via: Vec<String>,
        event: Option<String>,
    },
    NewConversation(NewConversation),
    RoomAction {
        room_id: String,
        action: RoomAction,
    },
    Send {
        room_id: String,
        body: String,
        transaction: OwnedTransactionId,
    },
    MessageAction(MessageRequest),
    QueueMessageAction(MessageRequest),
    RoomTool(ToolRequest),
    Logout,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NewConversation {
    Direct(String),
    Group {
        name: String,
        topic: String,
        invitees: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Membership {
    Joined,
    Invited,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoomAction {
    Accept,
    Decline,
    Leave,
    Invite(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomSummary {
    pub info: RoomInfo,
    pub id: String,
    pub name: String,
    pub membership: Membership,
    pub inviter: Option<String>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RoomInfo {
    pub preview: Option<RoomPreview>,
    pub children: Vec<String>,
    pub favourite: bool,
    pub recent: u64,
    pub direct: bool,
    pub members_known: bool,
    pub topic: String,
    pub members: u64,
    pub encrypted: Option<bool>,
    pub space: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RoomPreview {
    pub event: String,
    pub sender: String,
    pub body: String,
    pub encrypted: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Message {
    pub transaction_id: Option<String>,
    /// Hidden events following this message, used to place public read positions.
    #[doc(hidden)]
    pub receipt_events: Vec<String>,
    #[doc(hidden)]
    pub thread_receipt_events: Vec<String>,
    pub read_by: Vec<String>,
    pub thread_read_by: Vec<String>,
    pub timestamp: u64,
    pub thread_root: Option<String>,
    pub id: String,
    pub sender: String,
    pub body: Option<String>,
    pub formatted_body: Option<String>,
    pub undecrypted: bool,
    pub editable: bool,
    pub deleted: bool,
    pub edited: bool,
    pub reply_to: Option<String>,
    pub reactions: Vec<Reaction>,
    pub attachment: Option<Attachment>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attachment {
    pub name: String,
    pub kind: String,
    pub size: Option<u64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchMode {
    Indexed,
    Server,
    Local,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolAction {
    Sticker {
        path: std::path::PathBuf,
    },
    Voice {
        path: std::path::PathBuf,
        duration_ms: u64,
    },
    Poll {
        question: String,
        answers: Vec<String>,
    },
    QueueUpload {
        path: std::path::PathBuf,
    },
    Preview {
        event_id: String,
    },
    Upload {
        path: std::path::PathBuf,
    },
    Download {
        event_id: String,
        path: std::path::PathBuf,
    },
    MarkThreadRead {
        event_id: String,
        private: bool,
    },
    MarkRead {
        event_id: String,
        private: bool,
    },
    Search {
        query: String,
        mode: SearchMode,
        more: bool,
    },
    Details {
        event_id: String,
        more: bool,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolRequest {
    pub room_id: String,
    pub id: TransactionId,
    pub action: ToolAction,
}
#[derive(Clone, Debug, Default)]
pub struct SearchPage {
    pub truncated: bool,
    pub unavailable: usize,
    pub messages: Vec<Message>,
    pub more: bool,
    pub scanned: usize,
}
#[derive(Clone, Debug)]
pub struct MessageDetails {
    pub thread: Vec<Message>,
    pub unavailable: usize,
    pub message: Message,
    pub reply: Option<Message>,
    pub reply_unavailable: bool,
    pub revisions: Vec<String>,
    pub read_by: Vec<String>,
    pub thread_read_by: Vec<String>,
    pub more: bool,
    pub loaded: usize,
}
#[derive(Clone, Debug)]
pub struct ImagePreview {
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
}
#[derive(Clone, Debug)]
pub enum ToolData {
    QueuedUpload,
    Preview {
        pixels: Vec<u8>,
        width: u32,
        height: u32,
    },
    Uploaded,
    Downloaded,
    Read,
    Search(SearchPage),
    Details(Box<MessageDetails>),
}
/// One loaded reaction event. Counts are deduplicated by sender in the view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reaction {
    pub id: String,
    pub sender: String,
    pub key: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MessageAction {
    Thread(String),
    Forward(String),
    Reply(String),
    Edit(String),
    Delete,
    React(String),
    RemoveReactions { key: String, ids: Vec<String> },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageRequest {
    pub room_id: String,
    pub event_id: String,
    pub action: MessageAction,
    pub transaction: TransactionId,
}
#[derive(Debug)]
pub struct Event {
    pub epoch: u64,
    pub kind: EventKind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryLoad {
    Newer,
    Recent,
    Older,
    Latest,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HistoryInfo {
    pub can_load_newer: bool,
    pub can_load_more: bool,
    pub at_limit: bool,
    pub gap: bool,
}

#[derive(Clone, Debug, Default)]
pub struct HistorySyncStatus {
    pub rooms: usize,
    pub complete: usize,
    pub events: u64,
    pub encrypted: u64,
    pub retrying: usize,
}
#[derive(Debug)]
pub enum EventKind {
    HistorySync(HistorySyncStatus),
    MemberProfile(MemberProfile),
    DisplayName {
        key: String,
        name: String,
    },
    ImagePreview {
        room_id: String,
        event_id: String,
        result: Result<ImagePreview, &'static str>,
    },
    Avatar {
        key: String,
        pixels: Option<Vec<u8>>,
    },
    MemberCount {
        room_id: String,
        count: u64,
    },
    LiveDetailsFailed {
        room_id: String,
        key: &'static str,
    },
    Blocked(std::collections::HashSet<String>),
    DraftSaved {
        room_id: String,
        body: String,
    },
    Notification {
        room_id: String,
        name: String,
        event_id: String,
    },
    Organization {
        id: TransactionId,
        result: Result<organization::Page, &'static str>,
    },
    TransferProgress {
        room_id: String,
        id: TransactionId,
        current: u64,
        total: u64,
    },
    LiveDetails {
        room_id: String,
        details: Box<MessageDetails>,
    },
    Drafts(std::collections::HashMap<String, String>),
    ReauthenticationRequired,
    Activity {
        room_id: String,
        unread: u64,
        latest: Option<String>,
        typing: Option<Vec<String>>,
    },
    BrowserUrl(String),
    Queued {
        transaction: TransactionId,
    },
    Outbox(Vec<outbox::Item>),
    ThreadQueued {
        transaction: String,
        result: Result<(), &'static str>,
    },
    Delivery {
        room_id: String,
        transaction: String,
        event_id: Option<String>,
    },
    SecuritySnapshot(security::Snapshot),
    SecurityResult {
        id: TransactionId,
        result: Result<Option<security::Secret>, &'static str>,
    },
    RoomToolResult {
        request: ToolRequest,
        result: Result<ToolData, &'static str>,
    },
    Connected(String),
    SessionSaved(bool),
    RestoreBlocked(&'static str),
    Rooms(Vec<RoomSummary>),
    History {
        room_id: String,
        messages: Vec<Message>,
        info: HistoryInfo,
        kind: HistoryLoad,
    },
    HistoryFailed {
        room_id: String,
        kind: HistoryLoad,
        key: &'static str,
    },
    Sent {
        transaction: OwnedTransactionId,
    },
    MessageActionResult {
        request: MessageRequest,
        result: Result<(), &'static str>,
    },
    Joined(RoomSummary),
    ConversationOpened {
        room: RoomSummary,
        warning: Option<&'static str>,
    },
    CreateFailed(&'static str),
    RoomActionResult {
        room_id: String,
        action: RoomAction,
        result: Result<(), &'static str>,
    },
    JoinFailed(&'static str),
    SignedOut,
    Status(&'static str),
    Error(&'static str),
}

pub type Sender = mpsc::Sender<Command>;
pub type Receiver = mpsc::Receiver<Event>;

pub fn channels() -> (
    Sender,
    mpsc::Receiver<Command>,
    mpsc::Sender<Event>,
    Receiver,
) {
    let (commands, command_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let (events, event_rx) = mpsc::channel(CHANNEL_CAPACITY);
    (commands, command_rx, events, event_rx)
}

pub fn transaction_id() -> OwnedTransactionId {
    matrix_sdk::ruma::TransactionId::new()
}

pub fn homeserver_url(input: &str) -> Result<Url, &'static str> {
    let url = Url::parse(input.trim()).map_err(|_| "invalid-server")?;
    let loopback = url
        .host_str()
        .is_some_and(|host| host == "localhost" || host == "127.0.0.1" || host == "[::1]");
    if !(url.scheme() == "https" || url.scheme() == "http" && loopback)
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("invalid-server");
    }
    Ok(url)
}

/// Construct links from validated protocol identifiers, never by interpreting event bodies.
pub fn matrix_event_link(room: &str, event: &str) -> Result<String, &'static str> {
    use matrix_sdk::ruma::{EventId, RoomId};
    let room = RoomId::parse(room).map_err(|_| "link-invalid")?;
    let event = EventId::parse(event).map_err(|_| "link-invalid")?;
    Ok(room.matrix_to_event_uri(event).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unsafe_homeserver_urls() {
        for url in [
            "http://matrix.org",
            "file:///etc/passwd",
            "https://alice:secret@example.com",
            "https://example.com?token=secret",
            "https://example.com/#fragment",
            "matrix.org",
        ] {
            assert!(homeserver_url(url).is_err(), "{url}");
        }
        for url in [
            "https://matrix.org",
            "http://127.0.0.1:1234",
            "http://[::1]:8008",
        ] {
            assert!(homeserver_url(url).is_ok());
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct MemberProfile {
    pub room: String,
    pub user: String,
    pub name: String,
    pub role: String,
    pub verified: bool,
    pub receipt: Option<String>,
    pub loaded: bool,
}
