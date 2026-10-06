//! Global appearance and account-scoped notification preferences (no secrets).
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::Path,
};
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Profiles {
    #[serde(default)]
    compact_messages: bool,
    #[serde(default)]
    text_size: u8,
    notifications: BTreeMap<String, bool>,
}
fn read(path: &Path) -> std::io::Result<Profiles> {
    let mut bytes = Vec::new();
    match std::fs::File::open(path) {
        Ok(file) => {
            file.take(65537).read_to_end(&mut bytes)?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Profiles::default()),
        Err(e) => return Err(e),
    }
    if bytes.len() > 65536 {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    serde_json::from_slice(&bytes).map_err(std::io::Error::other)
}
pub fn notifications(path: &Path, profile: &str) -> bool {
    read(path)
        .ok()
        .and_then(|s| s.notifications.get(profile).copied())
        .unwrap_or(false)
}
pub fn save_notifications(path: &Path, profile: &str, enabled: bool) -> std::io::Result<()> {
    archaic_matrix::storage::validate_profile_name(profile).map_err(std::io::Error::other)?;
    let mut settings = read(path)?;
    if settings.notifications.len() >= 1024 && !settings.notifications.contains_key(profile) {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    settings.notifications.insert(profile.into(), enabled);
    write(path, &settings)
}
pub fn compact_messages(path: &Path) -> bool {
    read(path).map(|p| p.compact_messages).unwrap_or(false)
}
pub fn save_compact_messages(path: &Path, compact: bool) -> std::io::Result<()> {
    let mut settings = read(path)?;
    settings.compact_messages = compact;
    write(path, &settings)
}
pub fn text_size(path: &Path) -> u8 {
    read(path).map(|p| p.text_size.min(4)).unwrap_or(0)
}
pub fn save_text_size(path: &Path, size: u8) -> std::io::Result<()> {
    if size > 4 {
        return Err(std::io::ErrorKind::InvalidInput.into());
    }
    let mut settings = read(path)?;
    settings.text_size = size;
    write(path, &settings)
}
fn write(path: &Path, settings: &Profiles) -> std::io::Result<()> {
    let parent = path.parent().ok_or(std::io::ErrorKind::InvalidInput)?;
    std::fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(&serde_json::to_vec(&settings)?)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|e| e.error)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_size_persists_and_validates_without_erasing_other_preferences() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("desktop.json");
        assert_eq!(text_size(&path), 0);
        save_notifications(&path, "work", true).unwrap();
        save_compact_messages(&path, true).unwrap();
        save_text_size(&path, 4).unwrap();
        assert_eq!(text_size(&path), 4);
        assert!(notifications(&path, "work"));
        assert!(compact_messages(&path));
        assert!(save_text_size(&path, 5).is_err());
        assert_eq!(text_size(&path), 4);
        std::fs::write(&path, br#"{"notifications":{}}"#).unwrap();
        assert_eq!(text_size(&path), 0); // Existing settings migrate without a rewrite.
        std::fs::write(&path, b"corrupt").unwrap();
        assert!(save_text_size(&path, 1).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"corrupt");
    }
    #[test]
    fn message_layout_persists_without_changing_account_notifications() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("desktop.json");
        save_notifications(&path, "work", true).unwrap();
        assert!(!compact_messages(&path));
        save_compact_messages(&path, true).unwrap();
        assert!(compact_messages(&path));
        assert!(notifications(&path, "work"));
        save_notifications(&path, "home", false).unwrap();
        assert!(compact_messages(&path));
    }
    #[test]
    fn notification_settings_are_account_scoped_and_corruption_is_not_overwritten() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("desktop.json");
        assert!(!notifications(&path, "default"));
        save_notifications(&path, "work", true).unwrap();
        assert!(notifications(&path, "work"));
        assert!(!notifications(&path, "default"));
        std::fs::write(&path, b"corrupt").unwrap();
        assert!(!notifications(&path, "work"));
        assert!(save_notifications(&path, "work", false).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"corrupt");
    }
}
