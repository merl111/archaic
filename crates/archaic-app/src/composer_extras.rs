//! Overflow actions are native menus; forms retain an explicit room/account target.
use crate::{
    Controller,
    daylight::{Panel, Shell, style},
    voice,
};
use archaic_i18n::Translator;
use archaic_matrix::{ToolAction, ToolRequest, transaction_id};
use cui::{App, Command, Menu, Result, Widget, chat::Theme, sys};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
pub struct Extras {
    pub root: Widget,
    poll: Widget,
    voice: Widget,
    question: Widget,
    answers: Widget,
    record: Widget,
    send: Widget,
    status: Widget,
    mode: Cell<u8>,
    target: RefCell<Option<(u64, String)>>,
    menu: RefCell<Option<(Menu, Vec<Command>)>>,
    recorder: RefCell<Option<voice::Recorder>>,
    clip: RefCell<Option<voice::Clip>>,
    request: RefCell<Option<ToolRequest>>,
}
impl Extras {
    pub fn new(ui: &Shell, tr: &Translator) -> Result<Self> {
        let root = ui.card(&ui.sheet_body, 0., 0)?;
        let poll = ui.card(&root, 0., 0)?;
        poll.label(&tr.text("poll-question"))?;
        let question = poll.entry("")?;
        question.set_placeholder(&tr.text("poll-question"))?;
        poll.label(&tr.text("poll-answers"))?;
        let answers = poll.textarea("")?;
        answers.textarea_height(180)?;
        let voice = ui.card(&root, 0., 0)?;
        voice.label(&tr.text("voice-hint"))?;
        let record = voice.button(&tr.text("voice-record"))?;
        let status = root.label("")?;
        let send = root.button(&tr.text("send"))?;
        Ok(Self {
            root,
            poll,
            voice,
            question,
            answers,
            record,
            send,
            status,
            mode: Cell::new(0),
            target: Default::default(),
            menu: Default::default(),
            recorder: Default::default(),
            clip: Default::default(),
            request: Default::default(),
        })
    }
    pub fn title(&self) -> &'static str {
        if self.mode.get() == 2 {
            "composer-poll"
        } else {
            "composer-voice"
        }
    }
    pub fn reset(&self) {
        self.recorder.borrow_mut().take();
        self.clip.borrow_mut().take();
        self.target.borrow_mut().take();
        self.request.borrow_mut().take();
    }
    pub fn theme(&self, t: &Theme) -> Result<()> {
        for w in [&self.question, &self.answers, &self.record] {
            style(w, t, t.selected, 12., 10)?;
        }
        self.send.set_style(Some(&sys::cui_widget_style {
            background: t.accent,
            foreground: t.on_accent,
            radius: 12.,
            padding: 12,
            ..Default::default()
        }))?;
        Ok(())
    }
}
impl Controller {
    pub fn install_composer_menu(self: &Rc<Self>, app: &App) -> Result<()> {
        let menu = app.menu()?;
        let mut commands = Vec::new();
        for (mode, key) in [
            (0, "composer-sticker"),
            (1, "composer-voice"),
            (2, "composer-poll"),
        ] {
            let weak = Rc::downgrade(self);
            let command = app.command(&self.tr.text(key), 0, 0, move || {
                if let Some(c) = weak.upgrade() {
                    c.open_composer_extra(mode).expect("live composer");
                }
            })?;
            menu.add(&command)?;
            commands.push(command);
        }
        *self.view.extras.menu.borrow_mut() = Some((menu, commands));
        for (button, action) in [
            (
                &self.view.extras.record,
                Self::record_voice as fn(&Self) -> Result<()>,
            ),
            (&self.view.extras.send, Self::send_extra),
        ] {
            let weak = Rc::downgrade(self);
            button.on_action(move |_| {
                if let Some(c) = weak.upgrade() {
                    action(&c).expect("live composer");
                }
            })?;
        }
        Ok(())
    }
    pub fn composer_more(&self) -> Result<()> {
        if !self.fixture && !self.model.borrow().can_tool() {
            return Ok(());
        }
        let menu = self
            .view
            .extras
            .menu
            .borrow()
            .as_ref()
            .map(|(m, _)| m.clone());
        if let Some(menu) = menu {
            menu.popup_at(&self.view.ui.composer.part(8)?, 0., 0., 40., 44.)?;
        }
        Ok(())
    }
    fn open_composer_extra(&self, mode: u8) -> Result<()> {
        if !self.fixture && !self.model.borrow().can_tool() {
            return Ok(());
        }
        if mode == 0 {
            self.model.borrow_mut().tools.sticker = true;
            return self.choose_file();
        }
        let m = self.model.borrow();
        let Some(room) = m.selected.clone() else {
            return Ok(());
        };
        let ui = &self.view.extras;
        ui.reset();
        *ui.target.borrow_mut() = Some((m.epoch, room));
        drop(m);
        ui.mode.set(mode);
        ui.poll.set_visible(mode == 2)?;
        ui.voice.set_visible(mode == 1)?;
        ui.question.set_text("")?;
        ui.answers.set_text("")?;
        ui.status.set_text("")?;
        ui.record.set_text(&self.tr.text("voice-record"))?;
        ui.send.set_enabled(mode == 2)?;
        self.model.borrow_mut().tools.error = None;
        self.model.borrow_mut().tools.status = None;
        self.view.ui.panel.set(Panel::Composer);
        self.render()
    }
    fn record_voice(&self) -> Result<()> {
        let ui = &self.view.extras;
        if self.model.borrow().tools_busy() {
            return Ok(());
        }
        let old = ui.recorder.borrow_mut().take();
        if let Some(recorder) = old {
            match recorder.stop() {
                Ok(clip) => {
                    ui.status.set_text(&format!(
                        "{} · {:.1}s",
                        self.tr.text("voice-ready"),
                        clip.duration_ms as f64 / 1000.
                    ))?;
                    *ui.clip.borrow_mut() = Some(clip);
                    ui.send.set_enabled(true)?;
                }
                Err(key) => ui.status.set_text(&self.tr.text(key))?,
            }
            ui.record.set_text(&self.tr.text("voice-record"))?;
        } else if !self.fixture {
            ui.clip.borrow_mut().take();
            ui.send.set_enabled(false)?;
            match voice::Recorder::start() {
                Ok(recorder) => {
                    *ui.recorder.borrow_mut() = Some(recorder);
                    ui.record.set_text(&self.tr.text("voice-stop"))?;
                }
                Err(key) => ui.status.set_text(&self.tr.text(key))?,
            }
        }
        Ok(())
    }
    pub fn extras_tick(&self) -> Result<()> {
        let ui = &self.view.extras;
        if self.view.ui.panel.get() != Panel::Composer {
            ui.reset();
            return Ok(());
        }
        let duration = ui.recorder.borrow().as_ref().map(|r| r.duration());
        if let Some(ms) = duration {
            ui.status.set_text(&format!(
                "{} · {:02}:{:02} / 02:00",
                self.tr.text("voice-recording"),
                ms / 60000,
                (ms / 1000) % 60
            ))?;
            if ms >= 120000 {
                self.record_voice()?;
            }
        }
        let busy = self.model.borrow().tools_busy();
        ui.send
            .set_enabled(!busy && (ui.mode.get() == 2 || ui.clip.borrow().is_some()))?;
        ui.record.set_enabled(!busy)?;
        if let Some(key) = self.model.borrow().tools.error {
            ui.status.set_text(&self.tr.text(key))?;
        }
        Ok(())
    }
    fn send_extra(&self) -> Result<()> {
        let ui = &self.view.extras;
        let Some((epoch, room)) = ui.target.borrow().clone() else {
            return Ok(());
        };
        let m = self.model.borrow();
        if self.fixture || epoch != m.epoch || m.selected.as_ref() != Some(&room) || !m.can_tool() {
            return Ok(());
        }
        drop(m);
        let action = if ui.mode.get() == 2 {
            ToolAction::Poll {
                question: ui.question.text()?,
                answers: ui.answers.text()?.lines().map(str::to_owned).collect(),
            }
        } else if let Some(clip) = ui.clip.borrow().as_ref() {
            ToolAction::Voice {
                path: clip.file.path().into(),
                duration_ms: clip.duration_ms,
            }
        } else {
            return Ok(());
        };
        let request = ui
            .request
            .borrow()
            .as_ref()
            .filter(|r| r.room_id == room && r.action == action)
            .cloned()
            .unwrap_or_else(|| ToolRequest {
                room_id: room,
                id: transaction_id(),
                action,
            });
        *ui.request.borrow_mut() = Some(request.clone());
        self.submit_tool(request)
    }
}
