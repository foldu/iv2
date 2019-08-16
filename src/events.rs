use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum UserEvent {
    Quit,
    Next,
    Previous,
    ScaleToFitCurrent,
    OriginalSize,
    ResizeToFitImage,
    ResizeToFitScreen,
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
    Quit,
}
