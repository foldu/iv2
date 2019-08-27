use linked_slotlist::DefaultKey;
use serde::Deserialize;
use snafu::Snafu;

use crate::context::LoadError;

#[derive(Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum UserEvent {
    Quit,
    Next,
    Previous,
    ScaleToFitCurrent,
    OriginalSize,
    ResizeToFitImage,
    ZoomOut,
    ZoomIn,
    ScrollDown,
    ScrollUp,
    ScrollLeft,
    ScrollRight,
    ScrollVStart,
    ScrollVEnd,
    ScrollHStart,
    ScrollHEnd,
    ToggleStatus,
    JumpToStart,
    JumpToEnd,
    RotateClockwise,
    RotateCounterClockwise,
    RotateUpsideDown,
}

#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy)]
pub struct KeyPress(pub u32);

pub enum Event {
    User(UserEvent),
    ImageLoaded {
        img: gdk_pixbuf::Pixbuf,
        id: DefaultKey,
    },
    ImageMeta {
        meta: crate::ImageMeta,
        id: DefaultKey,
    },
    LoadFailed {
        id: DefaultKey,
        err: LoadError,
    },
    Quit,
}
