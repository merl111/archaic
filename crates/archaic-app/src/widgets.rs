use cui::{Result, Widget};

pub fn label(parent: &Widget, text: &str, role: i32) -> Result<Widget> {
    let widget = parent.label(text)?;
    widget.set_role(role)?;
    Ok(widget)
}
