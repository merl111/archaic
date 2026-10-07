use crate::{Controller, model::Phase};
use archaic_matrix::{Command, SearchMode, ToolAction, ToolRequest, transaction_id};
use cui::{Result, sys};
struct DialogTarget {
    epoch: u64,
    room_id: String,
    event_id: Option<String>,
}
impl Controller {
    pub fn toggle_tools(&self) -> Result<()> {
        self.remember_draft()?;
        let mut model = self.model.borrow_mut();
        if model.can_tool() {
            model.tools.show = !model.tools.show;
        }
        drop(model);
        self.render()
    }
    pub fn tool_mode(&self) -> Result<()> {
        self.remember_draft()?;
        let mut model = self.model.borrow_mut();
        if model.can_tool() {
            model.tools.mode = self.view.tools.mode.get_selected()?.clamp(0, 3);
        }
        drop(model);
        self.render()
    }
    pub fn search_input(&self) -> Result<()> {
        let mut model = self.model.borrow_mut();
        if !model.tools_busy() {
            model.tools.query = self.view.tools.query.text()?;
            model.tools.search_mode = self.view.tools.search_mode.get_selected()?.clamp(0, 2);
        }
        drop(model);
        self.render()
    }
    pub fn tool_selection(&self) -> Result<()> {
        self.render()
    }
    fn send_tool(&self, action: ToolAction) -> Result<()> {
        self.remember_draft()?;
        if self.fixture || !self.model.borrow().can_tool() {
            return Ok(());
        }
        let Some(room_id) = self.model.borrow().selected.clone() else {
            return Ok(());
        };
        self.submit_tool(ToolRequest {
            room_id,
            id: transaction_id(),
            action,
        })
    }
    pub(crate) fn submit_tool(&self, request: ToolRequest) -> Result<()> {
        if self.enqueue(Command::RoomTool(request.clone())) {
            let mut model = self.model.borrow_mut();
            model.tools.pending = Some(request);
            model.tools.error = None;
            model.tools.status = None;
        }
        self.render()
    }
    pub fn run_search(&self) -> Result<()> {
        self.search_page(false)
    }
    pub fn more_search(&self) -> Result<()> {
        self.search_page(true)
    }
    fn search_page(&self, more: bool) -> Result<()> {
        self.search_input()?;
        let (query, mode) = {
            let m = self.model.borrow();
            (
                m.tools.query.trim().to_owned(),
                if m.tools.search_mode == 2 {
                    SearchMode::Indexed
                } else if m.tools.search_mode == 1 {
                    SearchMode::Server
                } else {
                    SearchMode::Local
                },
            )
        };
        if more && self.model.borrow().tools.search_key.as_ref() != Some(&(query.clone(), mode)) {
            self.model.borrow_mut().tools.error = Some("search-restart");
            return self.render();
        }
        self.send_tool(ToolAction::Search { query, mode, more })
    }
    pub fn load_details(&self) -> Result<()> {
        self.details_page(false)
    }
    pub fn more_details(&self) -> Result<()> {
        self.details_page(true)
    }
    fn details_page(&self, more: bool) -> Result<()> {
        let id = self.model.borrow().message_selected.clone();
        if let Some(event_id) = id {
            self.send_tool(ToolAction::Details { event_id, more })?;
        }
        Ok(())
    }
    pub fn open_hit(&self) -> Result<()> {
        let index = self.view.tools.hits.get_selected()?;
        let hit = usize::try_from(index)
            .ok()
            .and_then(|i| self.model.borrow().tools.search.messages.get(i).cloned());
        if let Some(hit) = hit {
            if !self.model.borrow().can_tool() {
                return Ok(());
            }
            let id = hit.id.clone();
            {
                let mut model = self.model.borrow_mut();
                model.message_selected = Some(id.clone());
                model.tools.mode = 2;
                model.tools.details = None;
            }
            self.send_tool(ToolAction::Details {
                event_id: id,
                more: false,
            })?;
        }
        Ok(())
    }
    pub fn open_original(&self) -> Result<()> {
        let id = self
            .model
            .borrow()
            .selected_message()
            .and_then(|m| m.reply_to.clone());
        if let Some(event_id) = id {
            self.send_tool(ToolAction::Details {
                event_id,
                more: false,
            })?;
        }
        Ok(())
    }
    pub fn mark_read(&self) -> Result<()> {
        let id = self.model.borrow().message_selected.clone();
        if let Some(event_id) = id {
            let private = self.view.tools.receipt_mode.get_selected()? != 1;
            self.send_tool(if self.view.tools.receipt_thread.get_checked()? {
                ToolAction::MarkThreadRead { event_id, private }
            } else {
                ToolAction::MarkRead { event_id, private }
            })?;
        }
        Ok(())
    }
    pub fn upload_file(&self) -> Result<()> {
        if self.fixture || !self.model.borrow().can_tool() {
            return Ok(());
        }
        let request = self.model.borrow().tools.upload.clone();
        if let Some(request) = request {
            self.submit_tool(request)?;
        }
        Ok(())
    }
    pub fn discard_upload(&self) -> Result<()> {
        if self.model.borrow().can_tool() && self.enqueue(Command::DiscardUpload) {
            self.model.borrow_mut().tools.upload = None;
        }
        self.render()
    }
    pub fn choose_file(&self) -> Result<()> {
        self.file_dialog(false)
    }
    pub fn open_image(&self) -> Result<()> {
        self.remember_draft()?;
        {
            let mut m = self.model.borrow_mut();
            let Some(event) = m.message_selected.clone() else {
                return Ok(());
            };
            let Some(room) = m.selected.clone() else {
                return Ok(());
            };
            if !m.image_message(&event).is_some_and(|v| {
                !v.deleted && v.attachment.as_ref().is_some_and(|a| a.kind == "m.image")
            }) {
                return Ok(());
            }
            m.image_open = Some((room, event));
            m.tools.show = false;
            m.tools.error = None;
        }
        self.view.ui.panel.set(crate::daylight::Panel::None);
        self.render()?;
        if self
            .model
            .borrow()
            .message_selected
            .as_ref()
            .is_some_and(|id| id.starts_with('$'))
        {
            self.preview_file()?;
        }
        Ok(())
    }
    pub fn preview_file(&self) -> Result<()> {
        let id = self.model.borrow().message_selected.clone();
        if let Some(event_id) = id {
            self.send_tool(ToolAction::Preview { event_id })?;
        }
        Ok(())
    }
    pub fn cancel_transfer(&self) -> Result<()> {
        let id = self
            .model
            .borrow()
            .tools
            .pending
            .as_ref()
            .map(|r| r.id.clone());
        if let Some(id) = id {
            self.enqueue(Command::CancelTransfer { id });
        }
        Ok(())
    }
    pub fn download_file(&self) -> Result<()> {
        self.file_dialog(true)
    }
    fn file_dialog(&self, save: bool) -> Result<()> {
        if self.fixture || !self.model.borrow().can_tool() {
            return Ok(());
        }
        let (epoch, room_id, event_id) = {
            let m = self.model.borrow();
            (
                m.epoch,
                m.selected.clone().unwrap(),
                m.message_selected.clone(),
            )
        };
        if save
            && self
                .model
                .borrow()
                .selected_message()
                .is_none_or(|m| m.attachment.is_none())
        {
            return Ok(());
        }
        self.model.borrow_mut().tools.dialog = true;
        let weak = self.self_weak.borrow().clone();
        let result = self.window.file_dialog(
            if save {
                sys::CUI_DIALOG_SAVE
            } else {
                sys::CUI_DIALOG_OPEN
            },
            &self
                .tr
                .text(if save { "download-file" } else { "choose-file" }),
            "",
            move |_, result, path| {
                if let Some(c) = weak.upgrade() {
                    let target = DialogTarget {
                        epoch,
                        room_id: room_id.clone(),
                        event_id: if save { event_id.clone() } else { None },
                    };
                    c.file_dialog_result(&target, result, path)
                        .expect("live controls");
                }
            },
        );
        if result.is_err() {
            self.model.borrow_mut().tools.dialog = false;
            self.model.borrow_mut().tools.error = Some("file-dialog-failed");
        }
        self.render()
    }
    fn file_dialog_result(&self, target: &DialogTarget, result: i32, path: String) -> Result<()> {
        let mut model = self.model.borrow_mut();
        if model.epoch != target.epoch || model.selected.as_ref() != Some(&target.room_id) {
            return Ok(());
        }
        model.tools.dialog = false;
        if model.phase != Phase::Connected || result != sys::CUI_DIALOG_ACCEPTED {
            if result == sys::CUI_DIALOG_FAILED {
                model.tools.error = Some("file-dialog-failed");
            }
            drop(model);
            return self.render();
        }
        let action = match &target.event_id {
            Some(id) => ToolAction::Download {
                event_id: id.clone(),
                path: path.into(),
            },
            None if model.tools.sticker => ToolAction::Sticker { path: path.into() },
            None => ToolAction::QueueUpload { path: path.into() },
        };
        let request = ToolRequest {
            room_id: target.room_id.clone(),
            id: transaction_id(),
            action,
        };
        if target.event_id.is_some() {
            drop(model);
            self.submit_tool(request)
        } else {
            model.tools.upload = Some(request);
            model.tools.error = None;
            model.tools.status = None;
            drop(model);
            self.render()
        }
    }
}
