//! Just some maps for `[0.0, 1.0]` ranges.

#![allow(dead_code)]

use std::f32::consts::PI;

#[inline]
pub fn open_f32_distribution<'a>(
    n: u32,
    distr: &'a impl Fn(f32) -> f32,
) -> impl Iterator<Item = f32> + 'a {
    (0..n)
        .into_iter()
        .map(move |i| distr((i as f32) / (n as f32)))
}

#[inline]
pub fn closed_f32_distribution<'a>(
    n: u32,
    distr: &'a impl Fn(f32) -> f32,
) -> impl Iterator<Item = f32> + 'a {
    (0..n)
        .into_iter()
        .map(move |i| distr((i as f32) / ((n - 1) as f32)))
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
    ((3.0 + x) * PI / 2.0).sin() + 1.0
}

#[inline]
pub fn rev(distr: impl Fn(f32) -> f32) -> impl Fn(f32) -> f32 {
    move |x| distr(1.0 - x)
}
