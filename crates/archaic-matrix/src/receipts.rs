//! Latest public read positions from the SDK store, never private receipts.
use crate::Message;
use matrix_sdk::{
    Room,
    ruma::{
        EventId,
        events::receipt::{ReceiptThread, ReceiptType},
    },
};

pub(crate) async fn populate(room: &Room, messages: &mut [Message]) {
    for message in messages.iter_mut() {
        let Ok(id) = EventId::parse(&message.id) else {
            continue;
        };
        let general = readers(room, &id, ReceiptThread::Unthreaded).await;
        message.read_by = Vec::new();
        if message.thread_root.is_none() {
            message.read_by = combine(
                general.clone(),
                readers(room, &id, ReceiptThread::Main).await,
            );
            for anchor in &message.receipt_events {
                if let Ok(anchor) = EventId::parse(anchor) {
                    let extra = combine(
                        readers(room, &anchor, ReceiptThread::Unthreaded).await,
                        readers(room, &anchor, ReceiptThread::Main).await,
                    );
                    message.read_by = combine(std::mem::take(&mut message.read_by), extra);
                }
            }
        }
        if let Ok(root) = EventId::parse(message.thread_root.as_deref().unwrap_or(&message.id)) {
            message.thread_read_by = combine(
                general,
                readers(room, &id, ReceiptThread::Thread(root.clone())).await,
            );
            for anchor in &message.thread_receipt_events {
                if let Ok(anchor) = EventId::parse(anchor) {
                    let extra = combine(
                        readers(room, &anchor, ReceiptThread::Unthreaded).await,
                        readers(room, &anchor, ReceiptThread::Thread(root.clone())).await,
                    );
                    message.thread_read_by =
                        combine(std::mem::take(&mut message.thread_read_by), extra);
                }
            }
        }
    }
    // Clients may switch between legacy unthreaded and thread-aware receipts.
    // Show a user once at their latest visible position in each timeline.
    let mut room_seen = std::collections::HashSet::new();
    let mut thread_seen = std::collections::HashSet::new();
    for message in messages.iter_mut().rev() {
        message.read_by.retain(|u| room_seen.insert(u.clone()));
        let root = message.thread_root.as_ref().unwrap_or(&message.id);
        message
            .thread_read_by
            .retain(|u| thread_seen.insert((root.clone(), u.clone())));
    }
}
async fn readers(room: &Room, event: &EventId, thread: ReceiptThread) -> Vec<String> {
    room.load_event_receipts(ReceiptType::Read, &thread, event)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(u, _)| u)
        .map(|u| u.to_string())
        .collect()
}
fn combine(mut a: Vec<String>, b: Vec<String>) -> Vec<String> {
    a.extend(b);
    a.sort();
    a.dedup();
    a.truncate(128);
    a
}

/// Resolve one thread as a single timeline so fallback and scoped receipts
/// cannot leave the same person on both the parent and a later reply.
pub(crate) async fn populate_details(room: &Room, details: &mut crate::MessageDetails) {
    let mut items = std::mem::take(&mut details.thread);
    items.insert(0, std::mem::take(&mut details.message));
    populate(room, &mut items).await;
    details.message = items.remove(0);
    details.thread = items;
}
