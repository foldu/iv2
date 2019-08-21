#![feature(async_await)]

mod config;
mod events;
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
use std::time::Instant;

fn gtk_run() -> Result<(), Error> {
    let (_, config) = config::Config::load_or_write_default().context(ReadConfig)?;
    let opt = Opt::from_args();

    let main = widgets::Main::new();
    let (main_tx, main_rx) = glib::MainContext::channel(glib::source::PRIORITY_HIGH);
    let tx = main_tx.clone();
    let window = cascade! {
        gtk::Window::new(gtk::WindowType::Toplevel);
        ..add(main.as_ref());
        ..set_default_size(640, 480);
        ..set_title("iv");
        ..connect_delete_event(move |_, _| {
            let _ = tx.send(Event::Quit);
            Inhibit(false)
        });
    };
    let tx = main_tx.clone();
    window.connect_key_press_event(move |_, key_evt| {
        if let Some(user_event) = config
            .keymap
            .get(&KeyPress(key_evt.get_keyval(), key_evt.get_state()))
        {
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
        event_tx: main_tx,
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
            Event::ImageLoaded { id, img } => {
                if Some(id) == app.cursor.map(|cursor| cursor.id()) {
                    let img_widget = &main.image.image;
                    let alloc = img_widget.get_allocation();
                    let resized = img
                        .scale_simple(alloc.width, alloc.height, gdk_pixbuf::InterpType::Bilinear)
                        .unwrap();
                    img_widget.set_from_pixbuf(Some(&resized));
                    app.state = State::DisplayImage { img };
                }
            }
        }
        Continue(true)
    });

    window.show_all();

    gtk::main();

    Ok(())
}

struct AppCtx {
    g_ctx: glib::MainContext,
    event_tx: glib::Sender<Event>,
}

impl AppCtx {
    fn load_image(&self, id: DefaultKey, path: &str) -> future::AbortHandle {
        let file = gio::File::new_for_path(path);
        let tx = self.event_tx.clone();
        let fut = async move {
            let fh = file
                .read_async_future(glib::source::PRIORITY_DEFAULT)
                .await
                .unwrap();
            let img = gdk_pixbuf::Pixbuf::new_from_stream_async_future(&fh).await;
            if let Ok(img) = img {
                let _ = tx.send(Event::ImageLoaded { img, id });
            }
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
