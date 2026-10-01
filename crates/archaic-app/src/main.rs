mod appearance;
mod join_form;
mod login;
mod model;
mod options;
mod view;
mod widgets;
use archaic_i18n::Translator;
use archaic_matrix::{Command, Event, Sender};
use cui::{App, Result};
use model::{Model, Phase};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use view::View;

struct Controller {
    view: View,
    model: RefCell<Model>,
    tr: Translator,
    sender: Sender,
    rendering: Cell<bool>,
    room_labels: RefCell<Vec<String>>,
    fixture: bool,
    app: std::rc::Weak<App>,
    preference_path: Option<std::path::PathBuf>,
}

impl Controller {
    fn render(&self) -> Result<()> {
        self.rendering.set(true);
        let result = self.render_inner();
        self.rendering.set(false);
        result
    }
    fn render_inner(&self) -> Result<()> {
        let model = self.model.borrow();
        let visible = model.visible_rooms();
        let labels = visible
            .iter()
            .map(|room| room.name.replace('\0', "�"))
            .collect::<Vec<_>>();
        if *self.room_labels.borrow() != labels {
            self.view
                .rooms
                .set_items(&labels.iter().map(String::as_str).collect::<Vec<_>>())?;
            *self.room_labels.borrow_mut() = labels;
        }
        let index = visible
            .iter()
            .position(|room| Some(&room.id) == model.selected.as_ref())
            .map(|index| index as i32)
            .unwrap_or(-1);
        if self.view.rooms.get_selected()? != index {
            self.view.rooms.set_selected(index)?;
        }
        self.view.render(&model, &self.tr, self.fixture)
    }
    fn enqueue(&self, command: Command) -> bool {
        if self.sender.try_send(command).is_ok() {
            true
        } else {
            self.model.borrow_mut().error = Some("queue-full");
            false
        }
    }
    fn session_action(&self, temporary: bool) -> Result<()> {
        let epoch = self.model.borrow().epoch + 1;
        let command = if temporary {
            Command::Temporary { epoch }
        } else {
            Command::Restore { epoch }
        };
        if self.enqueue(command) {
            let mut model = self.model.borrow_mut();
            model.epoch = epoch;
            model.phase = Phase::Restoring;
            model.temporary = temporary;
            model.error = None;
        }
        self.render()
    }
    fn temporary(&self) -> Result<()> {
        self.session_action(true)
    }
    fn login(&self) -> Result<()> {
        if self.model.borrow().phase == Phase::RestoreBlocked {
            return self.session_action(false);
        }
        if self.model.borrow().phase != Phase::SignedOut {
            return Ok(());
        }
        let epoch = self.model.borrow().epoch + 1;
        let command = Command::Login {
            epoch,
            server: self.view.server.text()?,
            username: self.view.username.text()?,
            password: self.view.password.text()?,
        };
        if self.enqueue(command) {
            let mut model = self.model.borrow_mut();
            model.epoch = epoch;
            model.phase = Phase::Connecting;
            model.status = "signing-in";
            model.error = None;
            drop(model);
            self.view.password.set_text("")?;
        }
        self.render()
    }
    fn remember_draft(&self) -> Result<()> {
        let text = self.view.composer.text()?;
        let mut model = self.model.borrow_mut();
        if let Some(id) = model.selected.clone() {
            model.drafts.insert(id, text);
        }
        Ok(())
    }
    fn select(&self) -> Result<()> {
        self.remember_draft()?;
        let index = self.view.rooms.get_selected()?;
        let id = usize::try_from(index).ok().and_then(|index| {
            self.model
                .borrow()
                .visible_rooms()
                .get(index)
                .map(|room| room.id.clone())
        });
        if let Some(id) = id {
            if self.model.borrow().selected.as_ref() == Some(&id) {
                return Ok(());
            }
            if self.fixture || self.enqueue(Command::Select(id.clone())) {
                let mut model = self.model.borrow_mut();
                model.selected = Some(id);
                model.messages.clear();
                model.status = if self.fixture {
                    "preview-fixture"
                } else {
                    "loading"
                };
                model.error = None;
            }
        }
        self.render()
    }
    fn send(&self) -> Result<()> {
        self.remember_draft()?;
        let command = self.model.borrow_mut().send_command();
        if let Some(command) = command
            && self.enqueue(command)
        {
            let mut model = self.model.borrow_mut();
            model.sending = true;
            model.error = None;
        }
        self.render()
    }
    fn logout(&self) -> Result<()> {
        if self.enqueue(Command::Logout) {
            let mut model = self.model.borrow_mut();
            model.phase = Phase::SigningOut;
            model.status = "signing-out";
            model.error = None;
        }
        self.render()
    }
    fn refresh(&self) -> Result<()> {
        self.remember_draft()?;
        if self.enqueue(Command::Refresh) {
            self.model.borrow_mut().error = None;
        }
        self.render()
    }
    fn filter(&self) -> Result<()> {
        self.remember_draft()?;
        self.model.borrow_mut().filter = self.view.search.text()?;
        self.render()
    }
    fn appearance(&self) -> Result<()> {
        let appearance = appearance::Appearance::from_index(self.view.appearance.get_selected()?);
        if let Some(app) = self.app.upgrade() {
            app.theme(appearance.theme());
        }
        let failed = self
            .preference_path
            .as_ref()
            .is_some_and(|path| appearance.save(path).is_err());
        self.view.appearance_error.set_visible(failed)?;
        Ok(())
    }
    fn toggle_join(&self) -> Result<()> {
        let mut model = self.model.borrow_mut();
        if model.phase != Phase::Connected || model.joining || self.fixture {
            return Ok(());
        }
        model.show_join = !model.show_join;
        model.join_error = None;
        drop(model);
        self.render()
    }
    fn join(&self) -> Result<()> {
        if self.fixture
            || self.model.borrow().joining
            || self.model.borrow().phase != Phase::Connected
        {
            return Ok(());
        }
        self.remember_draft()?;
        let address = self.view.join.join_address.text()?;
        if self.enqueue(Command::Join(address)) {
            let mut model = self.model.borrow_mut();
            model.joining = true;
            model.join_error = None;
        }
        self.render()
    }
    fn event(&self, event: Event) -> Result<()> {
        self.remember_draft()?;
        let joined = event.epoch == self.model.borrow().epoch
            && matches!(event.kind, archaic_matrix::EventKind::Joined(_));
        self.model.borrow_mut().apply(event);
        if joined {
            self.rendering.set(true);
            let cleared = self
                .view
                .search
                .set_text("")
                .and_then(|_| self.view.join.join_address.set_text(""));
            self.rendering.set(false);
            cleared?;
        }
        self.render()
    }
    fn bind(self: &Rc<Self>) -> Result<()> {
        let controller = self.clone();
        self.view.password.on_key(move |key, _| {
            if key == cui::sys::CUI_KEY_ENTER && !controller.rendering.get() {
                controller.login().expect("live CUI controls");
                true
            } else {
                false
            }
        })?;
        let controller = self.clone();
        self.view.join.join_address.on_key(move |key, _| {
            if key == cui::sys::CUI_KEY_ENTER && !controller.rendering.get() {
                controller.join().expect("live CUI controls");
                true
            } else {
                false
            }
        })?;
        for (widget, action) in [
            (&self.view.sign_in, Self::login as fn(&Self) -> Result<()>),
            (&self.view.temporary, Self::temporary),
            (&self.view.rooms, Self::select),
            (&self.view.send, Self::send),
            (&self.view.sign_out, Self::logout),
            (&self.view.refresh, Self::refresh),
            (&self.view.search, Self::filter),
            (&self.view.join_toggle, Self::toggle_join),
            (&self.view.join.join_cancel, Self::toggle_join),
            (&self.view.join.join_submit, Self::join),
            (&self.view.appearance, Self::appearance),
        ] {
            let controller = self.clone();
            widget.on_action(move |_| {
                if !controller.rendering.get() {
                    action(&controller).expect("live CUI controls");
                }
            })?;
        }
        Ok(())
    }
}

fn fixture() -> Model {
    use archaic_matrix::{Message, RoomSummary};
    Model {
        phase: Phase::Connected, user: "@alex:example.org".into(),
        rooms: vec![RoomSummary { id: "!design:example.org".into(), name: "Design studio".into() }, RoomSummary { id: "!general:example.org".into(), name: "Archaic".into() }],
        selected: Some("!design:example.org".into()),
        messages: vec![Message { id: "1".into(), sender: "@maya:example.org".into(), body: Some("A place for thoughtful conversations.\nNative controls, a calm workspace, and room to focus.".into()) }, Message { id: "2".into(), sender: "@alex:example.org".into(), body: Some("This is an explicit offline visual-test fixture.\nThe normal application starts at real Matrix sign-in.".into()) }],
        ..Model::default()
    }
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let Some(options) = options::parse()? else {
        return Ok(());
    };
    let temporary = options.temporary || options.fixture_mode || options.smoke;
    let preference_path = if temporary {
        None
    } else {
        appearance::preference_path()
    };
    let appearance = options.theme.unwrap_or_else(|| {
        preference_path
            .as_deref()
            .map(appearance::Appearance::load)
            .unwrap_or_default()
    });
    let app = Rc::new(App::new()?);
    app.theme(appearance.theme());
    let window = app.window("Archaic", 1180, 800)?;
    window.scrollable(true)?;
    let root = window.root()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let (sender, commands, events, mut receiver) = archaic_matrix::channels();
    let worker = runtime.spawn(async move {
        if temporary {
            archaic_matrix::run(commands, events).await;
        } else {
            archaic_matrix::run_persistent(commands, events).await;
        }
    });
    let tr = Translator::new(&options.locale);
    let controller = Rc::new(Controller {
        view: View::new(&root, &tr)?,
        model: RefCell::new(if options.fixture_mode {
            fixture()
        } else {
            Model {
                temporary,
                phase: if temporary {
                    Phase::SignedOut
                } else {
                    Phase::Restoring
                },
                ..Model::default()
            }
        }),
        tr,
        sender,
        rendering: Cell::new(false),
        room_labels: RefCell::new(Vec::new()),
        fixture: options.fixture_mode,
        app: Rc::downgrade(&app),
        preference_path,
    });
    controller
        .view
        .appearance
        .set_selected(appearance.index())?;
    controller.bind()?;
    controller.render()?;
    let updates = controller.clone();
    // Bounded main-thread polling until CUI exposes a worker-safe dispatcher.
    let _pump = app.every(50, move || {
        for _ in 0..8 {
            let Ok(event) = receiver.try_recv() else {
                break;
            };
            updates.event(event).expect("live CUI controls");
        }
    })?;
    let _smoke_timer = if options.smoke {
        Some(app.every(1800, move || {
            root.quit().expect("live root");
        })?)
    } else {
        None
    };
    window.show()?;
    let result = app.run();
    worker.abort();
    runtime.shutdown_timeout(std::time::Duration::from_secs(1));
    result?;
    Ok(())
}
