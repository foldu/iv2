mod deserializers;

use cfgen::prelude::*;
use hashbrown::HashMap;
use serde::Deserialize;

use crate::events::{KeyPress, UserEvent};
use deserializers::{percent, ratio};

const DEFAULT: &str = include_str!("../default_config.toml");

#[derive(Deserialize, Debug, Clone, Cfgen)]
#[serde(rename_all = "kebab-case")]
#[cfgen(default = "DEFAULT")]
pub struct Config {
    pub status_format: String,
    pub show_scrollbars: bool,
    // FIXME
    pub interpolation_algorithm: String,

    pub mode: ModeEntry,

    // This is read from an user provided config so I'm pretty sure
    // he won't hash ddos himself
    pub keymap: HashMap<KeyPress, UserEvent>,
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
    // FIXME:
    #[serde(deserialize_with = "percent")]
    pub scale: u8,
    // FIXME:
    #[serde(deserialize_with = "ratio")]
    pub aspect_ratio: (u8, u8),
}
