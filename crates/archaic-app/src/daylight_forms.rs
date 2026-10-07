//! Shared Daylight treatment for the application's native form controls.
use crate::{daylight::style, view::View};
use cui::{Result, Widget, chat::Theme, sys};
fn fields(widgets: &[&Widget], t: &Theme) -> Result<()> {
    for w in widgets {
        style(w, t, t.selected, 12., 10)?;
    }
    Ok(())
}
fn primary(widgets: &[&Widget], t: &Theme) -> Result<()> {
    for w in widgets {
        w.set_style(Some(&sys::cui_widget_style {
            background: t.foreground,
            foreground: t.surface,
            radius: 12.,
            padding: 10,
            ..Default::default()
        }))?;
    }
    Ok(())
}
impl View {
    pub fn theme_forms(&self, t: &Theme) -> Result<()> {
        self.theme_account(t)?;
        self.theme_security(t)?;
        self.theme_tools(t)?;
        let r = &self.room_controls;
        let n = &self.new_conversation;
        let j = &self.join;
        fields(
            &[
                &r.invite_user,
                &r.invite,
                &r.accept,
                &r.decline,
                &r.leave,
                &r.confirm,
                &r.cancel,
                &n.mode,
                &n.person,
                &n.cancel,
                &j.join_address,
                &j.join_cancel,
                &self.history_controls.older,
                &self.history_controls.newer,
                &self.history_controls.latest,
            ],
            t,
        )?;
        primary(&[&n.submit, &j.join_submit], t)?;
        let o = &self.organization;
        fields(
            &[
                &o.mode, &o.target, &o.value, &o.extra, &o.more, &o.select, &o.join, &o.confirm,
            ],
            t,
        )?;
        primary(&[&o.apply], t)
    }
    fn theme_account(&self, t: &Theme) -> Result<()> {
        self.settings.theme(t)?;
        self.extras.theme(t)?;
        fields(
            &[
                &self.server,
                &self.username,
                &self.password,
                &self.browser_login,
                &self.oauth_login,
                &self.cancel_login,
                &self.temporary,
                &self.profile,
                &self.profiles,
                &self.switch_profile,
                &self.appearance,
                &self.message_layout,
                &self.text_size,
                &self.shortcuts,
                &self.deep_link,
                &self.open_link,
                &self.sign_out,
                &self.reauth_password,
                &self.reauth,
                &self.reauth_sso,
                &self.reauth_oauth,
                &self.reauth_cancel,
            ],
            t,
        )
    }
    fn theme_security(&self, t: &Theme) -> Result<()> {
        let s = &self.security;
        fields(
            &[
                &s.mode,
                &s.refresh,
                &s.devices,
                &s.verify,
                &s.accept,
                &s.start,
                &s.mismatch,
                &s.cancel,
                &s.password,
                &s.recovery_key,
                &s.bootstrap,
                &s.enable,
                &s.recover,
                &s.dismiss_key,
                &s.passphrase,
                &s.export,
                &s.import,
                &s.room_keys,
            ],
            t,
        )?;
        primary(&[&s.confirm], t)
    }
    fn theme_tools(&self, t: &Theme) -> Result<()> {
        let s = &self.tools;
        fields(
            &[
                &s.mode,
                &s.choose_file,
                &s.discard,
                &s.query,
                &s.search_mode,
                &s.more_search,
                &s.hits,
                &s.open_hit,
                &s.details,
                &s.more_details,
                &s.download,
                &s.preview,
                &s.cancel_transfer,
                &s.original,
                &s.receipt_mode,
            ],
            t,
        )?;
        primary(&[&s.upload, &s.search, &s.mark_read], t)
    }
}
