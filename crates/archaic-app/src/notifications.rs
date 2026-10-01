//! Bounded native delivery and activation; no Matrix calls or CUI handles leave the GUI thread.
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
    mpsc,
};
struct Notice {
    generation: u64,
    body: String,
    open: crate::instance::Open,
}
pub struct Notifications {
    sender: mpsc::SyncSender<Notice>,
    generation: Arc<AtomicU64>,
    activations: mpsc::Receiver<(u64, crate::instance::Open)>,
}
impl Notifications {
    pub fn new() -> Self {
        let generation = Arc::new(AtomicU64::new(0));
        let (sender, receiver) = mpsc::sync_channel::<Notice>(8);
        let receiver = Arc::new(Mutex::new(receiver));
        let (activate, activations) = mpsc::sync_channel(8);
        // Fixed workers cap outstanding native handles even when an OS ignores expiry.
        for _ in 0..4 {
            let (receiver, current, activate) =
                (receiver.clone(), generation.clone(), activate.clone());
            std::thread::spawn(move || {
                loop {
                    let request = receiver.lock().expect("notification receiver").recv();
                    let Ok(notice) = request else {
                        break;
                    };
                    if notice.generation != current.load(Ordering::Acquire) {
                        continue;
                    }
                    let body = notice
                        .body
                        .replace('&', "&amp;")
                        .replace('<', "&lt;")
                        .replace('>', "&gt;");
                    let shown = notify_rust::Notification::new()
                        .appname("Archaic")
                        .summary("Archaic")
                        .body(&body)
                        .action("default", "Archaic")
                        .timeout(10000)
                        .show();
                    if let Ok(handle) = shown {
                        let route = |action: bool| {
                            if action && notice.generation == current.load(Ordering::Acquire) {
                                let _ = activate.try_send((notice.generation, notice.open.clone()));
                            }
                        };
                        #[cfg(all(unix, not(target_os = "macos")))]
                        if let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build()
                        {
                            runtime.block_on(async {
                                let _=tokio::time::timeout(std::time::Duration::from_secs(60),handle.wait_for_action_async(|response|{
                                    route(matches!(response,notify_rust::NotificationResponse::Default) || matches!(response,notify_rust::NotificationResponse::Action(action) if action=="default"));
                                })).await;
                                let _=tokio::time::timeout(std::time::Duration::from_secs(2),handle.close_async()).await;
                            });
                        }
                        #[cfg(any(target_os = "macos", target_os = "windows"))]
                        handle.wait_for_action(|action| route(action == "default"));
                    }
                }
            });
        }
        Self {
            sender,
            generation,
            activations,
        }
    }
    pub fn invalidate(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
    }
    pub fn send(&self, body: String, open: crate::instance::Open) {
        let _ = self.sender.try_send(Notice {
            generation: self.generation.load(Ordering::Acquire),
            body,
            open,
        });
    }
    pub fn activation(&self) -> Option<crate::instance::Open> {
        while let Ok((generation, open)) = self.activations.try_recv() {
            if generation == self.generation.load(Ordering::Acquire) {
                return Some(open);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn activation_discards_prior_account_generation() {
        let (sender, _) = mpsc::sync_channel(1);
        let (tx, activations) = mpsc::sync_channel(3);
        let n = Notifications {
            sender,
            activations,
            generation: Arc::new(AtomicU64::new(0)),
        };
        let open = crate::instance::Open {
            profile: Some("work".into()),
            link: Some("matrix:roomid/studio:local/e/message".into()),
        };
        tx.send((0, open.clone())).unwrap();
        n.invalidate();
        assert!(n.activation().is_none());
        tx.send((1, open.clone())).unwrap();
        let result = n.activation().unwrap();
        assert_eq!(result.profile, open.profile);
        assert_eq!(result.link, open.link);
    }
    #[test]
    #[ignore = "requires private notification bus; use tools/test_notifications.py"]
    fn native_notification_activation() {
        let n = Notifications::new();
        n.send(
            "Room <unsafe> & label".into(),
            crate::instance::Open {
                profile: Some("work".into()),
                link: Some("matrix:roomid/studio:local/e/message".into()),
            },
        );
        let until = std::time::Instant::now() + std::time::Duration::from_secs(12);
        while std::time::Instant::now() < until {
            if let Some(open) = n.activation() {
                assert_eq!(open.profile.as_deref(), Some("work"));
                assert_eq!(
                    open.link.as_deref(),
                    Some("matrix:roomid/studio:local/e/message")
                );
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        panic!("notification action did not reach the account/event route");
    }
}
