#![feature(bind_by_move_pattern_guards)]
mod config;
mod events;
mod math;
mod widgets;

use std::{convert::TryFrom, path::Path};

use cascade::cascade;
use cfgen::prelude::CfgenDefault;
use euclid::{vec2, Vector2D};
use futures::{future, prelude::*};
use gio::prelude::*;
use glib::prelude::*;
use gtk::prelude::*;
use linked_slotlist::{DefaultKey, LinkedSlotlist};
use snafu::{ResultExt, Snafu};
use structopt::StructOpt;

use crate::events::{Event, KeyPress};
use math::Pixels;
use widgets::Scroll;

fn gtk_run() -> Result<(), Error> {
    let (_, config) = config::UserConfig::load_or_write_default().context(ReadConfig)?;
    let opt = Opt::from_args();
    let mode = {
        let probably_wants_to_read_archive = opt.images.iter().take(4).all(|file| {
            // clashes with something in gio so scoped import here
            use std::os::unix::prelude::*;

            let ext = Path::new(file).extension();
            ext.map(|ext| ext.as_bytes())
                .map(|ext| ext == b"cbz" || ext == b"zip")
                .unwrap_or(false)
        });

        if probably_wants_to_read_archive {
            config::ViewerMode::Archive
        } else {
            config::ViewerMode::Image
        }
    };
    let (keymap, config) = config.split_for_app_use(mode);

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
        let keypress = KeyPress(key_evt.get_keyval(), key_evt.get_state());
        log::debug!("{:?}", &keypress);
        if let Some(user_event) = keymap.get(&keypress) {
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
                abort_handle: ctx.load_image(cursor, images.get(cursor).unwrap()),
                last_transition: ImageTransition::Next,
            },
            None => State::NoImages,
        },
        images,
    };

    window.show_all();
    let ratio = gtk_win_scale(
        &window.get_window().unwrap(),
        config.mode.geometry.aspect_ratio.0,
        config.mode.geometry.scale.0,
    )
    .unwrap();
    window.resize(ratio.x, ratio.y);

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
                            main.set_image(None);
                        }
                    }
                    UserEvent::Previous => {
                        if app.try_load(&ctx, ImageTransition::Prev) {
                            main.set_image(None);
                        }
                    }
                    UserEvent::ZoomIn => {
                        app.zoom_in(&config, &main);
                    }
                    UserEvent::ZoomOut => {
                        app.zoom_out(&config, &main);
                    }
                    other => {
                        if let Ok(scroll) = Scroll::try_from(other) {
                            main.scroll(scroll);
                        } else {
                            log::debug!("Unhandled user input: {:?}", other);
                        }
                    }
                }
            }
            Event::ImageLoaded { id, result } => match result {
                Ok(img) if app.is_currently_loading_image(id) => {
                    let alloc = main.image_allocation();
                    let img_px = vec2(img.get_width(), img.get_height());

                    let (scaled, scale) = math::scale_with_aspect_ratio(alloc, img_px).unwrap();

                    let resized = img
                        .scale_simple(scaled.x, scaled.y, config.interpolation_algorithm)
                        .unwrap();
                    println!("{}", scale);
                    main.set_image(Some(&resized));

                    app.state = State::DisplayImage { img, scale };
                }
                Err(e) => {
                    if app.is_currently_loading_image(id) {
                        match app.state {
                            State::DisplayImage { .. } | State::NoImages => panic!("stop"),
                            State::LoadingImage {
                                last_transition, ..
                            } => {
                                app.try_load(&ctx, last_transition);
                            }
                        };
                    }
                    if let Some(path) = app.images.remove(id) {
                        log::error!("Failed loading image {}: {}", path, e);
                        // removed the last image
                        if app.images.head().is_none() {
                            app.state = State::NoImages;
                        }
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

pub fn gtk_win_scale(
    win: &gdk::Window,
    ratio: Vector2D<f64, Pixels>,
    fact: f64,
) -> Option<Vector2D<i32, Pixels>> {
    let disp = gdk::Display::get_default()?;
    let dims = disp.get_monitor_at_window(win)?.get_geometry();
    let dims = vec2(dims.width, dims.height).to_f64();
    let scaled = (dims * fact).floor();
    println!("{} {}", scaled, dims);
    math::scale_with_aspect_ratio(scaled, ratio).and_then(|(r, _)| r.try_cast())
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
    cursor: Option<DefaultKey>,
    images: LinkedSlotlist<String>,
    state: State,
}

#[derive(Copy, Clone)]
enum ImageTransition {
    Next,
    Prev,
}

impl App {
    fn change_index(&self, transition: ImageTransition) -> Option<DefaultKey> {
        match (transition, self.cursor) {
            (ImageTransition::Prev, Some(cur)) => self.images.prev(cur),
            (ImageTransition::Next, Some(cur)) => self.images.next(cur),
            _ => None,
        }
    }

    fn is_currently_loading_image(&self, id: DefaultKey) -> bool {
        Some(id) == self.cursor
    }

    fn try_load(&mut self, ctx: &AppCtx, transition: ImageTransition) -> bool {
        if let Some(cur) = self.change_index(transition) {
            self.state = match &self.state {
                State::NoImages => State::NoImages,
                State::LoadingImage { abort_handle, .. } => {
                    abort_handle.abort();
                    State::LoadingImage {
                        abort_handle: ctx.load_image(cur, self.images.get(cur).unwrap()),
                        last_transition: transition,
                    }
                }
                State::DisplayImage { .. } => State::LoadingImage {
                    abort_handle: ctx.load_image(cur, self.images.get(cur).unwrap()),
                    last_transition: transition,
                },
            };
            self.cursor = Some(cur);
            true
        } else {
            false
        }
    }

    fn zoom_in(&mut self, config: &config::Config, main: &widgets::Main) {
        if let State::DisplayImage { img, scale } = &self.state {
            self.state = {
                let next = math::step_next(*scale, config.zoom_step_size.0);
                let img_px: euclid::Vector2D<_, math::Pixels> =
                    vec2(img.get_width(), img.get_height());
                let scaled = (img_px.to_f64() * next).cast();
                let resized = img
                    .scale_simple(scaled.x, scaled.y, config.interpolation_algorithm)
                    .unwrap();
                main.set_image(Some(&resized));
                State::DisplayImage {
                    img: img.clone(),
                    scale: next,
                }
            };
        }
    }

    fn zoom_out(&mut self, config: &config::Config, main: &widgets::Main) {
        if let State::DisplayImage { img, scale } = &self.state {
            self.state = {
                let step_size = config.zoom_step_size.0;
                let next = f64::max(math::step_prev(*scale, step_size), step_size);
                let img_px: euclid::Vector2D<_, math::Pixels> =
                    vec2(img.get_width(), img.get_height());
                let scaled = (img_px.to_f64() * next).cast();
                let resized = img
                    .scale_simple(scaled.x, scaled.y, config.interpolation_algorithm)
                    .unwrap();
                main.set_image(Some(&resized));
                State::DisplayImage {
                    img: img.clone(),
                    scale: next,
                }
            };
        }
    }
}

enum State {
    NoImages,
    LoadingImage {
        abort_handle: future::AbortHandle,
        last_transition: ImageTransition,
    },
    DisplayImage {
        img: gdk_pixbuf::Pixbuf,
        scale: f64,
    },
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
