use cascade::cascade;
use gtk::prelude::*;
use shrinkwraprs::Shrinkwrap;

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
}
