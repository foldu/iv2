use once_cell::sync::Lazy;
use regex::Regex;
use serde::{
    de::{self, Deserializer, Visitor},
    Deserialize,
};

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

pub fn percent<'de, D>(de: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    static REGEX: Lazy<Regex> = Lazy::new(|| Regex::new("^(0|(:?[1-9][0-9]*))%$").unwrap());
}

pub fn ratio<'de, D>(de: D) -> Result<(u8, u8), D::Error>
where
    D: Deserializer<'de>,
{
    static REGEX: Lazy<Regex> = Lazy::new(|| Regex::new("^([1-9][0-9]*)x([1-9][0-9]*)$").unwrap());
}
