mod deserializers;

use cfgen::prelude::*;
use hashbrown::HashMap;
use serde::Deserialize;

use crate::{
    events::{KeyPress, UserEvent},
    percent::Percent,
    ratio::Ratio,
};

const DEFAULT: &str = include_str!("../default_config.toml");

#[derive(Deserialize, Debug, Clone, Cfgen)]
#[serde(rename_all = "kebab-case")]
#[cfgen(default = "DEFAULT")]
pub struct UserConfig {
    pub status_format: String,
    pub show_scrollbars: bool,
    #[serde(with = "deserializers::InterpTypeDef")]
    pub interpolation_algorithm: gdk_pixbuf::InterpType,

    pub mode: ModeEntry,

    // This is read from an user provided config so I'm pretty sure
    // he won't hash ddos himself
    pub keymap: HashMap<KeyPress, UserEvent>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub status_format: String,
    pub show_scrollbars: bool,
    pub interpolation_algorithm: gdk_pixbuf::InterpType,

    pub mode: Mode,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModeEntry {
    pub image: Mode,
    pub archive: Mode,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct Mode {
    pub scale_to_fit_window: Option<ImageScaling>,
    pub hide_status: bool,
    pub geometry: Geometry,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum ImageScaling {
    Width,
    Height,
}

#[derive(Deserialize, Debug, Clone, Cfgen)]
#[serde(rename_all = "kebab-case")]
pub struct Geometry {
    pub scale: Percent,
    pub aspect_ratio: Ratio,
}

pub enum ViewerMode {
    Image,
    Archive,
}

impl UserConfig {
    pub fn split_for_app_use(self, mode: ViewerMode) -> (HashMap<KeyPress, UserEvent>, Config) {
        (
            self.keymap,
            Config {
                status_format: self.status_format,
                show_scrollbars: self.show_scrollbars,
                interpolation_algorithm: self.interpolation_algorithm,
                mode: match mode {
                    ViewerMode::Image => self.mode.image,
                    ViewerMode::Archive => self.mode.archive,
                },
            },
        )
    }
}
