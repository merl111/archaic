//! Application-owned emoji and reaction picker; native Unicode text throughout.
use crate::{Controller, daylight::style, view::set_text};
use archaic_i18n::Translator;
use cui::{Result, Widget, chat::Theme, sys};
use std::{cell::RefCell, rc::Rc};
const EMOJI: &[(&str, &str)] = &[
    ("😀", "grin smile happy"),
    ("😊", "smile blush happy"),
    ("😂", "joy laugh"),
    ("🥰", "love hearts"),
    ("😍", "heart eyes love"),
    ("😎", "cool sunglasses"),
    ("🤔", "thinking"),
    ("😭", "cry sad"),
    ("😅", "sweat smile"),
    ("🙃", "upside down"),
    ("😉", "wink"),
    ("😮", "surprised wow"),
    ("❤️", "heart love red"),
    ("🧡", "heart orange"),
    ("💛", "heart yellow"),
    ("💚", "heart green"),
    ("💙", "heart blue"),
    ("💜", "heart purple"),
    ("🖤", "heart black"),
    ("🤍", "heart white"),
    ("👍", "thumbs up yes"),
    ("👎", "thumbs down no"),
    ("👋", "wave hello"),
    ("👏", "clap applause"),
    ("🙌", "raised hands hooray"),
    ("🙏", "please thanks pray"),
    ("💪", "strong muscle"),
    ("🤝", "handshake"),
    ("🎉", "party tada celebrate"),
    ("🎊", "confetti celebrate"),
    ("🔥", "fire hot"),
    ("✨", "sparkles"),
    ("⭐", "star"),
    ("💯", "hundred perfect"),
    ("✅", "check done yes"),
    ("❌", "cross no"),
    ("👀", "eyes looking"),
    ("🚀", "rocket launch"),
    ("💡", "idea bulb"),
    ("💬", "chat speech"),
    ("🐶", "dog"),
    ("🐱", "cat"),
    ("🦊", "fox"),
    ("🐻", "bear"),
    ("🌈", "rainbow"),
    ("☀️", "sun"),
    ("🌙", "moon"),
    ("🌻", "sunflower flower"),
    ("☕", "coffee"),
    ("🍕", "pizza"),
    ("🍰", "cake"),
    ("🍻", "cheers beer"),
    ("🎵", "music"),
    ("🎮", "game controller"),
    ("💻", "computer laptop"),
    ("🦀", "crab rust"),
];
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Compose,
    Thread,
    Reaction,
}
#[derive(Clone)]
pub struct Target {
    pub epoch: u64,
    pub room: String,
    pub event: Option<String>,
    pub kind: Kind,
}
pub struct Picker {
    pub root: Widget,
    pub query: Widget,
    pub close: Widget,
    pub insert: Widget,
    pub target: RefCell<Option<Target>>,
    pub events: Rc<RefCell<Vec<String>>>,
    choices: Rc<RefCell<Vec<String>>>,
    buttons: Vec<Widget>,
    empty: Widget,
}
impl Picker {
    pub fn new(layers: &Widget, tr: &Translator, t: &Theme) -> Result<Self> {
        let root = layers.stack_layer(sys::CUI_LAYER_CENTER, 480, 0, 24)?;
        style(&root, t, t.surface, 24., 24)?;
        let content = root.box_layout(sys::CUI_VERTICAL, 16)?;
        let head = content.box_layout(sys::CUI_HORIZONTAL, 12)?;
        let title = head.label(&tr.text("daylight-emoji"))?;
        title.set_font("sans", 16.5, 700)?;
        title.expand(true)?;
        let close = head.symbol_button(sys::CUI_SYMBOL_CLOSE, &tr.text("cancel"))?;
        let query = content.search(&tr.text("daylight-emoji-search"))?;
        query.accessibility(&tr.text("daylight-emoji-search"), "")?;
        let grid = content.grid(8, 8)?;
        let events = Rc::new(RefCell::new(Vec::new()));
        let choices = Rc::new(RefCell::new(Vec::<String>::new()));
        let mut buttons = Vec::new();
        for i in 0..56 {
            let cell = grid.grid_cell(i / 8, i % 8, 1, 1)?;
            let b = cell.button("")?;
            b.set_font("sans", 18., 400)?;
            b.set_min_size(40, 40)?;
            let queue = events.clone();
            let values = choices.clone();
            b.on_action(move |_| {
                if let Some(v) = values.borrow().get(i as usize) {
                    queue.borrow_mut().push(v.clone());
                }
            })?;
            buttons.push(b);
        }
        let empty = content.label(&tr.text("daylight-emoji-empty"))?;
        let insert = content.button(&tr.text("daylight-emoji-insert"))?;
        root.set_visible(false)?;
        let result = Self {
            root,
            query,
            close,
            insert,
            target: Default::default(),
            events,
            choices,
            buttons,
            empty,
        };
        result.theme(t)?;
        result.filter()?;
        Ok(result)
    }
    pub fn theme(&self, t: &Theme) -> Result<()> {
        style(&self.root, t, t.surface, 24., 24)?;
        style(&self.query, t, t.selected, 12., 10)?;
        for b in self.buttons.iter().chain([&self.close, &self.insert]) {
            style(b, t, t.selected, 10., 6)?;
        }
        Ok(())
    }
    pub fn filter(&self) -> Result<()> {
        let query = self.query.text()?.trim().to_lowercase();
        let matches = EMOJI
            .iter()
            .filter(|(glyph, name)| {
                query.is_empty() || glyph.contains(&query) || name.contains(query.trim_matches(':'))
            })
            .collect::<Vec<_>>();
        *self.choices.borrow_mut() = matches.iter().map(|v| v.0.to_owned()).collect();
        for (i, b) in self.buttons.iter().enumerate() {
            b.set_visible(i < matches.len())?;
            if let Some((glyph, name)) = matches.get(i) {
                set_text(b, glyph)?;
                b.accessibility(name, "")?;
                b.set_tooltip(name)?;
            }
        }
        self.empty.set_visible(matches.is_empty())?;
        self.insert
            .set_enabled(!query.is_ascii() && query.len() <= 64 && !query.contains('\0'))?;
        Ok(())
    }
}
impl Controller {
    pub fn show_emoji(&self, kind: Kind) -> Result<()> {
        self.remember_draft()?;
        let target = {
            let m = self.model.borrow();
            let Some(room) = m.selected.clone() else {
                return Ok(());
            };
            Target {
                epoch: m.epoch,
                room,
                event: m.message_selected.clone(),
                kind,
            }
        };
        *self.view.emoji.target.borrow_mut() = Some(target);
        self.view.emoji.query.set_text("")?;
        self.view.emoji.filter()?;
        self.render()?;
        self.view.emoji.query.focus().map(|_| ())
    }
    pub fn filter_emoji(&self) -> Result<()> {
        self.view.emoji.filter()
    }
    pub fn close_emoji(&self) -> Result<()> {
        self.view.emoji.target.borrow_mut().take();
        self.render()?;
        self.view.composer.focus().map(|_| ())
    }
    pub fn insert_typed_emoji(&self) -> Result<()> {
        let value = self.view.emoji.query.text()?;
        if !value.trim().is_empty() && !value.is_ascii() && value.len() <= 64 {
            self.choose_emoji(&value)?;
        }
        Ok(())
    }
    pub fn choose_emoji(&self, glyph: &str) -> Result<()> {
        let Some(target) = self.view.emoji.target.borrow_mut().take() else {
            return Ok(());
        };
        {
            let m = self.model.borrow();
            if m.epoch != target.epoch || m.selected.as_ref() != Some(&target.room) {
                return self.render();
            }
        }
        self.render()?;
        if target.kind == Kind::Reaction {
            self.model.borrow_mut().message_selected = target.event;
            self.view.ui.quick_reaction.set(true);
            self.react_message()?;
            self.view.message_controls.body.set_text(glyph)?;
            self.submit_message()?;
        } else {
            let input = if target.kind == Kind::Thread {
                self.view.thread_pane.composer.part(0)?
            } else {
                self.view.composer.clone()
            };
            let mut value = input.text()?;
            if value.len() + glyph.len() <= 65536 {
                value.push_str(glyph);
                input.set_text(&value)?;
            }
            if target.kind == Kind::Compose {
                self.composer_input()?;
            }
            input.focus()?;
        }
        Ok(())
    }
}
