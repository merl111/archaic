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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oauth_client_id: Option<String>,
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
        Self::system_named("default")
    }
    pub fn system_named(name: &str) -> Result<Self> {
        validate_profile_name(name)?;
        let root = if name == "default" {
            profile_directory()?
        } else {
            profile_directory()?.join("profiles").join(name)
        };
        let account = if name == "default" {
            "default-profile-v1".to_owned()
        } else {
            format!("profile-v1:{name}")
        };
        let entry = keyring::Entry::new("org.archaic.desktop", &account)
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

    pub(crate) fn read_data<T: serde::de::DeserializeOwned + Default>(
        &self,
        store: &str,
        name: &str,
    ) -> Result<T> {
        let (path, aad) = self.data_location(store, name)?;
        let file = match File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(T::default()),
            Err(_) => return Err("storage-failed"),
        };
        let mut bytes = Vec::new();
        file.take(64 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "storage-failed")?;
        if bytes.len() < 40 || bytes.len() > 64 * 1024 * 1024 {
            return Err("local-data-invalid");
        }
        let (nonce, ciphertext) = bytes.split_at(24);
        let nonce: &[u8; 24] = nonce.try_into().map_err(|_| "local-data-invalid")?;
        let cipher =
            XChaCha20Poly1305::new_from_slice(&self.key).map_err(|_| "local-data-invalid")?;
        let plain = Zeroizing::new(
            cipher
                .decrypt(
                    nonce.into(),
                    Payload {
                        msg: ciphertext,
                        aad: aad.as_bytes(),
                    },
                )
                .map_err(|_| "local-data-invalid")?,
        );
        serde_json::from_slice(&plain).map_err(|_| "local-data-invalid")
    }
    pub(crate) fn write_data<T: Serialize>(&self, store: &str, name: &str, data: &T) -> Result<()> {
        let (path, aad) = self.data_location(store, name)?;
        let plain = Zeroizing::new(serde_json::to_vec(data).map_err(|_| "storage-failed")?);
        if plain.len() > 64 * 1024 * 1024 - 40 {
            return Err("tool-limit");
        }
        let mut nonce = [0; 24];
        getrandom::fill(&mut nonce).map_err(|_| "storage-failed")?;
        let cipher = XChaCha20Poly1305::new_from_slice(&self.key).map_err(|_| "storage-failed")?;
        let bytes = cipher
            .encrypt(
                (&nonce).into(),
                Payload {
                    msg: &plain,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| "storage-failed")?;
        let parent = path.parent().ok_or("storage-failed")?;
        let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|_| "storage-failed")?;
        file.write_all(&nonce)
            .and_then(|_| file.write_all(&bytes))
            .and_then(|_| file.as_file().sync_all())
            .map_err(|_| "storage-failed")?;
        file.persist(&path).map_err(|_| "storage-failed")?;
        #[cfg(unix)]
        File::open(parent)
            .and_then(|f| f.sync_all())
            .map_err(|_| "storage-failed")?;
        Ok(())
    }
    fn data_location(&self, store: &str, name: &str) -> Result<(PathBuf, String)> {
        if name.is_empty()
            || name.len() > 64
            || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
        {
            return Err("storage-failed");
        }
        Ok((
            self.store_path(store)?.join(format!("{name}.enc")),
            format!("archaic-data-v1:{store}:{name}"),
        ))
    }

    pub(crate) fn archive_cipher(
        &self,
        store: &str,
    ) -> Result<matrix_sdk_store_encryption::StoreCipher> {
        use matrix_sdk_store_encryption::StoreCipher;
        let exported: Vec<u8> = self.read_data(store, "archive-key")?;
        if !exported.is_empty() {
            return StoreCipher::import_with_key(&self.key, &exported)
                .map_err(|_| "storage-failed");
        }
        let cipher = StoreCipher::new().map_err(|_| "storage-failed")?;
        let exported = cipher
            .export_with_key(&self.key)
            .map_err(|_| "storage-failed")?;
        self.write_data(store, "archive-key", &exported)?;
        Ok(cipher)
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

pub fn validate_profile_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 40
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err("profile-invalid");
    }
    Ok(())
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

/// Enumerate only this application's valid profile directories, without opening any vault.
pub fn profile_names() -> Result<Vec<String>> {
    let root = profile_directory()?.join("profiles");
    let mut names = vec!["default".to_owned()];
    let entries = match std::fs::read_dir(root) {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(names),
        Err(_) => return Err("storage-failed"),
    };
    for entry in entries.take(1024).flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name != "default"
            && validate_profile_name(&name).is_ok()
            && entry.file_type().is_ok_and(|t| t.is_dir())
        {
            names.push(name);
        }
    }
    names[1..].sort();
    Ok(names)
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
            oauth_client_id: None,
            session: serde_json::from_value(serde_json::json!({
                "user_id":"@alice:example.org", "device_id":"DEVICE", "access_token":"private-token"
            }))
            .unwrap(),
        }
    }
    #[test]
    fn encrypted_application_data_is_bound_to_store_and_kind() {
        let dir = tempfile::tempdir().unwrap();
        let vault = MemoryVault::default();
        let profile = Profile::open(dir.path().into(), &vault).unwrap();
        let first = record(&profile);
        let second = record(&profile);
        let data = vec!["private draft".to_owned()];
        profile
            .write_data(&first.store_id, "drafts", &data)
            .unwrap();
        assert_eq!(
            profile
                .read_data::<Vec<String>>(&first.store_id, "drafts")
                .unwrap(),
            data
        );
        let source = dir
            .path()
            .join("stores")
            .join(&first.store_id)
            .join("drafts.enc");
        let encrypted = fs::read(&source).unwrap();
        assert!(!encrypted.windows(13).any(|w| w == b"private draft"));
        let other = dir
            .path()
            .join("stores")
            .join(&second.store_id)
            .join("drafts.enc");
        fs::write(&other, &encrypted).unwrap();
        assert!(
            profile
                .read_data::<Vec<String>>(&second.store_id, "drafts")
                .is_err()
        );
        let mut damaged = encrypted;
        *damaged.last_mut().unwrap() ^= 1;
        fs::write(&source, damaged).unwrap();
        assert!(
            profile
                .read_data::<Vec<String>>(&first.store_id, "drafts")
                .is_err()
        );
        for name in ["", "../escape", "a/b", "name with spaces"] {
            assert!(validate_profile_name(name).is_err());
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
