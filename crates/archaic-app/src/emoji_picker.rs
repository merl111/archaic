//! Application-owned emoji and reaction picker; native Unicode text throughout.
use crate::{Controller, daylight::style, view::set_text};
use archaic_i18n::Translator;
use cui::{App, ChatComponent, ChatEvent, Result, Widget, Window, chat::Theme, sys};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
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
    ("😁", "beaming grin smile"),
    ("😆", "laugh squint"),
    ("🤣", "rolling laughing rofl"),
    ("🙂", "slightly smiling"),
    ("😘", "kiss love"),
    ("😗", "kissing"),
    ("😚", "kiss closed eyes"),
    ("😋", "delicious yum"),
    ("😛", "tongue"),
    ("😜", "wink tongue"),
    ("🤪", "crazy zany"),
    ("🤨", "raised eyebrow"),
    ("🧐", "monocle curious"),
    ("🤓", "nerd glasses"),
    ("🥳", "party birthday"),
    ("😏", "smirk"),
    ("😒", "unamused"),
    ("😞", "disappointed"),
    ("😔", "pensive"),
    ("😟", "worried"),
    ("😕", "confused"),
    ("😤", "angry triumph"),
    ("😠", "angry"),
    ("😡", "rage angry"),
    ("🤬", "swearing"),
    ("🤯", "mind blown"),
    ("😳", "flushed embarrassed"),
    ("🥺", "pleading please"),
    ("🥹", "tears touched"),
    ("😨", "fear scared"),
    ("😱", "scream shocked"),
    ("😢", "cry tear"),
    ("😓", "sweat sad"),
    ("🤗", "hug"),
    ("🤭", "giggle hand mouth"),
    ("🫢", "gasp surprise"),
    ("🫡", "salute"),
    ("🤫", "quiet shush"),
    ("🤥", "lying"),
    ("😶", "speechless"),
    ("😐", "neutral"),
    ("😑", "expressionless"),
    ("🙄", "eye roll"),
    ("😬", "grimace"),
    ("😴", "sleep tired"),
    ("🥱", "yawn tired"),
    ("🤤", "drool"),
    ("🤒", "ill fever"),
    ("🤕", "hurt bandage"),
    ("🤢", "nauseous sick"),
    ("🤮", "vomit"),
    ("🤧", "sneeze"),
    ("😷", "mask"),
    ("🥶", "cold freezing"),
    ("🥵", "hot heat"),
    ("😇", "angel innocent"),
    ("😈", "devil"),
    ("👻", "ghost spooky"),
    ("💀", "skull dead"),
    ("🤖", "robot bot"),
    ("👽", "alien"),
    ("💩", "poop"),
    ("🙈", "see no evil monkey"),
    ("🙉", "hear no evil monkey"),
    ("🙊", "speak no evil monkey"),
    ("🫶", "heart hands love"),
    ("👌", "ok hand"),
    ("🤌", "pinched fingers"),
    ("🤏", "small pinch"),
    ("✌️", "peace victory"),
    ("🤞", "crossed fingers luck"),
    ("🤟", "love you hand"),
    ("🤘", "rock horns"),
    ("🤙", "call me"),
    ("👈", "point left"),
    ("👉", "point right"),
    ("👆", "point up"),
    ("👇", "point down"),
    ("✋", "stop hand"),
    ("🤚", "raised hand"),
    ("✊", "fist solidarity"),
    ("👊", "fist bump"),
    ("🖖", "vulcan live long"),
    ("🫰", "finger heart"),
    ("👍🏻", "thumbs up light skin"),
    ("👍🏼", "thumbs up medium light skin"),
    ("👍🏽", "thumbs up medium skin"),
    ("👍🏾", "thumbs up medium dark skin"),
    ("👍🏿", "thumbs up dark skin"),
    ("👋🏻", "wave light skin"),
    ("👋🏼", "wave medium light skin"),
    ("👋🏽", "wave medium skin"),
    ("👋🏾", "wave medium dark skin"),
    ("👋🏿", "wave dark skin"),
    ("💔", "broken heart"),
    ("💕", "two hearts"),
    ("💖", "sparkling heart"),
    ("💗", "growing heart"),
    ("💘", "heart arrow"),
    ("💝", "heart gift"),
    ("🩵", "light blue heart"),
    ("🩷", "pink heart"),
    ("🩶", "gray grey heart"),
    ("💥", "boom collision"),
    ("💫", "dizzy star"),
    ("💦", "sweat water"),
    ("💤", "sleep zzz"),
    ("🎈", "balloon birthday"),
    ("🎁", "gift present"),
    ("🏆", "trophy winner"),
    ("🥇", "gold medal first"),
    ("🎯", "target bullseye"),
    ("⚽", "football soccer"),
    ("🏀", "basketball"),
    ("🎸", "guitar music"),
    ("🎧", "headphones music"),
    ("🎬", "movie film"),
    ("📷", "camera photo"),
    ("📚", "books read"),
    ("📝", "memo note write"),
    ("📌", "pin"),
    ("📎", "paperclip attachment"),
    ("🔗", "link"),
    ("🔒", "lock secure"),
    ("🔑", "key"),
    ("🛠️", "tools work"),
    ("⚙️", "gear settings"),
    ("🐧", "penguin linux"),
    ("🐼", "panda"),
    ("🐨", "koala"),
    ("🦁", "lion"),
    ("🐸", "frog"),
    ("🐢", "turtle slow"),
    ("🦋", "butterfly"),
    ("🐝", "bee"),
    ("🌱", "seedling grow"),
    ("🌲", "tree"),
    ("🌸", "cherry blossom"),
    ("🌹", "rose flower"),
    ("🍀", "clover luck"),
    ("🌍", "earth globe europe africa"),
    ("🌎", "earth globe america"),
    ("🌏", "earth globe asia"),
    ("⚡", "lightning fast"),
    ("❄️", "snowflake winter"),
    ("🌧️", "rain weather"),
    ("🌊", "wave sea ocean"),
    ("🍎", "apple"),
    ("🍌", "banana"),
    ("🍓", "strawberry"),
    ("🍉", "watermelon"),
    ("🍔", "burger"),
    ("🍟", "fries"),
    ("🌮", "taco"),
    ("🍣", "sushi"),
    ("🍪", "cookie"),
    ("🍩", "donut"),
    ("🍦", "ice cream"),
    ("🍫", "chocolate"),
    ("🫖", "tea"),
    ("🥤", "drink soda"),
    ("🍷", "wine"),
    ("🥂", "cheers champagne"),
    ("✈️", "airplane travel"),
    ("🚗", "car"),
    ("🚲", "bicycle bike"),
    ("🚆", "train"),
    ("🏠", "house home"),
    ("🏖️", "beach holiday"),
    ("⛰️", "mountain"),
    ("⏰", "alarm time"),
    ("⌛", "hourglass wait"),
    ("🔔", "bell notification"),
    ("📢", "announcement"),
    ("🚧", "construction work"),
    ("⚠️", "warning"),
    ("🚨", "alert emergency"),
    ("⛔", "stop no entry"),
    ("❓", "question"),
    ("❗", "exclamation important"),
    ("🔴", "red circle"),
    ("🟢", "green circle"),
    ("🔵", "blue circle"),
    ("🏳️‍🌈", "rainbow pride flag"),
    ("🇦🇹", "austria flag"),
    ("🇩🇪", "germany flag"),
    ("🇬🇧", "uk united kingdom flag"),
    ("🇺🇸", "us united states flag"),
    ("🇫🇷", "france flag"),
    ("🇳🇱", "netherlands flag"),
    ("🇪🇸", "spain flag"),
    ("🇮🇹", "italy flag"),
    ("🇨🇭", "switzerland flag"),
    ("🇺🇦", "ukraine flag"),
    ("🇯🇵", "japan flag"),
    ("🇨🇦", "canada flag"),
    ("🇧🇷", "brazil flag"),
    ("🇦🇺", "australia flag"),
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
    pub window: Option<Window>,
    pub more: Widget,
    pub previous: Widget,
    pub next: Widget,
    page_label: Widget,
    page: Cell<usize>,
    last_query: RefCell<String>,
    expanded: Widget,
    expanded_mode: Cell<bool>,
    quick_buttons: Vec<Widget>,
    cells: Vec<Widget>,
    anchor: RefCell<Option<(Widget, u32)>>,
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
        Self::build(root, None, tr, t)
    }
    pub fn popup(app: &App, tr: &Translator, t: &Theme) -> Result<Self> {
        let window = app.window(&tr.text("daylight-reactions"), 404, 126)?;
        window.frame(false, false, 24.)?;
        let root = window.root()?;
        root.box_set_padding(0)?;
        Self::build(root, Some(window), tr, t)
    }
    fn build(root: Widget, window: Option<Window>, tr: &Translator, t: &Theme) -> Result<Self> {
        style(&root, t, t.surface, 24., 24)?;
        let content = root.box_layout(sys::CUI_VERTICAL, 16)?;
        let head = content.box_layout(sys::CUI_HORIZONTAL, 12)?;
        let title = head.label(&tr.text(if window.is_some() {
            "daylight-reactions"
        } else {
            "daylight-emoji"
        }))?;
        title.set_font("sans", 16.5, 700)?;
        title.expand(true)?;
        let close = head.symbol_button(sys::CUI_SYMBOL_CLOSE, &tr.text("cancel"))?;
        let events = Rc::new(RefCell::new(Vec::new()));
        let quick = content.box_layout(sys::CUI_HORIZONTAL, 6)?;
        let mut quick_buttons = Vec::new();
        for glyph in ["👍", "❤️", "😂", "🎉", "😮", "😭"] {
            let b = quick.button(glyph)?;
            b.set_font("sans", 18., 400)?;
            style(&b, t, t.selected, 10., 6)?;
            b.accessibility(
                EMOJI
                    .iter()
                    .find(|e| e.0 == glyph)
                    .map(|e| e.1)
                    .unwrap_or(glyph),
                "",
            )?;
            let queue = events.clone();
            b.on_action(move |_| queue.borrow_mut().push(glyph.to_owned()))?;
            quick_buttons.push(b);
        }
        let more = quick.symbol_button(sys::CUI_SYMBOL_PLUS, &tr.text("daylight-emoji-more"))?;
        quick.set_visible(window.is_some())?;
        let expanded = content.box_layout(sys::CUI_VERTICAL, 16)?;
        let query = expanded.search(&tr.text("daylight-emoji-search"))?;
        query.accessibility(&tr.text("daylight-emoji-search"), "")?;
        let grid = expanded.grid(8, 8)?;
        let choices = Rc::new(RefCell::new(Vec::<String>::new()));
        let mut buttons = Vec::new();
        let mut cells = Vec::new();
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
            cells.push(cell);
        }
        let empty = expanded.label(&tr.text("daylight-emoji-empty"))?;
        let pager = expanded.box_layout(sys::CUI_HORIZONTAL, 12)?;
        let previous = pager.button(&tr.text("emoji-previous"))?;
        let page_label = pager.label("")?;
        page_label.expand(true)?;
        let next = pager.button(&tr.text("emoji-next"))?;
        let insert = expanded.button(&tr.text("daylight-emoji-insert"))?;
        root.set_visible(false)?;
        let result = Self {
            root,
            window,
            more,
            previous,
            next,
            page_label,
            page: Cell::new(0),
            last_query: RefCell::new(String::new()),
            expanded,
            expanded_mode: Cell::new(false),
            quick_buttons,
            cells,
            anchor: Default::default(),
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
    pub fn expand(&self, expanded: bool) -> Result<()> {
        self.expanded_mode.set(expanded);
        self.expanded.set_visible(expanded)?;
        if let Some(window) = &self.window {
            window.set_size(
                if expanded { 480 } else { 404 },
                if expanded { 638 } else { 126 },
            )?;
            if let Some((canvas, region)) = self.anchor.borrow().as_ref() {
                window.popup_region(canvas, *region)?;
            }
        }
        if expanded {
            self.query.focus()?;
        }
        Ok(())
    }
    pub fn theme(&self, t: &Theme) -> Result<()> {
        self.root.set_style(Some(&sys::cui_widget_style {
            background: t.surface,
            foreground: t.foreground,
            border: t.border,
            border_width: 1.,
            radius: 24.,
            padding: 24,
        }))?;
        style(&self.query, t, t.selected, 12., 10)?;
        for b in self.buttons.iter().chain(self.quick_buttons.iter()).chain([
            &self.close,
            &self.insert,
            &self.more,
            &self.previous,
            &self.next,
        ]) {
            style(b, t, t.selected, 10., 6)?;
        }
        Ok(())
    }
    pub fn filter(&self) -> Result<()> {
        let query = self.query.text()?.trim().to_lowercase();
        if *self.last_query.borrow() != query {
            self.page.set(0);
            *self.last_query.borrow_mut() = query.clone();
        }
        let normalized = query.trim_matches(':').replace('_', " ");
        let matches = EMOJI
            .iter()
            .filter(|(glyph, name)| {
                query.is_empty()
                    || glyph.contains(&query)
                    || normalized
                        .split_whitespace()
                        .all(|word| name.contains(word))
            })
            .collect::<Vec<_>>();
        let pages = matches.len().div_ceil(56).max(1);
        let page = self.page.get().min(pages - 1);
        self.page.set(page);
        self.previous.set_enabled(page > 0)?;
        self.next.set_enabled(page + 1 < pages)?;
        self.page_label
            .set_text(&format!("{} / {}", page + 1, pages))?;
        let matches = matches
            .into_iter()
            .skip(page * 56)
            .take(56)
            .collect::<Vec<_>>();
        *self.choices.borrow_mut() = matches.iter().map(|v| v.0.to_owned()).collect();
        for (i, b) in self.buttons.iter().enumerate() {
            self.cells[i].set_visible(i < matches.len())?;
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
    fn active_picker(&self) -> &Picker {
        if self.view.reaction.target.borrow().is_some() {
            &self.view.reaction
        } else {
            &self.view.emoji
        }
    }
    pub fn show_reactions(&self, chat: &ChatComponent, event: &ChatEvent) -> Result<()> {
        let raw = sys::cui_chat_event {
            action: event.action,
            id: event.id,
            detail_id: event.detail,
            index: event.index as u32,
            modifiers: event.modifiers,
            text: std::ptr::null(),
        };
        let Some(region) = chat.region(&raw) else {
            return Ok(());
        };
        let canvas = chat.part(0)?;
        canvas.canvas_focus_region(region)?;
        self.view.reaction.anchor.borrow_mut().take();
        self.show_emoji(Kind::Reaction)?;
        let picker = &self.view.reaction;
        *picker.anchor.borrow_mut() = Some((canvas.clone(), region));
        let window = picker.window.as_ref().expect("reaction popup");
        if !window.popup_region(&canvas, region)? {
            return self.close_emoji();
        }
        picker.more.focus()?;
        Ok(())
    }
    pub fn expand_reactions(&self) -> Result<()> {
        self.view
            .reaction
            .expand(!self.view.reaction.expanded_mode.get())
    }
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
        let picker = if kind == Kind::Reaction {
            &self.view.reaction
        } else {
            &self.view.emoji
        };
        *picker.target.borrow_mut() = Some(target);
        picker.query.set_text("")?;
        picker.filter()?;
        picker.expand(kind != Kind::Reaction)?;
        self.render()?;
        if kind != Kind::Reaction {
            picker.query.focus()?;
        }
        Ok(())
    }
    pub fn previous_emoji_page(&self) -> Result<()> {
        let p = self.active_picker();
        p.page.set(p.page.get().saturating_sub(1));
        p.filter()
    }
    pub fn next_emoji_page(&self) -> Result<()> {
        let p = self.active_picker();
        p.page.set(p.page.get() + 1);
        p.filter()
    }
    pub fn filter_emoji(&self) -> Result<()> {
        self.active_picker().filter()
    }
    pub fn close_emoji(&self) -> Result<()> {
        self.view.emoji.target.borrow_mut().take();
        self.view.reaction.target.borrow_mut().take();
        self.view.reaction.events.borrow_mut().clear();
        self.view
            .reaction
            .window
            .as_ref()
            .expect("reaction popup")
            .close()?;
        self.render()?;
        if let Some((canvas, region)) = self.view.reaction.anchor.borrow_mut().take() {
            canvas.focus()?;
            canvas.canvas_focus_region(region)?;
        }
        Ok(())
    }
    pub fn insert_typed_emoji(&self) -> Result<()> {
        let value = self.active_picker().query.text()?;
        if !value.trim().is_empty() && !value.is_ascii() && value.len() <= 64 {
            self.choose_emoji(&value)?;
        }
        Ok(())
    }
    pub fn choose_emoji(&self, glyph: &str) -> Result<()> {
        let Some(target) = self.active_picker().target.borrow_mut().take() else {
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
            let value = input.text()?;
            if value.len() + glyph.len() <= 65536 {
                input.insert_text(glyph)?;
            }
            if target.kind == Kind::Compose {
                self.composer_input()?;
            }
            input.focus()?;
        }
        Ok(())
    }
}
