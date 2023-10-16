use bevy::prelude::*;
use bevy_tweening::Lerp;
use rand::Rng;

pub trait ColorExt {
    fn lerp(&self, other: &Color, t: f32) -> Color;
}

impl ColorExt for Color {
    fn lerp(&self, other: &Color, t: f32) -> Color {
        match self {
            Self::Rgba {
                red,
                green,
                blue,
                alpha,
            } => Self::Rgba {
                red: red.lerp(&other.r(), &t),
                green: green.lerp(&other.g(), &t),
                blue: blue.lerp(&other.b(), &t),
                alpha: alpha.lerp(&other.a(), &t),
            },
            _ => unimplemented!(),
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
