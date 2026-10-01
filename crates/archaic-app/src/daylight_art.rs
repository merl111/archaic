//! Vector brand mark, matching the Daylight reference.
use cui::{Icon, Result, chat::Theme, sys};
fn command(op: i32, p: &[f32], color: u32, foreground: i32) -> sys::cui_icon_command {
    let mut c = sys::cui_icon_command {
        op,
        rgba: color,
        current_color: foreground,
        ..Default::default()
    };
    c.values[..p.len()].copy_from_slice(p);
    c
}
pub fn brand(t: &Theme) -> Result<Icon> {
    use sys::*;
    let c = |op, p: &[f32], color| command(op, p, color, 0);
    Icon::vector(
        22.,
        22.,
        &[
            c(CUI_ICON_MOVE, &[7., 0.], 0),
            c(CUI_ICON_LINE, &[15., 0.], 0),
            c(CUI_ICON_CUBIC, &[19., 0., 22., 3., 22., 7.], 0),
            c(CUI_ICON_LINE, &[22., 15.], 0),
            c(CUI_ICON_CUBIC, &[22., 19., 19., 22., 15., 22.], 0),
            c(CUI_ICON_LINE, &[7., 22.], 0),
            c(CUI_ICON_CUBIC, &[3., 22., 0., 19., 0., 15.], 0),
            c(CUI_ICON_LINE, &[0., 7.], 0),
            c(CUI_ICON_CUBIC, &[0., 3., 3., 0., 7., 0.], 0),
            c(CUI_ICON_CLOSE, &[], 0),
            c(CUI_ICON_FILL, &[], t.foreground),
            c(CUI_ICON_MOVE, &[15., 11.], 0),
            c(CUI_ICON_CUBIC, &[15., 13.21, 13.21, 15., 11., 15.], 0),
            c(CUI_ICON_CUBIC, &[8.79, 15., 7., 13.21, 7., 11.], 0),
            c(CUI_ICON_CUBIC, &[7., 8.79, 8.79, 7., 11., 7.], 0),
            c(CUI_ICON_CUBIC, &[13.21, 7., 15., 8.79, 15., 11.], 0),
            c(CUI_ICON_CLOSE, &[], 0),
            c(CUI_ICON_FILL, &[], t.surface),
        ],
    )
}
