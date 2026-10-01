//! Matrix-only application service. No GUI handles or callbacks cross this boundary.
mod connection;
mod conversations;
mod history;
mod membership;
mod message_actions;
pub mod storage;
mod timeline;
mod worker;
use matrix_sdk::ruma::OwnedTransactionId;
pub use worker::{run, run_persistent, run_with_profile};
pub type TransactionId = OwnedTransactionId;
use tokio::sync::mpsc;
use url::Url;

pub const CHANNEL_CAPACITY: usize = 32;

// Deliberately not Debug: login commands contain a password.
pub enum Command {
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
    pub id: String,
    pub name: String,
    pub membership: Membership,
    pub inviter: Option<String>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Message {
    pub id: String,
    pub sender: String,
    pub body: Option<String>,
    pub editable: bool,
    pub deleted: bool,
    pub edited: bool,
    pub reply_to: Option<String>,
    pub reactions: Vec<Reaction>,
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
    Recent,
    Older,
    Latest,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HistoryInfo {
    pub can_load_more: bool,
    pub at_limit: bool,
    pub gap: bool,
}

#[derive(Debug)]
pub enum EventKind {
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
