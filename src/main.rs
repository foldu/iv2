#![feature(bind_by_move_pattern_guards)]
mod config;
mod events;
mod percent;
mod ratio;
mod state;
mod widgets;

use cascade::cascade;
use cfgen::prelude::CfgenDefault;
use futures::{future, prelude::*};
use gio::prelude::*;
use glib::prelude::*;
use gtk::prelude::*;
use linked_slotlist::{Cursor, DefaultKey, LinkedSlotlist};
use snafu::{ResultExt, Snafu};
use structopt::StructOpt;

use crate::events::{Event, KeyPress};

fn gtk_run() -> Result<(), Error> {
    let (_, config) = config::UserConfig::load_or_write_default().context(ReadConfig)?;
    let opt = Opt::from_args();
    let (keymap, config) = config.split_for_app_use(config::ViewerMode::Image);

    let main = widgets::Main::new();
    let (main_tx, main_rx) = glib::MainContext::channel(glib::source::PRIORITY_HIGH);
    let tx = main_tx.clone();
    let window = cascade! {
        gtk::Window::new(gtk::WindowType::Toplevel);
        ..add(main.as_ref());
        ..set_title("iv - ");
        ..connect_delete_event(move |_, _| {
            let _ = tx.send(Event::Quit);
            Inhibit(false)
        });
    };

    let tx = main_tx.clone();
    window.connect_key_press_event(move |_, key_evt| {
        if let Some(user_event) = keymap.get(&KeyPress(key_evt.get_keyval(), key_evt.get_state())) {
            let _ = tx.send(Event::User(*user_event));
            Inhibit(true)
        } else {
            Inhibit(false)
        }
    });

    let tx = main_tx.clone();
    let g_ctx = glib::MainContext::default();
    let ctx = AppCtx {
        g_ctx,
        event_tx: main_tx.clone(),
    };
    let images: LinkedSlotlist<_> = opt.images.into_iter().collect();
    let cursor = images.head();
    let mut app = App {
        cursor,
        state: match cursor {
            Some(cursor) => State::LoadingImage {
                abort_handle: ctx.load_image(cursor.id(), images.get(cursor.id()).unwrap()),
            },
            None => State::NoImages,
        },
        images,
    };

    window.show_all();
    let ratio = ratio::gtk_win_scale(
        &window.get_window().unwrap(),
        config.mode.geometry.aspect_ratio,
        config.mode.geometry.scale,
    )
    .unwrap();
    window.resize(ratio.0, ratio.1);

    main_rx.attach(None, move |event| {
        match event {
            Event::Quit => {
                gtk::main_quit();
            }
            Event::User(action) => {
                use events::UserEvent;
                log::debug!("Received user action: {:#?}", action);
                match action {
                    UserEvent::Quit => {
                        log::debug!("User quit");
                        let _ = tx.send(Event::Quit);
                    }
                    UserEvent::Next => {
                        if app.try_load(&ctx, ImageTransition::Next) {
                            main.image.image.set_from_pixbuf(None);
                        }
                    }
                    UserEvent::Previous => {
                        if app.try_load(&ctx, ImageTransition::Prev) {
                            main.image.image.set_from_pixbuf(None);
                        }
                    }
                    _ => {}
                }
            }
            Event::ImageLoaded { id, result } => match result {
                Ok(img) if Some(id) == app.cursor.map(|cursor| cursor.id()) => {
                    let img_widget = &main.image.image;
                    let alloc = img_widget.get_allocation();
                    let (_, scaled) = ratio::Ratio::new(img.get_width(), img.get_height())
                        .unwrap()
                        .scale(alloc.width, alloc.height)
                        .unwrap();
                    let resized = img
                        .scale_simple(scaled.0, scaled.1, config.interpolation_algorithm)
                        .unwrap();
                    img_widget.set_from_pixbuf(Some(&resized));
                    app.state = State::DisplayImage { img };
                }
                Err(e) => {
                    if let Some(path) = app.images.remove(id) {
                        log::error!("Failed loading image {}: {}", path, e);
                    }
                }
                _ => {}
            },
        }
        Continue(true)
    });

    gtk::main();

    Ok(())
}

struct AppCtx {
    g_ctx: glib::MainContext,
    event_tx: glib::Sender<Event>,
}

async fn load_image(path: gio::File) -> Result<gdk_pixbuf::Pixbuf, glib::Error> {
    let fh = path
        .read_async_future(glib::source::PRIORITY_DEFAULT)
        .await?;
    gdk_pixbuf::Pixbuf::new_from_stream_async_future(&fh).await
}

impl AppCtx {
    fn load_image(&self, id: DefaultKey, path: &str) -> future::AbortHandle {
        let file = gio::File::new_for_path(path);
        let tx = self.event_tx.clone();
        let fut = async move {
            let result = load_image(file).await;
            let _ = tx.send(Event::ImageLoaded { result, id });
        };
        let (fut, handle) = future::abortable(fut);
        self.g_ctx.spawn_local(fut.map(|_| ()));
        handle
    }
}

struct App {
    cursor: Option<Cursor>,
    images: LinkedSlotlist<String>,
    state: State,
}

#[derive(Copy, Clone)]
enum ImageTransition {
    Next,
    Prev,
}

impl App {
    fn change_index(&self, transition: ImageTransition) -> Option<Cursor> {
        match (transition, self.cursor) {
            (ImageTransition::Prev, Some(cur)) => cur.prev_with(&self.images),
            (ImageTransition::Next, Some(cur)) => cur.next_with(&self.images),
            _ => None,
        }
    }

    fn try_load(&mut self, ctx: &AppCtx, transition: ImageTransition) -> bool {
        if let Some(cur) = self.change_index(transition) {
            self.state = match &self.state {
                State::NoImages => State::NoImages,
                State::LoadingImage { abort_handle } => {
                    abort_handle.abort();
                    State::LoadingImage {
                        // FIXME:
                        abort_handle: ctx.load_image(cur.id(), self.images.get(cur.id()).unwrap()),
                    }
                }
                State::DisplayImage { .. } => State::LoadingImage {
                    abort_handle: ctx.load_image(cur.id(), self.images.get(cur.id()).unwrap()),
                },
            };
            self.cursor = Some(cur);
            true
        } else {
            false
        }
    }
}

enum State {
    NoImages,
    LoadingImage { abort_handle: future::AbortHandle },
    DisplayImage { img: gdk_pixbuf::Pixbuf },
}

fn run() -> Result<(), Error> {
    gtk::init().map_err(|_| Error::InitGtk)?;
    if let Err(e) = gtk_run() {
        gtk::MessageDialog::new(
            None::<&gtk::Window>,
            gtk::DialogFlags::empty(),
            gtk::MessageType::Error,
            gtk::ButtonsType::Close,
            &e.to_string(),
        )
        .run();

        Err(e)
    } else {
        Ok(())
    }
}

#[derive(StructOpt, Debug)]
struct Opt {
    images: Vec<String>,
}

#[derive(Snafu, Debug)]
enum Error {
    #[snafu(display("Can't init gtk"))]
    InitGtk,

    #[snafu(display("Can't read config: {}", source))]
    ReadConfig { source: cfgen::Error },
}

fn main() {
    env_logger::init();
    if let Err(e) = run() {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
