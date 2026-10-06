//! Local echoes and per-message delivery state. The SDK owns retries and persistence.
use crate::model::Model;
use archaic_matrix::{Message, outbox::Item};

impl Model {
    pub fn update_outbox(&mut self, items: Vec<Item>) {
        for mut item in items {
            if let Some(old) = self
                .outbox
                .iter_mut()
                .find(|v| v.room_id == item.room_id && v.id == item.id)
            {
                // A delayed snapshot must never turn an acknowledged send back into sending.
                if old.event_id.is_some() {
                    continue;
                }
                if let (Some(old_message), Some(message)) = (&old.message, &mut item.message) {
                    message.timestamp = old_message.timestamp;
                }
                *old = item;
            } else {
                self.outbox.push(item);
            }
        }
    }
    pub fn delivery_result(&mut self, room: &str, transaction: &str, event: Option<String>) {
        if let Some(event) = event {
            if let Some(item) = self
                .outbox
                .iter_mut()
                .find(|i| i.room_id == room && i.id == transaction)
            {
                item.event_id = Some(event);
                item.failed = false;
            }
            // Sent local echoes bridge the gap until sync returns their event IDs.
            // Keep a bounded recent set; persisted pending requests are never evicted.
            let mut sent = 0;
            for item in self.outbox.iter().rev() {
                if item.event_id.is_some() {
                    sent += 1;
                }
            }
            self.outbox.retain(|i| {
                if i.event_id.is_some() && sent > 256 {
                    sent -= 1;
                    false
                } else {
                    true
                }
            });
        } else {
            self.outbox
                .retain(|i| i.room_id != room || i.id != transaction);
        }
    }
    pub fn delivery_for(&self, message: &str) -> Option<&Item> {
        self.delivery_matching(message, self.messages.iter().find(|m| m.id == message))
    }
    pub fn delivery_for_message(&self, message: &Message) -> Option<&Item> {
        self.delivery_matching(&message.id, Some(message))
    }
    fn delivery_matching(&self, message: &str, source: Option<&Message>) -> Option<&Item> {
        self.outbox
            .iter()
            .filter(|i| Some(&i.room_id) == self.selected.as_ref())
            .filter(|i| {
                i.target.as_deref() == Some(message)
                    || i.message.as_ref().is_some_and(|m| m.id == message)
                    || i.event_id.as_deref() == Some(message)
                    || source.is_some_and(|m| {
                        m.transaction_id.as_ref() == Some(&i.id)
                            || m.reactions.iter().any(|r| Some(&r.id) == i.target.as_ref())
                    })
            })
            .max_by_key(|i| (i.failed, i.event_id.is_none()))
    }
    pub fn with_local_echoes(&self, items: &[Message], thread: Option<&str>) -> Vec<Message> {
        let mut messages = items.to_vec();
        for item in self
            .outbox
            .iter()
            .filter(|i| Some(&i.room_id) == self.selected.as_ref())
        {
            let Some(message) = &item.message else {
                continue;
            };
            if message.thread_root.as_deref() != thread {
                continue;
            }
            if messages.iter().any(|m| {
                m.transaction_id.as_ref() == Some(&item.id)
                    || Some(&m.id) == item.event_id.as_ref()
                    || m.id == message.id
            }) {
                continue;
            }
            let mut message = message.clone();
            if let Some(id) = &item.event_id {
                message.id = id.clone();
            }
            messages.push(message);
        }
        messages
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn queued() -> Item {
        Item {
            room_id: "!r:x".into(),
            id: "txn".into(),
            body: "Hello".into(),
            failed: false,
            message: Some(Message {
                id: "~txn".into(),
                sender: "@me:x".into(),
                body: Some("Hello".into()),
                transaction_id: Some("txn".into()),
                timestamp: 12,
                ..Default::default()
            }),
            target: None,
            reaction: None,
            event_id: None,
        }
    }
    #[test]
    fn echo_survives_ack_gap_and_deduplicates_against_sync() {
        let mut m = Model {
            selected: Some("!r:x".into()),
            ..Default::default()
        };
        m.update_outbox(vec![queued()]);
        assert_eq!(m.with_local_echoes(&[], None)[0].id, "~txn");
        m.update_outbox(vec![]);
        assert_eq!(m.with_local_echoes(&[], None).len(), 1);
        m.delivery_result("!r:x", "txn", Some("$sent".into()));
        m.update_outbox(vec![queued()]);
        let echoed = m.with_local_echoes(&[], None);
        assert_eq!(echoed[0].id, "$sent");
        assert_eq!(m.with_local_echoes(&echoed, None).len(), 1);
        let remote = Message {
            id: "$remote".into(),
            transaction_id: Some("txn".into()),
            ..Default::default()
        };
        assert_eq!(m.with_local_echoes(&[remote], None).len(), 1);
    }
    #[test]
    fn failure_retry_cancel_and_reaction_are_bound_to_the_message() {
        let mut m = Model {
            selected: Some("!r:x".into()),
            ..Default::default()
        };
        let mut item = queued();
        item.failed = true;
        m.update_outbox(vec![item.clone()]);
        assert!(m.delivery_for("~txn").unwrap().failed);
        item.failed = false;
        m.update_outbox(vec![item]);
        assert!(!m.delivery_for("~txn").unwrap().failed);
        m.delivery_result("!r:x", "txn", None);
        assert!(m.with_local_echoes(&[], None).is_empty());
        let mut reaction = queued();
        reaction.message = None;
        reaction.target = Some("$original".into());
        reaction.reaction = Some("👍".into());
        m.update_outbox(vec![reaction]);
        assert!(m.with_local_echoes(&[], None).is_empty());
        assert_eq!(
            m.delivery_for("$original").unwrap().reaction.as_deref(),
            Some("👍")
        );
        m.selected = Some("!other:x".into());
        assert!(m.delivery_for("$original").is_none());
    }
    #[test]
    fn pending_thread_reply_stays_in_its_thread() {
        let mut m = Model {
            selected: Some("!r:x".into()),
            ..Default::default()
        };
        let mut item = queued();
        item.message.as_mut().unwrap().thread_root = Some("$root".into());
        m.update_outbox(vec![item]);
        assert!(m.with_local_echoes(&[], None).is_empty());
        assert_eq!(m.with_local_echoes(&[], Some("$root")).len(), 1);
        assert!(m.with_local_echoes(&[], Some("$other")).is_empty());
    }
}
