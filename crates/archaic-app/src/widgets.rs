use cui::{Result, Widget};

pub fn label(parent: &Widget, text: &str, role: i32) -> Result<Widget> {
    let widget = parent.label(text)?;
    widget.set_role(role)?;
    match role {
        cui::sys::CUI_ROLE_TITLE => {
            widget.set_font("sans", 24., 800)?;
        }
        cui::sys::CUI_ROLE_HEADING => {
            widget.set_font("sans", 16.5, 700)?;
        }
        _ => {}
    }
    Ok(widget)
}
