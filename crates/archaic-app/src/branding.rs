//! Embedded identity assets; no working-directory or network dependency.
use cui::{Icon, Result};

pub const TRAY_RGBA: &[u8] = include_bytes!("../../../assets/branding/icon-32.rgba");
const MARK_RGBA: &[u8] = include_bytes!("../../../assets/branding/icon-256.rgba");

pub fn mark() -> Result<Icon> {
    Icon::rgba(MARK_RGBA, 256, 256)
}

/// Set process identity before GTK initializes its display or creates windows.
pub fn prepare() {
    #[cfg(target_os = "linux")]
    unsafe {
        native::g_set_prgname(c"org.archaic.desktop".as_ptr());
        native::g_set_application_name(c"Archaic".as_ptr());
    }
}

/// Keep Linux's private icon search directory alive until after app destruction.
pub struct NativeBranding {
    #[cfg(target_os = "linux")]
    _icons: tempfile::TempDir,
}

impl NativeBranding {
    /// Called on the UI thread after CUI initialization, before creating windows.
    pub fn install() -> std::result::Result<Self, Box<dyn std::error::Error>> {
        #[cfg(target_os = "linux")]
        {
            use std::{ffi::CString, os::unix::ffi::OsStrExt};
            let icons = tempfile::Builder::new()
                .prefix("archaic-icons-")
                .tempdir()?;
            // A sized hicolor entry lets GTK enumerate native window icon sizes.
            let theme_dir = icons.path().join("hicolor");
            let images = theme_dir.join("256x256/apps");
            std::fs::create_dir_all(&images)?;
            std::fs::write(
                theme_dir.join("index.theme"),
                "[Icon Theme]\nName=Archaic\nDirectories=256x256/apps\n[256x256/apps]\nSize=256\nType=Fixed\nContext=Applications\n",
            )?;
            std::fs::write(
                images.join("org.archaic.desktop.png"),
                include_bytes!("../../../assets/branding/icon-256.png"),
            )?;
            let path = CString::new(icons.path().as_os_str().as_bytes())?;
            unsafe {
                let display = native::gdk_display_get_default();
                if display.is_null() {
                    return Err("GTK has no default display".into());
                }
                let theme = native::gtk_icon_theme_get_for_display(display);
                native::gtk_icon_theme_add_search_path(theme, path.as_ptr());
                native::gtk_window_set_default_icon_name(c"org.archaic.desktop".as_ptr());
            }
            Ok(Self { _icons: icons })
        }
        #[cfg(target_os = "macos")]
        {
            use objc2::{AllocAnyThread, MainThreadMarker};
            use objc2_app_kit::{NSApplication, NSImage};
            use objc2_foundation::NSData;
            let main = MainThreadMarker::new().ok_or("Branding must run on the UI thread")?;
            let data = NSData::with_bytes(include_bytes!("../../../assets/branding/archaic.icns"));
            let image = NSImage::initWithData(NSImage::alloc(), &data).ok_or("Invalid app icon")?;
            unsafe {
                NSApplication::sharedApplication(main).setApplicationIconImage(Some(&image));
            }
            Ok(Self {})
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            // Windows loads the executable's embedded resource through CUI.
            Ok(Self {})
        }
    }
}

#[cfg(target_os = "linux")]
mod native {
    use std::ffi::{c_char, c_void};
    #[link(name = "glib-2.0")]
    unsafe extern "C" {
        pub fn g_set_prgname(name: *const c_char);
        pub fn g_set_application_name(name: *const c_char);
    }
    #[link(name = "gtk-4")]
    unsafe extern "C" {
        pub fn gdk_display_get_default() -> *mut c_void;
        pub fn gtk_icon_theme_get_for_display(display: *mut c_void) -> *mut c_void;
        pub fn gtk_icon_theme_add_search_path(theme: *mut c_void, path: *const c_char);
        pub fn gtk_window_set_default_icon_name(name: *const c_char);
    }
}
