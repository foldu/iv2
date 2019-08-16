use cfgen::prelude::*;
use hashbrown::HashMap;
use serde::{
    de::{self, Deserializer, Visitor},
    Deserialize,
};

use crate::events::{KeyPress, UserEvent};

const DEFAULT: &str = "
[keymap]
plus = 'zoom_in'
l = 'scroll_right'
f = 'rotate_upside_down'
n = 'next'
p = 'previous'
o = 'original_size'
0 = 'scroll_h_start'
m = 'toggle_status'
j = 'scroll_down'
b = 'jump_to_start'
k = 'scroll_up'
w = 'resize_to_fit_screen'
minus = 'zoom_out'
e = 'jump_to_end'
r = 'rotate_counter_clockwise'
g = 'scroll_v_end'
equal = 'scale_to_fit_current'
q = 'quit'
dollar = 'scroll_h_end'
h = 'scroll_left'
";

#[derive(Deserialize, Debug, Clone, Cfgen)]
#[cfgen(default = "DEFAULT")]
pub struct Config {
    // This is read from an user provided config so I'm pretty sure
    // he won't hash ddos himself
    pub keymap: HashMap<KeyPress, UserEvent>,
}

struct KeyPressVisitor;

impl<'de> Visitor<'de> for KeyPressVisitor {
    type Value = KeyPress;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a key like `Ctrl-a`")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<KeyPress, E> {
        gtk::init();
        let (keycode, _mask) = gtk::accelerator_parse(&value);
        if keycode == 0 {
            Err(E::custom(format!("Can't parse as key: {}", value)))
        } else {
            Ok(KeyPress(keycode))
        }
    }
}

impl<'de> Deserialize<'de> for KeyPress {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(KeyPressVisitor)
    }
}
