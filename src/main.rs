#![feature(bind_by_move_pattern_guards)]
mod config;
mod context;
mod events;
mod math;
mod widgets;

use std::{convert::TryFrom, path::Path};

use cascade::cascade;
use cfgen::prelude::CfgenDefault;
use euclid::{vec2, Vector2D};
use formatter::FormatMap;
use futures::future;
use gdk_pixbuf::Pixbuf;
use glib::prelude::*;
use gtk::prelude::*;
use linked_slotlist::{DefaultKey, LinkedSlotlist};
use slotmap::SecondaryMap;
use snafu::{ResultExt, Snafu};
use structopt::StructOpt;

use crate::{
    context::AppCtx,
    events::{Event, KeyPress},
};
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
    let (keymap, config) = config.split_for_app_use(mode).context(Format)?;

    let main = widgets::Main::new();
    let (main_tx, main_rx) = glib::MainContext::channel(glib::source::PRIORITY_DEFAULT);
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
        let keypress = KeyPress(key_evt.get_keyval());
        log::debug!("{:?}", &keypress);
        if let Some(user_event) = keymap.get(&keypress) {
            let _ = tx.send(Event::User(*user_event));
            Inhibit(true)
        } else {
            Inhibit(false)
        }
    });

    let tx = main_tx.clone();
    window.connect_window_state_event(move |_, evt| {
        if evt
            .get_changed_mask()
            .contains(gdk::WindowState::FULLSCREEN)
        {
            let _ = tx.send(Event::WindowFullScreenToggle);
        }

        Inhibit(false)
    });

    let tx = main_tx.clone();
    let ctx = AppCtx::new(tx);

    let images: LinkedSlotlist<_> = opt.images.into_iter().collect();
    let cursor = images.head();
    let mut app = App {
        cursor,
        index: cursor.map(|_| 0),
        format_map: default_format_map(),
        state: match cursor {
            Some(cursor) => State::LoadingImage {
                abort_handle: ctx.load_image(cursor, images.get(cursor).unwrap().to_owned()),
                last_transition: ImageTransition::Next,
            },
            None => State::NoImages,
        },
        images_meta: SecondaryMap::with_capacity(images.len()),
        filenames: SecondaryMap::with_capacity(images.len()),
        images,
        config,
        is_fullscreen: false,
    };

    window.show_all();
    let ratio = gtk_win_scale(
        &window.get_window().unwrap(),
        app.config.mode.geometry.aspect_ratio.0,
        app.config.mode.geometry.scale.0,
    )
    .unwrap();
    window.resize(ratio.x, ratio.y);

    let tx = main_tx.clone();
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
                        app.try_load(&ctx, &main, ImageTransition::Next);
                    }
                    UserEvent::Previous => {
                        app.try_load(&ctx, &main, ImageTransition::Prev);
                    }
                    UserEvent::JumpToStart => {
                        app.try_load(&ctx, &main, ImageTransition::Start);
                    }
                    UserEvent::JumpToEnd => {
                        app.try_load(&ctx, &main, ImageTransition::End);
                    }
                    UserEvent::ZoomIn => {
                        app.zoom_in(&main);
                    }
                    UserEvent::ZoomOut => {
                        app.zoom_out(&main);
                    }
                    UserEvent::ScaleToFitCurrent => {
                        app.scale_to_fit(&main);
                    }
                    UserEvent::ToggleFullscreen => {
                        app.toggle_fullscreen(&window);
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

            Event::ImageMeta { meta, id } => {
                log::debug!("Got meta for {:#?}: {:#?}", id, meta);
                app.images_meta.insert(id, meta);
                app.update_info(&main);
            }

            Event::LoadFailed { id, err } => {
                // FIXME: when rapidly going through images this seems to break
                app.index = app.index.map(|index| index - 1);
                if app.is_currently_loading_image(id) {
                    match app.state {
                        State::DisplayImage { .. } | State::NoImages => {
                            panic!("how did you even get here?")
                        }
                        State::LoadingImage {
                            last_transition, ..
                        } => match last_transition {
                            ImageTransition::Next | ImageTransition::Start => {
                                app.try_load(&ctx, &main, ImageTransition::Next);
                            }
                            ImageTransition::Prev | ImageTransition::End => {
                                app.try_load(&ctx, &main, ImageTransition::Prev);
                            }
                        },
                    };
                }
                if let (Some(path), _) = (app.images.remove(id), app.images_meta.remove(id)) {
                    log::error!("Failed loading image {}: {}", path, err);
                    // removed the last image
                    if app.images.head().is_none() {
                        app.state = State::NoImages;
                    }
                }
            }

            Event::ImageLoaded { id, img } => {
                if app.is_currently_loading_image(id) {
                    app.state = State::DisplayImage { img, scale: 100. };
                    app.scale_initial(&main);
                }
            }
            // FIXME: this doesn't work for some reason
            Event::WindowFullScreenToggle => {
                app.scale_initial(&main);
            }
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
    math::scale_to_fit(scaled, ratio).and_then(|(r, _)| r.try_cast())
}

struct App {
    cursor: Option<DefaultKey>,
    is_fullscreen: bool,
    index: Option<usize>,
    images: LinkedSlotlist<String>,
    images_meta: SecondaryMap<DefaultKey, ImageMeta>,
    filenames: SecondaryMap<DefaultKey, String>,
    state: State,
    config: config::Config,
    format_map: FormatMap,
}

#[derive(Debug)]
pub struct ImageMeta {
    dimensions: Vector2D<i32, Pixels>,
    filesize: i64,
}

#[derive(Copy, Clone, Debug)]
enum ImageTransition {
    Next,
    Prev,
    Start,
    End,
}

impl App {
    fn try_transition(&self, transition: ImageTransition) -> Option<DefaultKey> {
        match (transition, self.cursor) {
            (ImageTransition::Prev, Some(cur)) => self.images.prev(cur),
            (ImageTransition::Next, Some(cur)) => self.images.next(cur),
            (ImageTransition::Start, _) => self.images.head(),
            (ImageTransition::End, _) => self.images.tail(),
            _ => None,
        }
    }

    fn change_index(&mut self, transition: ImageTransition) -> Option<DefaultKey> {
        let ret = self.try_transition(transition);
        self.index = match (transition, ret) {
            (ImageTransition::Prev, Some(_)) => self.index.map(|idx| idx - 1),
            (ImageTransition::Next, Some(_)) => self.index.map(|idx| idx + 1),
            (ImageTransition::Start, Some(_)) => Some(0),
            (ImageTransition::End, Some(_)) => Some(self.images.len()),
            _ => self.index,
        };
        ret
    }

    fn update_info(&mut self, main: &widgets::Main) {
        if let Some(idx) = self.index {
            self.format_map.insert("index", (idx + 1) as f64);
        }

        self.format_map.insert("nimages", self.images.len() as f64);
        if let Some(cur) = self.cursor {
            if let Some(meta) = self.images_meta.get(cur) {
                self.format_map.insert("width", meta.dimensions.x as f64);
                self.format_map.insert("height", meta.dimensions.x as f64);
                self.format_map.insert("filesize", meta.filesize as f64);
            }

            if let Some(filename) = self.filenames.get(cur) {
                self.format_map.insert("filename", filename.clone());
            }

            if let Some(path) = self.images.get(cur) {
                self.format_map.insert("fullpath", path.to_owned());
            }
        }

        match self.config.status_format.fmt(&self.format_map) {
            Ok(fmt) => {
                main.set_info(&format!("{}", fmt));
            }
            Err(e) => {
                log::error!("Can't format: {}", e);
            }
        }
    }

    fn is_currently_loading_image(&self, id: DefaultKey) -> bool {
        Some(id) == self.cursor
    }

    fn try_load(
        &mut self,
        ctx: &context::AppCtx,
        main: &widgets::Main,
        transition: ImageTransition,
    ) {
        if let Some(cur) = self.change_index(transition) {
            let path = self.images.get(cur).unwrap().to_owned();
            if let None = self.filenames.get(cur) {
                let filename = Path::new(&path).file_name().unwrap().to_str().unwrap();
                self.filenames.insert(cur, filename.to_owned());
            }
            self.update_info(&main);
            self.state = match &self.state {
                State::NoImages => State::NoImages,
                State::LoadingImage { abort_handle, .. } => {
                    abort_handle.abort();
                    State::LoadingImage {
                        abort_handle: ctx.load_image(cur, path),
                        last_transition: transition,
                    }
                }
                State::DisplayImage { .. } => State::LoadingImage {
                    abort_handle: ctx.load_image(cur, path),
                    last_transition: transition,
                },
            };
            self.cursor = Some(cur);
            main.set_image(None);
        }
    }

    fn zoom_in(&mut self, main: &widgets::Main) {
        if let State::DisplayImage { img, scale } = &self.state {
            self.state = {
                let next = math::step_next(*scale, self.config.zoom_step_size.0);
                let img_px: euclid::Vector2D<_, Pixels> = vec2(img.get_width(), img.get_height());
                let scaled = (img_px.to_f64() * next).cast();
                let resized = img
                    .scale_simple(scaled.x, scaled.y, self.config.interpolation_algorithm)
                    .unwrap();
                main.set_image(Some(&resized));
                State::DisplayImage {
                    img: img.clone(),
                    scale: next,
                }
            };
        }
    }

    fn zoom_out(&mut self, main: &widgets::Main) {
        if let State::DisplayImage { img, scale } = &self.state {
            self.state = {
                let step_size = self.config.zoom_step_size.0;
                let next = f64::max(math::step_prev(*scale, step_size), step_size);
                let img_px: euclid::Vector2D<_, Pixels> = vec2(img.get_width(), img.get_height());
                let scaled = (img_px.to_f64() * next).cast();
                let resized = img
                    .scale_simple(scaled.x, scaled.y, self.config.interpolation_algorithm)
                    .unwrap();
                main.set_image(Some(&resized));
                State::DisplayImage {
                    img: img.clone(),
                    scale: next,
                }
            };
        }
    }

    fn scale<F>(&mut self, main: &widgets::Main, f: F)
    where
        F: Fn(Vector2D<i32, Pixels>, Vector2D<i32, Pixels>) -> Option<(Vector2D<i32, Pixels>, f64)>,
    {
        if let State::DisplayImage { img, .. } = &self.state {
            let alloc = main.image_allocation();
            let img_px = vec2(img.get_width(), img.get_height());

            let (scaled, scale) = f(alloc, img_px).unwrap();

            let resized = img
                .scale_simple(scaled.x, scaled.y, self.config.interpolation_algorithm)
                .unwrap();

            main.set_image(Some(&resized));
            self.state = State::DisplayImage {
                img: img.clone(),
                scale,
            };
        }
    }

    fn scale_initial(&mut self, main: &widgets::Main) {
        let scaling = self.config.mode.initial_scaling;
        self.scale(main, |a, b| math::scale(a, b, scaling))
    }

    fn scale_to_fit(&mut self, main: &widgets::Main) {
        self.scale(main, math::scale_to_fit)
    }

    fn toggle_fullscreen(&mut self, window: &gtk::Window) {
        let is_fullscreen = self.is_fullscreen;
        if std::mem::replace(&mut self.is_fullscreen, !is_fullscreen) {
            window.unfullscreen();
        } else {
            window.fullscreen();
        }
    }
}

#[derive(Debug)]
enum State {
    NoImages,
    LoadingImage {
        abort_handle: future::AbortHandle,
        last_transition: ImageTransition,
    },
    DisplayImage {
        img: Pixbuf,
        scale: f64,
    },
}

fn default_format_map() -> FormatMap {
    let mut ret = FormatMap::new();
    ret.insert("index", -1.0);
    ret.insert("nimages", -1.0);
    ret.insert("width", -1.0);
    ret.insert("height", -1.0);
    ret.insert("filesize", -1.0);
    ret.insert("filename", "".to_string());
    ret
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

    #[snafu(display("Bad status_format in config: {}", source))]
    Format { source: formatter::Error },
}

fn main() {
    env_logger::init();
    if let Err(e) = run() {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
