//! Matrix-only application service. No GUI handles or callbacks cross this boundary.
mod connection;
pub mod storage;
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
    Join(String),
    Send {
        room_id: String,
        body: String,
        transaction: OwnedTransactionId,
    },
    Logout,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomSummary {
    pub id: String,
    pub name: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub id: String,
    pub sender: String,
    pub body: Option<String>,
}
#[derive(Debug)]
pub struct Event {
    pub epoch: u64,
    pub kind: EventKind,
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
    },
    Sent {
        transaction: OwnedTransactionId,
    },
    Joined(RoomSummary),
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
