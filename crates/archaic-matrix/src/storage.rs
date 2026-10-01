//! Encrypted single-account profile. Tokens never appear in a plaintext file.
use chacha20poly1305::{
    KeyInit, XChaCha20Poly1305,
    aead::{Aead, Payload},
};
use matrix_sdk::authentication::matrix::MatrixSession;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use zeroize::Zeroizing;

const HEADER: &[u8] = b"archaic-session-v1\0";
const MAX_RECORD: u64 = 1024 * 1024;
type Result<T> = std::result::Result<T, &'static str>;

/// Injectable credential boundary; tests never access the user's actual vault.
pub trait Vault: Send + Sync {
    fn load(&self) -> Result<Option<Vec<u8>>>;
    fn save(&self, key: &[u8]) -> Result<()>;
}
struct SystemVault(keyring::Entry);
impl Vault for SystemVault {
    fn load(&self) -> Result<Option<Vec<u8>>> {
        match self.0.get_secret() {
            Ok(key) => Ok(Some(key)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err("vault-unavailable"),
        }
    }
    fn save(&self, key: &[u8]) -> Result<()> {
        self.0.set_secret(key).map_err(|_| "vault-unavailable")
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct SavedSession {
    pub server: String,
    pub fingerprint: String,
    pub store_id: String,
    pub session: MatrixSession,
}

/// Owns the exclusive profile lock and a zeroizing copy of the vault key.
/// No Debug implementation: this object holds key material.
pub struct Profile {
    root: PathBuf,
    key: Zeroizing<Vec<u8>>,
    _lock: File,
}
impl Profile {
    pub fn system() -> Result<Self> {
        let root = profile_directory()?;
        let entry = keyring::Entry::new("org.archaic.desktop", "default-profile-v1")
            .map_err(|_| "vault-unavailable")?;
        Self::open(root, &SystemVault(entry))
    }

    /// Custom profile/vault injection for isolated tests and application hosts.
    pub fn open(root: PathBuf, vault: &dyn Vault) -> Result<Self> {
        private_directory(&root)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join("profile.lock"))
            .map_err(|_| "storage-failed")?;
        lock.try_lock().map_err(|_| "profile-in-use")?;
        let key = match vault.load()? {
            Some(key) if key.len() == 32 => Zeroizing::new(key),
            Some(_) => return Err("vault-key-invalid"),
            None => {
                // A missing vault key is not an invitation to overwrite an existing profile.
                if root.join("session.enc").exists() || root.join("stores").exists() {
                    return Err("vault-key-missing");
                }
                let mut key = Zeroizing::new(vec![0; 32]);
                getrandom::fill(&mut key).map_err(|_| "storage-failed")?;
                vault.save(&key)?;
                let verified = Zeroizing::new(vault.load()?.ok_or("vault-unavailable")?);
                if *verified != *key {
                    return Err("vault-unavailable");
                }
                key
            }
        };
        Ok(Self {
            root,
            key,
            _lock: lock,
        })
    }

    pub(crate) fn passphrase(&self) -> Zeroizing<String> {
        Zeroizing::new(hex(&self.key))
    }

    pub(crate) fn new_store(&self) -> Result<(String, PathBuf)> {
        let mut random = [0; 16];
        getrandom::fill(&mut random).map_err(|_| "storage-failed")?;
        let id = hex(&random);
        let path = self.store_path(&id)?;
        private_directory(&path)?;
        Ok((id, path))
    }

    pub(crate) fn store_path(&self, id: &str) -> Result<PathBuf> {
        if id.len() != 32 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("session-invalid");
        }
        Ok(self.root.join("stores").join(id))
    }

    pub(crate) fn load(&self) -> Result<Option<SavedSession>> {
        let file = match File::open(self.root.join("session.enc")) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("storage-failed"),
        };
        let mut bytes = Vec::new();
        file.take(MAX_RECORD + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "storage-failed")?;
        if bytes.len() as u64 > MAX_RECORD
            || !bytes.starts_with(HEADER)
            || bytes.len() < HEADER.len() + 24
        {
            return Err("session-invalid");
        }
        let (nonce, ciphertext) = bytes[HEADER.len()..].split_at(24);
        let cipher = XChaCha20Poly1305::new_from_slice(&self.key).map_err(|_| "session-invalid")?;
        let nonce: &[u8; 24] = nonce.try_into().map_err(|_| "session-invalid")?;
        let plain = Zeroizing::new(
            cipher
                .decrypt(
                    nonce.into(),
                    Payload {
                        msg: ciphertext,
                        aad: HEADER,
                    },
                )
                .map_err(|_| "session-invalid")?,
        );
        let saved: Option<SavedSession> =
            serde_json::from_slice(&plain).map_err(|_| "session-invalid")?;
        if let Some(saved) = &saved {
            crate::homeserver_url(&saved.server).map_err(|_| "session-invalid")?;
            // Never recreate an empty crypto store for an existing Matrix device.
            if !self
                .store_path(&saved.store_id)?
                .join("matrix-sdk-crypto.sqlite3")
                .is_file()
            {
                return Err("session-store-missing");
            }
        }
        Ok(saved)
    }

    pub(crate) fn save(&self, session: Option<&SavedSession>) -> Result<()> {
        let plain = Zeroizing::new(serde_json::to_vec(&session).map_err(|_| "storage-failed")?);
        if plain.len() as u64 + HEADER.len() as u64 + 40 > MAX_RECORD {
            return Err("storage-failed");
        }
        let mut nonce = [0; 24];
        getrandom::fill(&mut nonce).map_err(|_| "storage-failed")?;
        let cipher = XChaCha20Poly1305::new_from_slice(&self.key).map_err(|_| "storage-failed")?;
        let ciphertext = cipher
            .encrypt(
                (&nonce).into(),
                Payload {
                    msg: &plain,
                    aad: HEADER,
                },
            )
            .map_err(|_| "storage-failed")?;
        let mut file = tempfile::NamedTempFile::new_in(&self.root).map_err(|_| "storage-failed")?;
        file.write_all(HEADER)
            .and_then(|_| file.write_all(&nonce))
            .and_then(|_| file.write_all(&ciphertext))
            .and_then(|_| file.as_file().sync_all())
            .map_err(|_| "storage-failed")?;
        file.persist(self.root.join("session.enc"))
            .map_err(|_| "storage-failed")?;
        #[cfg(unix)]
        File::open(&self.root)
            .and_then(|file| file.sync_all())
            .map_err(|_| "storage-failed")?;
        Ok(())
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn private_directory(path: &Path) -> Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path).map_err(|_| "storage-failed")
}
fn profile_directory() -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    let path = std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("Archaic"));
    #[cfg(target_os = "macos")]
    let path = std::env::var_os("HOME")
        .map(|p| PathBuf::from(p).join("Library/Application Support/Archaic"));
    #[cfg(target_os = "linux")]
    let path = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/share")))
        .map(|p| p.join("archaic"));
    path.filter(|p| p.is_absolute()).ok_or("storage-failed")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    #[derive(Default)]
    struct MemoryVault(Mutex<Option<Vec<u8>>>);
    impl Vault for MemoryVault {
        fn load(&self) -> Result<Option<Vec<u8>>> {
            Ok(self.0.lock().unwrap().clone())
        }
        fn save(&self, key: &[u8]) -> Result<()> {
            *self.0.lock().unwrap() = Some(key.to_vec());
            Ok(())
        }
    }
    fn record(profile: &Profile) -> SavedSession {
        let (store_id, path) = profile.new_store().unwrap();
        // This unit test exercises the envelope; protocol tests use real SDK databases.
        File::create(path.join("matrix-sdk-crypto.sqlite3")).unwrap();
        SavedSession {
            server: "https://example.org".into(),
            store_id,
            fingerprint: "fingerprint".into(),
            session: serde_json::from_value(serde_json::json!({
                "user_id":"@alice:example.org", "device_id":"DEVICE", "access_token":"private-token"
            }))
            .unwrap(),
        }
    }
    #[test]
    fn encrypted_record_roundtrip_tamper_detection_and_logout_tombstone() {
        let directory = tempfile::tempdir().unwrap();
        let vault = MemoryVault::default();
        let profile = Profile::open(directory.path().into(), &vault).unwrap();
        let saved = record(&profile);
        profile.save(Some(&saved)).unwrap();
        let path = directory.path().join("session.enc");
        let original = fs::read(&path).unwrap();
        assert!(!original.windows(13).any(|bytes| bytes == b"private-token"));
        assert_eq!(profile.load().unwrap().unwrap().session, saved.session);
        profile.save(Some(&saved)).unwrap();
        assert_ne!(
            original,
            fs::read(&path).unwrap(),
            "fresh nonce on each write"
        );
        let mut damaged = original;
        *damaged.last_mut().unwrap() ^= 1;
        fs::write(&path, damaged).unwrap();
        assert!(matches!(profile.load(), Err("session-invalid")));
        profile.save(None).unwrap();
        assert!(profile.load().unwrap().is_none());
    }
    #[test]
    fn profile_lock_wrong_key_and_missing_key_fail_closed() {
        let directory = tempfile::tempdir().unwrap();
        let vault = MemoryVault::default();
        let profile = Profile::open(directory.path().into(), &vault).unwrap();
        assert!(matches!(
            Profile::open(directory.path().into(), &vault),
            Err("profile-in-use")
        ));
        profile.save(Some(&record(&profile))).unwrap();
        drop(profile);
        let missing = MemoryVault::default();
        assert!(matches!(
            Profile::open(directory.path().into(), &missing),
            Err("vault-key-missing")
        ));
        assert!(missing.load().unwrap().is_none());
        let wrong = MemoryVault(Mutex::new(Some(vec![42; 32])));
        let profile = Profile::open(directory.path().into(), &wrong).unwrap();
        assert!(matches!(profile.load(), Err("session-invalid")));
    }
    #[test]
    fn missing_crypto_database_and_unsafe_store_paths_are_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let profile = Profile::open(directory.path().into(), &MemoryVault::default()).unwrap();
        let saved = record(&profile);
        profile.save(Some(&saved)).unwrap();
        fs::remove_file(
            profile
                .store_path(&saved.store_id)
                .unwrap()
                .join("matrix-sdk-crypto.sqlite3"),
        )
        .unwrap();
        assert!(matches!(profile.load(), Err("session-store-missing")));
        for id in ["../outside", "/tmp/profile", "", "not-a-random-store-id"] {
            assert!(profile.store_path(id).is_err());
        }
    }
    #[test]
    fn unavailable_vault_does_not_write_session_or_store() {
        struct Locked;
        impl Vault for Locked {
            fn load(&self) -> Result<Option<Vec<u8>>> {
                Err("vault-unavailable")
            }
            fn save(&self, _: &[u8]) -> Result<()> {
                panic!("must not write to locked vault")
            }
        }
        let directory = tempfile::tempdir().unwrap();
        assert!(matches!(
            Profile::open(directory.path().into(), &Locked),
            Err("vault-unavailable")
        ));
        assert!(!directory.path().join("session.enc").exists());
        assert!(!directory.path().join("stores").exists());
    }
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "run tools/test_linux_vault.py to provide an isolated Secret Service"]
    fn system_vault_roundtrip() {
        let sandbox = PathBuf::from(
            std::env::var_os("ARCHAIC_TEST_VAULT_ROOT").expect("use test_linux_vault.py"),
        );
        assert!(sandbox.starts_with(std::env::temp_dir()));
        assert!(
            sandbox
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("archaic-vault-test-")
        );
        assert_eq!(
            std::env::var_os("XDG_DATA_HOME").unwrap(),
            sandbox.join("data")
        );
        let profile = Profile::system().unwrap();
        let saved = record(&profile);
        profile.save(Some(&saved)).unwrap();
        drop(profile);
        let reopened = Profile::system().unwrap();
        assert!(reopened.load().unwrap().unwrap().session == saved.session);
        reopened.save(None).unwrap();
        assert!(reopened.load().unwrap().is_none());
    }
}
