#![feature(async_await)]

mod config;
mod events;
mod state;
mod widgets;

use cascade::cascade;
use cfgen::prelude::CfgenDefault;
use gio::prelude::*;
use glib::prelude::*;
use gtk::prelude::*;
use snafu::{ResultExt, Snafu};

use crate::events::{Event, KeyPress};

fn gtk_run() -> Result<(), Error> {
    let (_, config) = config::Config::load_or_write_default().context(ReadConfig)?;

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
        if let Some(user_event) = config.keymap.get(&KeyPress(key_evt.get_keyval())) {
            let _ = tx.send(Event::User(*user_event));
            Inhibit(true)
        } else {
            Inhibit(false)
        }
    });

    let tx = main_tx.clone();
    let g_ctx = glib::MainContext::default();
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
                    _ => {}
                }
                let file = gio::File::new_for_path("/home/barnabas/meddl_cdu.jpg");
                let tx = tx.clone();
                g_ctx.spawn_local(async move {
                    let fh = file
                        .read_async_future(glib::source::PRIORITY_DEFAULT)
                        .await
                        .unwrap();
                    let img = gdk_pixbuf::Pixbuf::new_from_stream_async_future(&fh).await;
                    if let Ok(img) = img {
                        let _ = tx.send(Event::ImageLoaded(img));
                    }
                });
            }
            Event::ImageLoaded(img) => {
                main.image.image.set_from_pixbuf(Some(&img));
            }
        }
        Continue(true)
    });

    window.show_all();

    gtk::main();

    Ok(())
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
