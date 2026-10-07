use crate::{
    model::Model,
    view::{message_text, set_text},
    widgets::label,
};
use archaic_i18n::Translator;
use cui::{Result, Widget, sys};
use std::cell::RefCell;
pub struct ToolControls {
    root: Widget,
    pub mode: Widget,
    upload_group: Widget,
    upload_path: Widget,
    pub choose_file: Widget,
    pub upload: Widget,
    pub discard: Widget,
    search_group: Widget,
    pub query: Widget,
    pub search_mode: Widget,
    pub search: Widget,
    pub more_search: Widget,
    pub hits: Widget,
    pub open_hit: Widget,
    search_status: Widget,
    detail_group: Widget,
    pub details: Widget,
    pub more_details: Widget,
    pub download: Widget,
    pub preview: Widget,
    preview_image: Widget,
    preview_version: RefCell<Option<archaic_matrix::TransactionId>>,
    pub cancel_transfer: Widget,
    progress: Widget,
    pub original: Widget,
    detail_text: Widget,
    detail_status: Widget,
    receipt_group: Widget,
    pub receipt_mode: Widget,
    pub receipt_thread: Widget,
    pub mark_read: Widget,
    target: Widget,
    status: Widget,
    error: Widget,
    hit_labels: RefCell<Vec<String>>,
}
impl ToolControls {
    pub fn new(parent: &Widget, tr: &Translator) -> Result<Self> {
        let root = parent.box_layout(sys::CUI_VERTICAL, 8)?;
        root.expand(true)?;
        let mode = root.select(&[
            &tr.text("tool-files"),
            &tr.text("tool-search"),
            &tr.text("tool-details"),
            &tr.text("tool-receipts"),
        ])?;
        mode.accessibility(&tr.text("room-tools"), "")?;
        let target = label(&root, "", sys::CUI_ROLE_CAPTION)?;
        let upload_group = root.box_layout(sys::CUI_VERTICAL, 8)?;
        label(
            &upload_group,
            &tr.text("upload-hint"),
            sys::CUI_ROLE_CAPTION,
        )?;
        let upload_path = label(&upload_group, "", sys::CUI_ROLE_CAPTION)?;
        let row = upload_group.box_layout(sys::CUI_HORIZONTAL, 8)?;
        let choose_file = row.button(&tr.text("choose-file"))?;
        let upload = row.button(&tr.text("upload-file"))?;
        let discard = row.button(&tr.text("cancel"))?;
        let search_group = root.box_layout(sys::CUI_VERTICAL, 8)?;
        search_group.expand(true)?;
        let search_mode = search_group.select(&[
            &tr.text("search-local"),
            &tr.text("search-server"),
            &tr.text("search-indexed"),
        ])?;
        search_mode.accessibility(&tr.text("search-scope"), "")?;
        label(
            &search_group,
            &tr.text("search-filter-hint"),
            sys::CUI_ROLE_CAPTION,
        )?;
        let query = search_group.entry("")?;
        query.set_placeholder(&tr.text("search-query"))?;
        query.accessibility(&tr.text("search-query"), "")?;
        let row = search_group.box_layout(sys::CUI_HORIZONTAL, 8)?;
        let search = row.button(&tr.text("tool-search"))?;
        let more_search = row.button(&tr.text("search-more"))?;
        let open_hit = row.button(&tr.text("search-open"))?;
        let search_status = label(&search_group, "", sys::CUI_ROLE_CAPTION)?;
        let hits = search_group.list(&[])?;
        hits.expand(true)?;
        hits.accessibility(&tr.text("search-results"), "")?;
        let detail_group = root.box_layout(sys::CUI_VERTICAL, 8)?;
        detail_group.expand(true)?;
        let row = detail_group.box_layout(sys::CUI_HORIZONTAL, 8)?;
        let details = row.button(&tr.text("details-load"))?;
        let more_details = row.button(&tr.text("details-more"))?;
        let original = row.button(&tr.text("reply-original"))?;
        let download = row.button(&tr.text("download-file"))?;
        let preview = row.button(&tr.text("preview-file"))?;
        let preview_image = detail_group.image()?;
        preview_image.set_min_size(200, 180)?;
        let detail_status = label(&detail_group, "", sys::CUI_ROLE_CAPTION)?;
        let detail_text = detail_group.textarea("")?;
        detail_text.set_read_only(true)?;
        detail_text.expand(true)?;
        let receipt_group = root.box_layout(sys::CUI_VERTICAL, 8)?;
        label(
            &receipt_group,
            &tr.text("receipt-hint"),
            sys::CUI_ROLE_CAPTION,
        )?;
        let receipt_mode =
            receipt_group.select(&[&tr.text("receipt-private"), &tr.text("receipt-public")])?;
        receipt_mode.set_selected(0)?;
        receipt_mode.accessibility(&tr.text("receipt-visibility"), "")?;
        let receipt_thread = receipt_group.checkbox(&tr.text("receipt-thread"), false)?;
        let mark_read = receipt_group.button(&tr.text("mark-read"))?;
        let cancel_transfer = root.button(&tr.text("cancel-transfer"))?;
        let progress = label(&root, "", sys::CUI_ROLE_CAPTION)?;
        let status = label(&root, "", sys::CUI_ROLE_CAPTION)?;
        let error = label(&root, "", sys::CUI_ROLE_DANGER)?;
        Ok(Self {
            root,
            mode,
            upload_group,
            upload_path,
            choose_file,
            upload,
            discard,
            search_group,
            query,
            search_mode,
            search,
            more_search,
            hits,
            open_hit,
            search_status,
            detail_group,
            details,
            more_details,
            download,
            preview,
            preview_image,
            preview_version: Default::default(),
            cancel_transfer,
            progress,
            original,
            detail_text,
            detail_status,
            receipt_group,
            receipt_mode,
            receipt_thread,
            mark_read,
            target,
            status,
            error,
            hit_labels: RefCell::new(vec![]),
        })
    }
    pub fn render(&self, model: &Model, tr: &Translator, fixture: bool) -> Result<()> {
        self.root.set_visible(model.tools.show)?;
        if !model.tools.show {
            return Ok(());
        }
        let enabled = model.can_tool() && !fixture;
        self.cancel_transfer
            .set_visible(model.tools.pending.as_ref().is_some_and(|r| {
                matches!(
                    r.action,
                    archaic_matrix::ToolAction::QueueUpload { .. }
                        | archaic_matrix::ToolAction::Download { .. }
                        | archaic_matrix::ToolAction::Preview { .. }
                )
            }))?;
        let progress = model
            .tools
            .pending
            .as_ref()
            .and_then(|r| model.transfer_progress.get(r.id.as_str()));
        set_text(
            &self.progress,
            &progress
                .map(|(c, t)| format!("{c} / {t}"))
                .unwrap_or_default(),
        )?;
        self.progress.set_visible(progress.is_some())?;
        self.preview.set_enabled(
            enabled
                && model
                    .selected_message()
                    .is_some_and(|m| m.attachment.as_ref().is_some_and(|a| a.kind == "m.image")),
        )?;
        let preview = model
            .tools
            .preview
            .as_ref()
            .filter(|p| model.message_selected.as_ref() == Some(&p.event));
        self.preview_image.set_visible(preview.is_some())?;
        if let Some(p) = preview {
            if self.preview_version.borrow().as_ref() != Some(&p.id) {
                self.preview_image
                    .rgba(&p.pixels, p.width as i32, p.height as i32)?;
                *self.preview_version.borrow_mut() = Some(p.id.clone());
            }
        } else {
            *self.preview_version.borrow_mut() = None;
        }
        if self.mode.get_selected()? != model.tools.mode {
            self.mode.set_selected(model.tools.mode)?;
        }
        self.mode.set_enabled(enabled)?;
        self.upload_group.set_visible(model.tools.mode == 0)?;
        self.search_group.set_visible(model.tools.mode == 1)?;
        self.detail_group.set_visible(model.tools.mode == 2)?;
        self.receipt_group.set_visible(model.tools.mode == 3)?;
        let target = model.selected_message();
        set_text(
            &self.target,
            &target
                .map(|m| {
                    format!(
                        "{}: {}",
                        tr.text("tool-target"),
                        m.body
                            .as_deref()
                            .unwrap_or("")
                            .chars()
                            .take(64)
                            .collect::<String>()
                    )
                })
                .unwrap_or_else(|| tr.text("message-select")),
        )?;
        self.choose_file.set_enabled(enabled)?;
        self.upload
            .set_enabled(enabled && model.tools.upload.is_some())?;
        self.discard
            .set_enabled(enabled && model.tools.upload.is_some())?;
        let name = model
            .tools
            .upload
            .as_ref()
            .and_then(|r| match &r.action {
                archaic_matrix::ToolAction::Upload { path }
                | archaic_matrix::ToolAction::QueueUpload { path } => {
                    path.file_name().map(|s| s.to_string_lossy().into_owned())
                }
                _ => None,
            })
            .unwrap_or_else(|| tr.text("no-file"));
        set_text(&self.upload_path, &name)?;
        self.render_search(model, tr, enabled)?;
        self.details
            .set_enabled(enabled && model.message_selected.is_some())?;
        self.more_details.set_enabled(
            enabled
                && model.tools.details.as_ref().is_some_and(|d| {
                    d.more && Some(&d.message.id) == model.message_selected.as_ref()
                }),
        )?;
        self.original
            .set_enabled(enabled && target.is_some_and(|m| m.reply_to.is_some()))?;
        self.download
            .set_enabled(enabled && target.is_some_and(|m| m.attachment.is_some() && !m.deleted))?;
        self.mark_read.set_enabled(enabled && target.is_some())?;
        self.receipt_mode.set_enabled(enabled)?;
        self.render_details(model, tr)?;
        set_text(
            &self.status,
            &tr.text(if model.tools_busy() {
                "tool-working"
            } else {
                model.tools.status.unwrap_or("tool-ready")
            }),
        )?;
        set_text(
            &self.error,
            &model.tools.error.map(|s| tr.text(s)).unwrap_or_default(),
        )?;
        self.error.set_visible(model.tools.error.is_some())?;
        Ok(())
    }
    fn render_search(&self, model: &Model, tr: &Translator, enabled: bool) -> Result<()> {
        self.query.set_enabled(enabled)?;
        self.search_mode.set_enabled(enabled)?;
        self.search.set_enabled(enabled)?;
        self.more_search
            .set_enabled(enabled && model.tools.search.more)?;
        self.hits.set_enabled(enabled)?;
        set_text(&self.query, &model.tools.query)?;
        if self.search_mode.get_selected()? != model.tools.search_mode {
            self.search_mode.set_selected(model.tools.search_mode)?;
        }
        let labels: Vec<_> = model
            .tools
            .search
            .messages
            .iter()
            .map(|m| {
                format!(
                    "{} · {}",
                    m.sender,
                    crate::search_snippet::snippet(
                        m.body.as_deref().unwrap_or(""),
                        model
                            .tools
                            .search_key
                            .as_ref()
                            .map(|(q, _)| q.as_str())
                            .unwrap_or(""),
                        model
                            .tools
                            .search_key
                            .as_ref()
                            .is_some_and(|(_, m)| *m == archaic_matrix::SearchMode::Indexed)
                    )
                )
                .replace(['\0', '\n', '\r'], " ")
            })
            .collect();
        if *self.hit_labels.borrow() != labels {
            self.hits
                .set_items(&labels.iter().map(String::as_str).collect::<Vec<_>>())?;
            *self.hit_labels.borrow_mut() = labels;
        }
        self.open_hit
            .set_enabled(enabled && self.hits.get_selected()? >= 0)?;
        set_text(
            &self.search_status,
            &format!(
                "{}: {} · {}: {} · {}: {} · {}",
                tr.text("search-results"),
                model.tools.search.messages.len(),
                tr.text("search-scanned"),
                model.tools.search.scanned,
                tr.text("events-unavailable"),
                model.tools.search.unavailable,
                tr.text(if model.tools.search.truncated {
                    "search-truncated"
                } else if model.tools.search.more {
                    "page-more"
                } else if model.tools.search_key.is_some() {
                    "page-complete"
                } else {
                    "search-hint"
                })
            ),
        )
    }
    fn render_details(&self, model: &Model, tr: &Translator) -> Result<()> {
        let Some(details) = model
            .tools
            .details
            .as_ref()
            .filter(|d| Some(&d.message.id) == model.message_selected.as_ref())
        else {
            set_text(&self.detail_text, "")?;
            return set_text(&self.detail_status, &tr.text("details-hint"));
        };
        let mut text = message_text(&details.message, model, tr);
        if let Some(reply) = &details.reply {
            text.push_str(&format!(
                "\n\n{}:\n{}",
                tr.text("reply-original"),
                message_text(reply, model, tr)
            ));
        }
        if !details.thread.is_empty() {
            text.push_str(&format!(
                "\n\n{}\n{}",
                tr.text("thread-replies"),
                details
                    .thread
                    .iter()
                    .map(|m| message_text(m, model, tr))
                    .collect::<Vec<_>>()
                    .join("\n\n")
            ));
        }
        if details.reply_unavailable {
            text.push_str(&format!("\n{}", tr.text("reply-unavailable")));
        }
        text.push_str(&format!(
            "\n\n{}: {}",
            tr.text("read-by"),
            details.read_by.join(", ")
        ));
        text.push_str(&format!(
            "\n{}: {}",
            tr.text("thread-read-by"),
            details.thread_read_by.join(", ")
        ));
        if !details.revisions.is_empty() {
            text.push_str(&format!(
                "\n\n{}\n{}",
                tr.text("edit-history"),
                details.revisions.join("\n\n")
            ));
        }
        set_text(&self.detail_text, &text)?;
        set_text(
            &self.detail_status,
            &format!(
                "{}: {} · {}: {} · {}",
                tr.text("relations-loaded"),
                details.loaded,
                tr.text("events-unavailable"),
                details.unavailable,
                tr.text(if details.more {
                    "page-more"
                } else {
                    "relations-complete"
                })
            ),
        )
    }
}
