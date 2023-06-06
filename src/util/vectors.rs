//! Common `Vec3` calculations.
use std::f32::consts::TAU;

use approx::assert_relative_eq;
use bevy::prelude::{Quat, Vec3};
use itertools::Itertools;

use super::distr::{self, closed_f32_distribution, open_f32_distribution};

pub trait Vec3Ext {
    /// Creates an arbitrary perpendicular vector of equal length.
    fn perp(&self) -> Self;
}

impl Vec3Ext for Vec3 {
    fn perp(&self) -> Self {
        if self.z.abs() < self.x.abs() {
            Vec3::new(self.y, -self.x, 0.0).normalize() * self.length()
        } else {
            Vec3::new(0.0, -self.z, self.y).normalize() * self.length()
        }
    }
}

/// Sets `Vec3.y` to `0.0`.
#[inline]
pub fn normalize_y(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z)
}

/// Distributes `n` points over `[p0, p1)`.
pub fn segment<'a>(
    p0: Vec3,
    p1: Vec3,
    n: u32,
    distr: &'a impl Fn(f32) -> f32,
) -> impl Iterator<Item = Vec3> + 'a {
    open_f32_distribution(n, distr).map(move |x| p0.clone().lerp(p1, x))
}

/// Distributes `n` points over `[p0, p1]`.
pub fn filled_segment<'a>(
    p0: Vec3,
    p1: Vec3,
    n: u32,
    distr: &'a impl Fn(f32) -> f32,
) -> impl Iterator<Item = Vec3> + 'a {
    closed_f32_distribution(n, distr).map(move |x| p0.clone().lerp(p1, x))
}

/// Distributes a point `p0` around `axis` `n` times over `[0.0, angle)` radians.
#[inline]
pub fn arc<'a>(
    p0: Vec3,
    axis: Vec3,
    n: u32,
    angle: f32,
    distr: &'a impl Fn(f32) -> f32,
) -> impl Iterator<Item = Vec3> + 'a {
    open_f32_distribution(n, distr).map(move |x| Quat::from_axis_angle(axis, angle * x) * p0)
}

/// Distributes a point `p0` around `axis` `n` times over `[0.0, angle]` radians.
#[inline]
pub fn filled_arc<'a>(
    p0: Vec3,
    axis: Vec3,
    n: u32,
    angle: f32,
    distr: &'a impl Fn(f32) -> f32,
) -> impl Iterator<Item = Vec3> + 'a {
    closed_f32_distribution(n, distr).map(move |x| Quat::from_axis_angle(axis, angle * x) * p0)
}

/// Rotates a point `p0` around `axis` `n` times within tau radians. Includes `p1`.
#[inline]
pub fn circle<'a>(
    p0: Vec3,
    axis: Vec3,
    n: u32,
    distr: &'a impl Fn(f32) -> f32,
) -> impl Iterator<Item = Vec3> + 'a {
    arc(p0, axis, n, TAU, distr)
}

/// Distributes `n` points between adjacent pairs from `vertices`.
pub fn link_vertices<'a, I>(
    vertices: impl IntoIterator<IntoIter = I>,
    n: u32,
    distr: &'a impl Fn(f32) -> f32,
) -> impl Iterator<Item = Vec3> + 'a
where
    I: Iterator<Item = Vec3> + Clone + ExactSizeIterator + 'a,
{
    vertices
        .into_iter()
        // look at that! itertools is like magic!
        .circular_tuple_windows()
        .flat_map(move |(p0, p1)| segment(p0, p1, n, distr))
}

/// Creates an equilateral `n`-gon with `segsize` points distributed per side.
pub fn polygon_eq<'a>(
    p0: Vec3,
    axis: Vec3,
    n: u32,
    segsize: u32,
    distr: &'a impl Fn(f32) -> f32,
) -> impl Iterator<Item = Vec3> + 'a {
    // the iterator needs to be collected as it's not copiable
    // whatever, I doubt `vertices` is getting that big
    let vertices = circle(p0, axis, n, &distr::linear).collect::<Vec<_>>();
    link_vertices(vertices, segsize, distr)
}

#[cfg(test)]
mod tests {
    use std::f32::consts::PI;

    use super::*;

    #[test]
    fn segment_test() {
        assert_eq!(
            segment(Vec3::X, Vec3::NEG_X, 4, &distr::linear).collect::<Vec<_>>(),
            vec![
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.5, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(-0.5, 0.0, 0.0),
            ],
        );
        assert_eq!(
            filled_segment(Vec3::X, Vec3::NEG_X, 5, &distr::linear).collect::<Vec<_>>(),
            vec![
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.5, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(-0.5, 0.0, 0.0),
                Vec3::new(-1.0, 0.0, 0.0),
            ],
        );
    }

    // floats... sigh... why can't you be like the segment test?
    #[test]
    fn arc_test() {
        let a =
            arc(Vec3::X, Vec3::Y, 3, 270.0_f32.to_radians(), &distr::linear).collect::<Vec<_>>();
        assert_relative_eq!(a[0].x, 1.0);
        assert_relative_eq!(a[1].z, -1.0);
        assert_relative_eq!(a[2].x, -1.0);

        let a = filled_arc(Vec3::X, Vec3::Y, 3, 270.0_f32.to_radians(), &distr::linear)
            .collect::<Vec<_>>();
        assert_relative_eq!(a[2].z, 1.0);
    }

    #[test]
    fn circle_test() {
        let c = circle(Vec3::X, Vec3::Y, 4, &distr::linear).collect::<Vec<_>>();
        assert_relative_eq!(c[0].x, 1.0);
        assert_relative_eq!(c[1].z, -1.0);
        assert_relative_eq!(c[2].x, -1.0);
        assert_relative_eq!(c[3].z, 1.0);
    }

    #[test]
    fn poly_test() {
        let p = polygon_eq(Vec3::X, Vec3::Y, 2, 2, &distr::linear).collect::<Vec<_>>();
        assert_relative_eq!(p[0].x, 1.0);
        assert_relative_eq!(p[1].x, 0.0);
        assert_relative_eq!(p[2].x, -1.0);
        assert_relative_eq!(p[3].x, 0.0);
    }
}
