use crate::{
    model::{Model, Phase},
    view::set_text,
    widgets::label,
};
use archaic_i18n::Translator;
use archaic_matrix::{Membership, RoomAction};
use cui::{Result, Widget, sys};

pub struct RoomControls {
    root: Widget,
    invitation: Widget,
    inviter: Widget,
    pub accept: Widget,
    pub decline: Widget,
    joined: Widget,
    pub invite_user: Widget,
    pub invite: Widget,
    pub leave: Widget,
    confirmation: Widget,
    question: Widget,
    pub confirm: Widget,
    pub cancel: Widget,
    feedback: Widget,
}
impl RoomControls {
    pub fn new(parent: &Widget, tr: &Translator) -> Result<Self> {
        let root = parent.box_layout(sys::CUI_VERTICAL, 8)?;
        let invitation = root.box_layout(sys::CUI_HORIZONTAL, 8)?;
        let inviter = label(&invitation, "", sys::CUI_ROLE_CAPTION)?;
        inviter.expand(true)?;
        let accept = invitation.button(&tr.text("accept-invitation"))?;
        let decline = invitation.button(&tr.text("decline-invitation"))?;
        let joined = root.box_layout(sys::CUI_HORIZONTAL, 8)?;
        let invite_user = joined.entry("")?;
        invite_user.set_placeholder(&tr.text("invite-user-placeholder"))?;
        invite_user.accessibility(&tr.text("invite-user"), "")?;
        invite_user.expand(true)?;
        let invite = joined.button(&tr.text("invite-user"))?;
        let leave = joined.button(&tr.text("leave-room"))?;
        let confirmation = root.box_layout(sys::CUI_VERTICAL, 8)?;
        let question = label(&confirmation, "", sys::CUI_ROLE_CAPTION)?;
        let actions = confirmation.box_layout(sys::CUI_HORIZONTAL, 8)?;
        let cancel = actions.button(&tr.text("cancel"))?;
        let confirm = actions.button(&tr.text("confirm"))?;
        let feedback = label(&root, "", sys::CUI_ROLE_CAPTION)?;
        Ok(Self {
            root,
            invitation,
            inviter,
            accept,
            decline,
            joined,
            invite_user,
            invite,
            leave,
            confirmation,
            question,
            confirm,
            cancel,
            feedback,
        })
    }
    pub fn render(&self, model: &Model, tr: &Translator, fixture: bool) -> Result<()> {
        let room = model.selected_room();
        self.root.set_visible(room.is_some())?;
        let invited = room.is_some_and(|room| room.membership == Membership::Invited);
        let confirming = model.room_confirmation.is_some();
        let enabled = !model.tools_busy()
            && model.phase == Phase::Connected
            && !fixture
            && !model.creating
            && model.room_pending.is_none()
            && !model.joining
            && !model.sending;
        self.invitation.set_visible(invited && !confirming)?;
        self.joined.set_visible(!invited && !confirming)?;
        for widget in [
            &self.accept,
            &self.decline,
            &self.invite,
            &self.invite_user,
            &self.leave,
            &self.confirm,
            &self.cancel,
        ] {
            widget.set_enabled(enabled)?;
        }
        let inviter = room.and_then(|room| room.inviter.as_deref());
        set_text(
            &self.inviter,
            &inviter
                .map(|user| format!("{} {user}", tr.text("invited-by")))
                .unwrap_or_else(|| tr.text("invitation")),
        )?;
        self.confirmation.set_visible(confirming)?;
        let leaving = model
            .room_confirmation
            .as_ref()
            .is_some_and(|(_, action)| *action == RoomAction::Leave);
        set_text(
            &self.question,
            &tr.text(if leaving {
                "leave-confirmation"
            } else {
                "decline-confirmation"
            }),
        )?;
        let text = if model.room_pending.is_some() {
            tr.text("room-action-pending")
        } else {
            model
                .room_feedback
                .as_ref()
                .filter(|(id, _)| Some(id) == model.selected.as_ref())
                .map(|(_, key)| tr.text(key))
                .unwrap_or_default()
        };
        self.feedback.set_visible(!text.is_empty())?;
        set_text(&self.feedback, &text)
    }
}
