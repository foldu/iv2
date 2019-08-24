use std::f64;

use euclid::Vector2D;
use num_traits::NumCast;

#[derive(Copy, Clone, Debug)]
pub struct Pixels;

pub fn scale_with_aspect_ratio<T>(
    bounding: Vector2D<T, Pixels>,
    other: Vector2D<T, Pixels>,
) -> Option<(Vector2D<T, Pixels>, f64)>
where
    T: NumCast + Copy,
{
    let (a, b) = (bounding.to_f64(), other.to_f64());
    let s = f64::min(a.x / b.x, a.y / b.y);
    (b * s).floor().try_cast::<T>().map(|r| (r, s))
}

fn step_with<F>(f: F) -> impl Fn(f64, f64) -> f64 + 'static
where
    F: Fn(f64, f64) -> f64 + 'static,
{
    move |orig, step_size| f64::floor(f(orig, step_size) / step_size) * step_size
}

pub fn step_prev(orig: f64, step_size: f64) -> f64 {
    step_with(|a, b| a - b)(orig, step_size)
}

pub fn step_next(orig: f64, step_size: f64) -> f64 {
    step_with(|a, b| a + b)(orig, step_size)
}
