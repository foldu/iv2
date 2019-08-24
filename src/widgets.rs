use std::convert::TryFrom;

use cascade::cascade;
use euclid::{vec2, Vector2D};
use gtk::prelude::*;
use shrinkwraprs::Shrinkwrap;

use crate::{events::UserEvent, math::Pixels};

#[derive(Shrinkwrap)]
pub struct Main {
    #[shrinkwrap(main_field)]
    vbox: gtk::Box,
    bottom_bar: BottomBar,
    pub image: ScrollableImage,
}

#[derive(Shrinkwrap)]
pub struct BottomBar {
    #[shrinkwrap(main_field)]
    hbox: gtk::Box,
    info: gtk::Label,
    err: gtk::Label,
}

#[derive(Shrinkwrap)]
pub struct ScrollableImage {
    #[shrinkwrap(main_field)]
    scroll: gtk::ScrolledWindow,
    pub image: gtk::Image,
}

impl BottomBar {
    pub fn new() -> Self {
        let info = cascade! {
            gtk::Label::new(None);
        };

        let err = cascade! {
            gtk::Label::new(None);
        };

        let hbox = cascade! {
            gtk::Box::new(gtk::Orientation::Horizontal, 10);
            ..add(&err);
            ..add(&info);
        };

        Self { hbox, info, err }
    }
}

impl ScrollableImage {
    pub fn new() -> Self {
        let image = cascade! {
            gtk::Image::new();
        };

        let scroll = cascade! {
            gtk::ScrolledWindow::new::<gtk::Adjustment, gtk::Adjustment>(None, None);
            ..add(&image);
        };

        Self { scroll, image }
    }
}

#[derive(Clone, Copy)]
pub enum Scroll {
    H(ScrollH),
    V(ScrollV),
}

#[derive(Clone, Copy)]
pub enum ScrollH {
    Left,
    Right,
    Start,
    End,
}

#[derive(Clone, Copy)]
pub enum ScrollV {
    Down,
    Up,
    Start,
    End,
}

impl TryFrom<UserEvent> for Scroll {
    type Error = ();
    fn try_from(evt: UserEvent) -> Result<Self, Self::Error> {
        Ok(match evt {
            UserEvent::ScrollDown => Scroll::V(ScrollV::Down),
            UserEvent::ScrollUp => Scroll::V(ScrollV::Up),
            UserEvent::ScrollVStart => Scroll::V(ScrollV::Start),
            UserEvent::ScrollVEnd => Scroll::V(ScrollV::End),
            UserEvent::ScrollLeft => Scroll::H(ScrollH::Left),
            UserEvent::ScrollRight => Scroll::H(ScrollH::Right),
            UserEvent::ScrollHStart => Scroll::H(ScrollH::Start),
            UserEvent::ScrollHEnd => Scroll::H(ScrollH::End),
            _ => return Err(()),
        })
    }
}

impl Main {
    pub fn new() -> Self {
        let bottom_bar = BottomBar::new();

        let image = ScrollableImage::new();

        let vbox = cascade! {
            gtk::Box::new(gtk::Orientation::Vertical, 0);
            ..pack_start(image.as_ref(), true, true, 0);
            ..add(bottom_bar.as_ref());
        };

        Self {
            image,
            bottom_bar,
            vbox,
        }
    }

    pub fn set_image(&self, img: Option<&gdk_pixbuf::Pixbuf>) {
        self.image.image.set_from_pixbuf(img);
    }

    pub fn image_allocation(&self) -> Vector2D<i32, Pixels> {
        let alloc = self.image.get_allocation();
        vec2(alloc.width, alloc.height)
    }

    pub fn scroll(&self, scroll: Scroll) {
        use Scroll::*;
        match scroll {
            H(scroll) => {
                if let Some(adjust) = self.image.get_hadjustment() {
                    use ScrollH::*;
                    match scroll {
                        Left => adjust.set_value(adjust.get_value() - adjust.get_step_increment()),
                        Right => adjust.set_value(adjust.get_value() + adjust.get_step_increment()),
                        Start => adjust.set_value(adjust.get_lower()),
                        End => adjust.set_value(adjust.get_upper()),
                    }
                }
            }
            V(scroll) => {
                if let Some(adjust) = self.image.get_vadjustment() {
                    use ScrollV::*;
                    match scroll {
                        Up => adjust.set_value(adjust.get_value() - adjust.get_step_increment()),
                        Down => adjust.set_value(adjust.get_value() + adjust.get_step_increment()),
                        Start => adjust.set_value(adjust.get_lower()),
                        End => adjust.set_value(adjust.get_upper()),
                    }
                }
            }
        }
    }
}
