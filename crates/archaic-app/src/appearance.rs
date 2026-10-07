//! Appearance is a local preference, separate from the credential vault.
use cui::sys;
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}
impl Appearance {
    pub fn from_index(index: i32) -> Self {
        match index {
            1 => Self::Light,
            2 => Self::Dark,
            _ => Self::System,
        }
    }
    pub fn index(self) -> i32 {
        match self {
            Self::System => 0,
            Self::Light => 1,
            Self::Dark => 2,
        }
    }
    pub fn theme(self) -> sys::cui_theme {
        match self {
            Self::System => sys::CUI_THEME_SYSTEM,
            Self::Light => sys::CUI_THEME_LIGHT,
            Self::Dark => sys::CUI_THEME_DARK,
        }
    }
    fn text(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
    pub fn load(path: &Path) -> Self {
        let mut text = String::new();
        let Ok(file) = std::fs::File::open(path) else {
            return Self::System;
        };
        if file.take(32).read_to_string(&mut text).is_err() {
            return Self::System;
        }
        match text.trim() {
            "light" => Self::Light,
            "dark" => Self::Dark,
            _ => Self::System,
        }
    }
    pub fn save(self, path: &Path) -> std::io::Result<()> {
        let parent = path.parent().ok_or(std::io::ErrorKind::InvalidInput)?;
        std::fs::create_dir_all(parent)?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        writeln!(file, "{}", self.text())?;
        file.as_file().sync_all()?;
        file.persist(path).map_err(|error| error.error)?;
        Ok(())
    }
}

pub fn preference_path() -> Option<PathBuf> {
    fn absolute_env(key: &str) -> Option<PathBuf> {
        std::env::var_os(key)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    }
    #[cfg(target_os = "linux")]
    let base = absolute_env("XDG_CONFIG_HOME")
        .or_else(|| absolute_env("HOME").map(|path| path.join(".config")))?;
    #[cfg(target_os = "macos")]
    let base = absolute_env("HOME")?.join("Library/Preferences");
    #[cfg(target_os = "windows")]
    let base = absolute_env("APPDATA")?;
    Some(base.join("archaic").join("appearance"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preference_round_trip_and_replacement() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config/appearance");
        assert_eq!(Appearance::load(&path), Appearance::System);
        for appearance in [Appearance::Dark, Appearance::Light, Appearance::System] {
            appearance.save(&path).unwrap();
            assert_eq!(Appearance::load(&path), appearance);
        }
        std::fs::write(&path, "future-option").unwrap();
        assert_eq!(Appearance::load(&path), Appearance::System);
    }
    #[test]
    fn failed_save_is_reported_without_touching_existing_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("blocking-file");
        std::fs::write(&path, "untouched").unwrap();
        assert!(Appearance::Dark.save(&path.join("appearance")).is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "untouched");
    }
}
