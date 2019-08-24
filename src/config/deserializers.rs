use std::fmt;

use euclid::vec2;
use gdk_pixbuf::InterpType;
use serde::{
    de::{self, Deserializer, Visitor},
    Deserialize,
};

use crate::{
    config::{Percent, Ratio},
    events::KeyPress,
};

struct KeyPressVisitor;

impl<'de> Visitor<'de> for KeyPressVisitor {
    type Value = KeyPress;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a key combination like `<Ctrl>a`")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<KeyPress, E> {
        let (keycode, mask) = gtk::accelerator_parse(&value);
        log::debug!("Deserializing key `{}`: {} {:?}", value, keycode, mask);
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
                if value.ends_with('%') {
                    let to_parse = &value[..value.len() - 1];
                    let ret = to_parse
                        .parse::<u8>()
                        .map_err(|e| E::custom(format!("{}", e)))?;
                    Ok(Percent(ret as f64 / 100.))
                } else {
                    Err(E::custom("Percent value must end in `%`"))
                }
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
                formatter.write_str("a ratio like 16x9")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                let mut split = value.splitn(2, 'x');
                let mut next = move || split.next().and_then(|split| split.parse::<u32>().ok());
                (|| {
                    let (a, b) = (next()?, next()?);
                    Some(Ratio(vec2(a as f64, b as f64)))
                })()
                .ok_or_else(|| E::custom("Can't parse ratio"))
            }
        }

        deserializer.deserialize_str(RatioVisitor)
    }
}
