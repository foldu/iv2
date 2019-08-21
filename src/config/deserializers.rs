use once_cell::sync::Lazy;
use regex::Regex;
use serde::{
    de::{self, Deserializer, Visitor},
    Deserialize,
};

use crate::events::KeyPress;

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

fn parse_percent(s: &str) -> Option<u8> {
    static REGEX: Lazy<Regex> = Lazy::new(|| Regex::new("^(0|(:?[1-9][0-9]*))%$").unwrap());
    REGEX
        .captures(s)
        .and_then(|caps| caps.get(1))
        .and_then(|s| s.as_str().parse().ok())
}

pub fn percent<'de, D>(de: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(de)?;
    parse_percent(&s).ok_or_else(|| serde::de::Error::custom("Can't deserialize as percent value"))
}

fn parse_ratio(s: &str) -> Option<(u8, u8)> {
    static REGEX: Lazy<Regex> = Lazy::new(|| Regex::new("^([1-9][0-9]*)x([1-9][0-9]*)$").unwrap());
    REGEX
        .captures(s)
        .and_then(|caps| Some((caps.get(1)?, caps.get(2)?)))
        .and_then(|(w, h)| Some((w.as_str().parse().ok()?, h.as_str().parse().ok()?)))
}

pub fn ratio<'de, D>(de: D) -> Result<(u8, u8), D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(de)?;
    parse_ratio(&s).ok_or_else(|| serde::de::Error::custom("Can't deserialize as ratio value"))
}
