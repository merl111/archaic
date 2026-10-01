mod model;
mod view;
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
    fn login(&self) -> Result<()> {
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
    fn event(&self, event: Event) -> Result<()> {
        self.remember_draft()?;
        self.model.borrow_mut().apply(event);
        self.render()
    }
    fn bind(self: &Rc<Self>) -> Result<()> {
        for (widget, action) in [
            (&self.view.sign_in, Self::login as fn(&Self) -> Result<()>),
            (&self.view.rooms, Self::select),
            (&self.view.send, Self::send),
            (&self.view.sign_out, Self::logout),
            (&self.view.refresh, Self::refresh),
            (&self.view.search, Self::filter),
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
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut locale = "en".to_string();
    let mut fixture_mode = false;
    let mut smoke = false;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--locale" => locale = args.next().ok_or("--locale requires en or de")?.clone(),
            "--fixture" => fixture_mode = true,
            "--smoke-test" => smoke = true,
            "--help" => {
                println!(
                    "Archaic [--locale en|de] [--fixture] [--smoke-test]\nDefault: real Matrix password sign-in. Fixture mode never connects."
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown option: {arg}").into()),
        }
    }
    let app = App::new()?;
    let window = app.window("Archaic", 1180, 800)?;
    let root = window.root()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let (sender, commands, events, mut receiver) = archaic_matrix::channels();
    let worker = runtime.spawn(archaic_matrix::run(commands, events));
    let tr = Translator::new(&locale);
    let controller = Rc::new(Controller {
        view: View::new(&root, &tr)?,
        model: RefCell::new(if fixture_mode {
            fixture()
        } else {
            Model::default()
        }),
        tr,
        sender,
        rendering: Cell::new(false),
        room_labels: RefCell::new(Vec::new()),
        fixture: fixture_mode,
    });
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
    let _smoke_timer = if smoke {
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
