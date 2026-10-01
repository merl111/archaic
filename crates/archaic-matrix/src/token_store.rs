//! Synchronous rotation callbacks persist tokens before the SDK reports refresh completion.
use crate::storage::{Profile, SavedSession};
use matrix_sdk::Client;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
pub(crate) struct TokenStore {
    profile: Arc<Profile>,
    metadata: Mutex<Option<SavedSession>>,
    failed: AtomicBool,
    suspended: AtomicBool,
}
impl TokenStore {
    pub fn attach(
        client: &Client,
        profile: Arc<Profile>,
        record: SavedSession,
    ) -> Result<Arc<Self>, &'static str> {
        let store = Arc::new(Self {
            profile,
            metadata: Mutex::new(Some(record)),
            failed: AtomicBool::new(false),
            suspended: AtomicBool::new(false),
        });
        let save = Arc::downgrade(&store);
        let reload = Arc::downgrade(&store);
        client
            .set_session_callbacks(
                Box::new(move |_| {
                    let reload = reload
                        .upgrade()
                        .ok_or_else(|| std::io::Error::other("session closed"))?;
                    let guard = reload
                        .metadata
                        .lock()
                        .map_err(|_| std::io::Error::other("session lock"))?;
                    let current = guard
                        .as_ref()
                        .ok_or_else(|| std::io::Error::other("session closed"))?;
                    let saved = reload
                        .profile
                        .load()
                        .map_err(std::io::Error::other)?
                        .ok_or_else(|| std::io::Error::other("session unavailable"))?;
                    if saved.session.meta != current.session.meta {
                        return Err(std::io::Error::other("session identity mismatch").into());
                    }
                    Ok(saved.session.tokens)
                }),
                Box::new(move |client| {
                    let Some(save) = save.upgrade() else {
                        return Ok(());
                    };
                    save.save(&client)
                        .map_err(|s| std::io::Error::other(s).into())
                }),
            )
            .map_err(|_| "session-invalid")?;
        Ok(store)
    }
    pub fn suspend(&self) {
        self.suspended.store(true, Ordering::Release);
    }
    pub fn save(&self, client: &Client) -> Result<(), &'static str> {
        if self.suspended.load(Ordering::Acquire) {
            return Err("reauth-unavailable");
        }
        let mut guard = self.metadata.lock().map_err(|_| "storage-failed")?;
        let Some(record) = guard.as_mut() else {
            return Ok(());
        };
        let session = crate::oauth::session(client)?;
        if session.meta != record.session.meta {
            return Err("session-invalid");
        }
        record.session = session;
        let result = self.profile.save(Some(record));
        self.failed.store(result.is_err(), Ordering::Release);
        result
    }
    pub fn replace_tokens(
        &self,
        renewed: matrix_sdk::authentication::matrix::MatrixSession,
    ) -> Result<(), &'static str> {
        let mut guard = self.metadata.lock().map_err(|_| "storage-failed")?;
        let record = guard.as_mut().ok_or("session-invalid")?;
        if record.session.meta != renewed.meta {
            return Err("session-key-mismatch");
        }
        record.session = renewed;
        self.profile.save(Some(record))?;
        // Disable callbacks from the expired client before its tasks are dropped.
        *guard = None;
        Ok(())
    }
    pub fn local_storage(&self) -> Result<(Arc<Profile>, String), &'static str> {
        let guard = self.metadata.lock().map_err(|_| "storage-failed")?;
        Ok((
            self.profile.clone(),
            guard.as_ref().ok_or("session-invalid")?.store_id.clone(),
        ))
    }
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }
    pub fn close(&self) {
        *self.metadata.lock().unwrap_or_else(|p| p.into_inner()) = None;
    }
}
