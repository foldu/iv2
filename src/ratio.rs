use std::{convert::TryFrom, ops};

use gtk::prelude::*;
use num::{FromPrimitive, ToPrimitive};
use once_cell::sync::Lazy;
use regex::Regex;

use crate::percent::Percent;

/// A ratio. Can be used for more than just correct aspect ratio transforms.
#[derive(Debug, Copy, Clone)]
pub struct Ratio(f64, f64);

impl std::str::FromStr for Ratio {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        static REGEX: Lazy<Regex> =
            Lazy::new(|| Regex::new("^([1-9][0-9]*)x([1-9][0-9]*)$").unwrap());
        REGEX
            .captures(s)
            .and_then(|caps| Some((caps.get(1)?, caps.get(2)?)))
            .and_then(|(w, h)| Some(Ratio(w.as_str().parse().ok()?, h.as_str().parse().ok()?)))
            .ok_or("Expecting f64xf64")
    }
}

/// Rescale operations that's guaranteed to work
impl ops::Mul<Percent> for Ratio {
    type Output = Self;
    fn mul(self, rhs: Percent) -> Self {
        let res = rescale(rhs, self.0, self.1).unwrap();
        Ratio(res.0, res.1)
    }
}

impl Ratio {
    pub fn new<T: ToPrimitive + Copy>(a: T, b: T) -> Option<Ratio> {
        Some(Ratio(a.to_f64()?, b.to_f64()?))
    }

    pub fn scale<T: FromPrimitive + ToPrimitive + Copy>(
        &self,
        a: T,
        b: T,
    ) -> Option<(Percent, (T, T))> {
        let (a_f, b_f) = (a.to_f64()?, b.to_f64()?);
        let ratio = f64::min(a_f / self.0, b_f / self.1);
        let ratio = Percent::try_from(ratio).ok()?;
        let scaled = rescale(ratio, T::from_f64(self.0)?, T::from_f64(self.1)?)?;
        Some((ratio, scaled))
    }
}

/// Rescales number with f64 factor
/// returns None if result can't be converted back to original data type
pub fn rescale<T: FromPrimitive + ToPrimitive + Copy>(fact: Percent, a: T, b: T) -> Option<(T, T)> {
    Some((
        T::from_f64((a.to_f64()? * fact).floor())?,
        T::from_f64((b.to_f64()? * fact).floor())?,
    ))
}

/// returns None if fact is 0, -inf, inf or NaN or if win doesn't have a screen
pub fn gtk_win_scale(win: &gdk::Window, ratio: Ratio, fact: Percent) -> Option<(i32, i32)> {
    let disp = gdk::Display::get_default()?;
    let dims = disp.get_monitor_at_window(win)?.get_geometry();
    let scale_dims = rescale(fact, dims.width, dims.height)?;
    let (_, scaled) = ratio.scale(scale_dims.0, scale_dims.1)?;
    Some(scaled)
}

#[test]
fn rat() {
    let (_, rat) = Ratio::new(16, 9).unwrap().scale(1920, 1200).unwrap();
    assert_eq!(rat, (1920, 1080));
}

#[test]
fn ratio_parse() {
    assert!("16x9".parse::<Ratio>().is_ok());
    assert!("16x9 ".parse::<Ratio>().is_err());
}
