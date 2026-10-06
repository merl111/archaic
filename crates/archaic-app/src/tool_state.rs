use crate::model::{Model, Phase};
use archaic_matrix::{Membership, MessageDetails, SearchPage, ToolAction, ToolData, ToolRequest};
#[derive(Default)]
pub struct ToolState {
    pub sticker: bool,
    pub show: bool,
    pub preview: Option<Preview>,
    pub mode: i32,
    pub pending: Option<ToolRequest>,
    pub dialog: bool,
    pub error: Option<&'static str>,
    pub status: Option<&'static str>,
    pub upload: Option<ToolRequest>,
    pub search: SearchPage,
    pub query: String,
    pub search_mode: i32,
    pub search_key: Option<(String, archaic_matrix::SearchMode)>,
    pub details: Option<MessageDetails>,
}
pub struct Preview {
    pub id: archaic_matrix::TransactionId,
    pub event: String,
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
}
impl Model {
    pub fn tools_busy(&self) -> bool {
        self.tools.pending.is_some() || self.tools.dialog
    }
    pub fn can_tool(&self) -> bool {
        self.phase == Phase::Connected
            && !self.tools_busy()
            && !self.sending
            && !self.creating
            && !self.joining
            && self.message_draft.is_none()
            && self.room_pending.is_none()
            && self.room_confirmation.is_none()
            && self
                .selected_room()
                .is_some_and(|r| r.membership == Membership::Joined)
    }
    pub fn tool_result(&mut self, request: ToolRequest, result: Result<ToolData, &'static str>) {
        if self.tools.pending.as_ref() != Some(&request)
            || self.selected.as_ref() != Some(&request.room_id)
        {
            return;
        }
        self.tools.pending = None;
        match result {
            Err(key) => self.tools.error = Some(key),
            Ok(data) => {
                self.tools.error = None;
                self.tools.status = Some(match data {
                    ToolData::Preview {
                        pixels,
                        width,
                        height,
                    } => {
                        if let ToolAction::Preview { event_id } = &request.action {
                            self.tools.preview = Some(Preview {
                                id: request.id.clone(),
                                event: event_id.clone(),
                                pixels,
                                width,
                                height,
                            });
                        }
                        "preview-ready"
                    }
                    ToolData::QueuedUpload => {
                        self.tools.upload = None;
                        "upload-queued"
                    }
                    ToolData::Uploaded => {
                        self.tools.upload = None;
                        "upload-done"
                    }
                    ToolData::Downloaded => "download-done",
                    ToolData::Read => "receipt-done",
                    ToolData::Search(page) => {
                        if let ToolAction::Search { query, mode, .. } = &request.action {
                            self.tools.search_key = Some((query.clone(), *mode));
                        }
                        self.tools.search = page;
                        "search-done"
                    }
                    ToolData::Details(details) => {
                        self.message_selected = Some(details.message.id.clone());
                        self.tools.details = Some(*details);
                        "details-done"
                    }
                });
            }
        }
    }
}
