//! Non-secret desktop preferences, scoped to a named account profile.
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::Path,
};
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Profiles {
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
