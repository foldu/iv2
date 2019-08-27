use euclid::vec2;
use futures::{future, prelude::*};
use gdk_pixbuf::Pixbuf;
use gio::prelude::*;
use slotmap::DefaultKey;
use snafu::{ResultExt, Snafu};

use crate::events::Event;

pub struct AppCtx {
    g_ctx: glib::MainContext,
    event_tx: glib::Sender<Event>,
}

impl AppCtx {
    pub fn new(event_tx: glib::Sender<Event>) -> Self {
        Self {
            g_ctx: glib::MainContext::default(),
            event_tx,
        }
    }
}

async fn load_image(stream: gio::FileInputStream) -> Result<Pixbuf, glib::Error> {
    Pixbuf::new_from_stream_async_future(&stream).await
}

impl AppCtx {
    pub fn load_image(&self, id: DefaultKey, path: String) -> future::AbortHandle {
        let g_path = gio::File::new_for_path(&path);

        let tx = self.event_tx.clone();

        let fut = async move {
            let open = async move { g_path.read_async_future(glib::PRIORITY_LOW).await };
            let pixbuf_info = async move { Pixbuf::get_file_info_async_future(path).await };
            let (fh, pixbuf_info) = futures::join!(open, pixbuf_info);

            let to_send = match (fh.context(FromGlib), pixbuf_info.context(FromGlib)) {
                (Ok(fh), Ok(Some(info))) => {
                    // FIXME: just hoping the file doesn't vanish is bad
                    // "" means query all standard file attributes
                    // that's an "interesting" way to implement flags
                    let file_meta = fh
                        .query_info_async_future("", glib::PRIORITY_LOW)
                        .await
                        .unwrap();
                    let _ = tx.send(Event::ImageMeta {
                        id,
                        meta: crate::ImageMeta {
                            dimensions: vec2(info.1, info.2),
                            filesize: file_meta.get_size(),
                        },
                    });

                    match load_image(fh).await.context(FromGlib) {
                        Ok(img) => Event::ImageLoaded { img, id },
                        Err(err) => Event::LoadFailed { err, id },
                    }
                }
                (Err(err), _) | (_, Err(err)) => Event::LoadFailed { err, id },
                (_, Ok(None)) => Event::LoadFailed {
                    err: LoadError::UnsupportedFormat,
                    id,
                },
            };
            let _ = tx.send(to_send);
        };

        let (fut, handle) = future::abortable(fut);
        self.g_ctx.spawn_local(fut.map(|_| ()));
        handle
    }
}

#[derive(Snafu, Debug)]
pub enum LoadError {
    #[snafu(display("Error from glib: {}", source))]
    FromGlib { source: glib::Error },

    #[snafu(display("Image format not supported or not an image"))]
    UnsupportedFormat,
}
