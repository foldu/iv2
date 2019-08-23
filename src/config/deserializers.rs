use std::fmt;

use gdk_pixbuf::InterpType;
use serde::{
    de::{self, Deserializer, Visitor},
    Deserialize,
};

use crate::{events::KeyPress, percent::Percent, ratio::Ratio};

struct KeyPressVisitor;

impl<'de> Visitor<'de> for KeyPressVisitor {
    type Value = KeyPress;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a key combination like `<Ctrl>a`")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<KeyPress, E> {
        let (keycode, mask) = gtk::accelerator_parse(&value);
        if keycode == 0 {
            Err(E::custom(format!("Can't parse as key: {}", value)))
        } else {
            Ok(KeyPress(keycode, mask))
        }
    }
}

impl<'de> Deserialize<'de> for KeyPress {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(KeyPressVisitor)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", remote = "InterpType")]
#[allow(dead_code)]
pub enum InterpTypeDef {
    Nearest,
    Tiles,
    Bilinear,
    Hyper,
    __Unknown(i32),
}

impl<'de> Deserialize<'de> for Percent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct PercentVisitor;
        impl<'de> de::Visitor<'de> for PercentVisitor {
            type Value = Percent;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a percentage")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                value.parse().map_err(|e| E::custom(format!("{}", e)))
            }
        }

        deserializer.deserialize_str(PercentVisitor)
    }
}

impl<'de> Deserialize<'de> for Ratio {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RatioVisitor;
        impl<'de> de::Visitor<'de> for RatioVisitor {
            type Value = Ratio;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a ratio")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                value
                    .parse()
                    .map_err(|e: &'static str| E::custom(e.to_string()))
            }
        }

        deserializer.deserialize_str(RatioVisitor)
    }
}
