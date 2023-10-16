//! Just some maps for `[0.0, 1.0]` ranges.

#![allow(dead_code)]

use std::f32::consts::FRAC_PI_2;

#[inline]
pub fn f32_distribution<'a>(
    n: usize,
    distr: &'a impl Fn(f32) -> f32,
) -> impl Iterator<Item = f32> + 'a {
    (0..n)
        .into_iter()
        .map(move |i| distr((i as f32) / ((n - 1) as f32)))
}

#[inline]
pub fn f32_map<'a>(
    n: usize,
    distr0: &'a impl Fn(f32) -> f32,
    distr1: &'a impl Fn(f32) -> f32,
) -> impl Iterator<Item = (f32, f32)> + 'a {
    f32_distribution(n, distr0).zip(f32_distribution(n, distr1))
}

#[inline]
pub fn linear(x: f32) -> f32 {
    x
}

#[inline]
pub fn quad(x: f32) -> f32 {
    x.powi(2)
}

#[inline]
pub fn cubic(x: f32) -> f32 {
    x.powi(3)
}

#[inline]
pub fn quart(x: f32) -> f32 {
    x.powi(4)
}

#[inline]
pub fn quint(x: f32) -> f32 {
    x.powi(5)
}

#[inline]
pub fn sine(x: f32) -> f32 {
    ((x + 3.0) * FRAC_PI_2).sin() + 1.0
}

#[inline]
pub fn rev(distr: impl Fn(f32) -> f32) -> impl Fn(f32) -> f32 {
    move |x| distr(1.0 - x)
}
