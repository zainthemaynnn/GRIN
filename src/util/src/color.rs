use bevy::prelude::*;
use bevy_tweening::Lerp;
use rand::Rng;

pub trait ColorExt {
    fn lerp(&self, other: &Color, t: f32) -> Color;
}

impl ColorExt for Color {
    fn lerp(&self, other: &Color, t: f32) -> Color {
        match (self, other) {
            (
                Color::Rgba {
                    red,
                    green,
                    blue,
                    alpha,
                },
                Color::Rgba {
                    red: r,
                    green: g,
                    blue: b,
                    alpha: a,
                },
            ) => Color::Rgba {
                red: red.lerp(r, &t),
                green: green.lerp(g, &t),
                blue: blue.lerp(b, &t),
                alpha: alpha.lerp(a, &t),
            },
            (
                Color::RgbaLinear {
                    red,
                    green,
                    blue,
                    alpha,
                },
                Color::RgbaLinear {
                    red: r,
                    green: g,
                    blue: b,
                    alpha: a,
                },
            ) => Color::RgbaLinear {
                red: red.lerp(r, &t),
                green: green.lerp(g, &t),
                blue: blue.lerp(b, &t),
                alpha: alpha.lerp(a, &t),
            },
            _ => unreachable!(),
        }
    }
}

pub fn rand_spectrum(rng: &mut impl Rng) -> Color {
    let t = rng.gen::<f32>() * 3.0;
    if t <= 1.0 {
        Color::RED.lerp(&Color::GREEN, t)
    } else if t <= 2.0 {
        Color::GREEN.lerp(&Color::BLUE, t - 1.0)
    } else {
        Color::BLUE.lerp(&Color::RED, t - 2.0)
    }
}
