//! Application mark shared with packaged desktop icons.
use cui::{Icon, Result, chat::Theme};
pub fn brand(_: &Theme) -> Result<Icon> {
    crate::branding::mark()
}
