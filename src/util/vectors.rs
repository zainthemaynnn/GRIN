//! Common `Vec3` calculations.

// this module is proof that I am no good at math...

#![allow(dead_code)]

use std::{cmp::Ordering, f32::consts::TAU};

use bevy::prelude::{Quat, Vec3};
use itertools::Itertools;

use super::distr::{self, closed_f32_distribution, open_f32_distribution};

pub trait Vec3Ext {
    /// Sets `Vec3.y` to `0.0`.
    fn xz_flat(&self) -> Self;

    /// Sets `Vec3.y` to `y`.
    fn with_y(&self, y: f32) -> Self;

    /// Returns the lexographic comparison of the two vectors.
    /// In baby words: comparison by priority of individual components `x`, `y`, `z`.
    fn lexographic_cmp(&self, other: &Vec3) -> std::cmp::Ordering;
}

impl Vec3Ext for Vec3 {
    #[inline]
    fn xz_flat(&self) -> Vec3 {
        Vec3::new(self.x, 0.0, self.z)
    }

    #[inline]
    fn with_y(&self, y: f32) -> Vec3 {
        Vec3::new(self.x, y, self.z)
    }

    fn lexographic_cmp(&self, other: &Vec3) -> std::cmp::Ordering {
        for i in 0..3 {
            match self[i].total_cmp(&other[i]) {
                std::cmp::Ordering::Equal => {
                    continue;
                }
                ord => {
                    return ord;
                }
            }
        }
        std::cmp::Ordering::Equal
    }
}

/// Distributes `n` points over `[p0, p1)`.
#[inline]
pub fn open_segment<'a>(
    p0: Vec3,
    p1: Vec3,
    n: u32,
    distr: &'a impl Fn(f32) -> f32,
) -> impl Iterator<Item = Vec3> + 'a {
    open_f32_distribution(n, distr).map(move |x| p0.clone().lerp(p1, x))
}

/// Distributes `n` points over `[p0, p1]`.
#[inline]
pub fn segment<'a>(
    p0: Vec3,
    p1: Vec3,
    n: u32,
    distr: &'a impl Fn(f32) -> f32,
) -> impl Iterator<Item = Vec3> + 'a {
    closed_f32_distribution(n, distr).map(move |x| p0.clone().lerp(p1, x))
}

/// Distributes a point `p0` around `axis` `n` times over `[0.0, angle)` radians.
#[inline]
pub fn open_arc<'a>(
    p0: Vec3,
    axis: Vec3,
    n: u32,
    angle: f32,
    distr: &'a impl Fn(f32) -> f32,
) -> impl Iterator<Item = Vec3> + 'a {
    open_f32_distribution(n, distr).map(move |x| Quat::from_axis_angle(axis, angle * x) * p0)
}

/// Distributes a point `p0` around `axis` `n` times over `[0.0, angle]` radians. CCW.
#[inline]
pub fn arc<'a>(
    p0: Vec3,
    axis: Vec3,
    n: u32,
    angle: f32,
    distr: &'a impl Fn(f32) -> f32,
) -> impl Iterator<Item = Vec3> + 'a {
    closed_f32_distribution(n, distr).map(move |x| Quat::from_axis_angle(axis, angle * x) * p0)
}

/// Distributes a point `p0` around `axis` `n` times over `[0.0, angle]` radians.
/// CCW, equally distributed to left and right.
#[inline]
pub fn centered_arc<'a>(
    p0: Vec3,
    axis: Vec3,
    n: u32,
    angle: f32,
    distr: &'a impl Fn(f32) -> f32,
) -> impl Iterator<Item = Vec3> + 'a {
    closed_f32_distribution(n, distr)
        .map(move |x| Quat::from_axis_angle(axis, angle * x - angle / 2.0) * p0)
}

/// Rotates a point `p0` around `axis` `n` times within tau radians.
#[inline]
pub fn circle<'a>(
    p0: Vec3,
    axis: Vec3,
    n: u32,
    distr: &'a impl Fn(f32) -> f32,
) -> impl Iterator<Item = Vec3> + 'a {
    open_arc(p0, axis, n, TAU, distr)
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
        .flat_map(move |(p0, p1)| open_segment(p0, p1, n, distr))
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

// courtesy of the people who write convex hulls in every language. I love you.
// https://github.com/TheAlgorithms/Rust/blob/master/src/general/convex_hull.rs

/// For every point, compares to `min`; sorted by angle difference, then distance difference.
pub fn sort_by_min_angle(pts: &[Vec3], min: &Vec3) -> Vec<Vec3> {
    let mut points: Vec<((f32, f32), Vec3)> = pts
        .iter()
        .map(|x| {
            (
                (
                    // angle
                    (x.z - min.z).atan2(x.x - min.x),
                    // distance (we want the closest to be first)
                    (x.z - min.z).hypot(x.x - min.x),
                ),
                *x,
            )
        })
        .collect();
    points.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));
    points.into_iter().map(|x| x.1).collect()
}

/// Calculates the z coordinate of the vector product of vectors ab and ac.
pub fn calc_z_coord_vector_product(a: &Vec3, b: &Vec3, c: &Vec3) -> f32 {
    (b.x - a.x) * (c.z - a.z) - (c.x - a.x) * (b.z - a.z)
}

/// Convex hull. Points should be y-normalized.
///
/// If three points are aligned and are part of the convex hull then the three are kept.
/// If one doesn't want to keep those points, it is easy to iterate the answer and remove them.
///
/// The first point is the one with the lowest y-coordinate and the lowest x-coordinate.
/// Points are then given counter-clockwise, and the closest one is given first if needed.
pub fn convex_hull_2d(pts: &[Vec3]) -> Vec<Vec3> {
    if pts.is_empty() {
        return vec![];
    }

    let mut stack: Vec<Vec3> = vec![];
    let min = pts.iter().min_by(|a, b| a.lexographic_cmp(&b)).unwrap();

    let points = sort_by_min_angle(pts, min);

    if points.len() <= 3 {
        return points;
    }

    for point in points {
        while stack.len() > 1
            && calc_z_coord_vector_product(&stack[stack.len() - 2], &stack[stack.len() - 1], &point)
                < 0.
        {
            stack.pop();
        }
        stack.push(point);
    }

    stack
}

// https://stackoverflow.com/a/16906278
/// Points should be CCW and y-normalized. Edges included.
pub fn lies_within_convex_hull(hull: &Vec<Vec3>, p: &Vec3) -> bool {
    for (a, b) in hull.iter().circular_tuple_windows() {
        if calc_z_coord_vector_product(a, b, p) < 0.0 {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;

    use super::*;

    #[test]
    fn segment_test() {
        assert_eq!(
            open_segment(Vec3::X, Vec3::NEG_X, 4, &distr::linear).collect::<Vec<_>>(),
            vec![
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.5, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(-0.5, 0.0, 0.0),
            ],
        );
        assert_eq!(
            segment(Vec3::X, Vec3::NEG_X, 5, &distr::linear).collect::<Vec<_>>(),
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
        let a = open_arc(Vec3::X, Vec3::Y, 3, 270.0_f32.to_radians(), &distr::linear)
            .collect::<Vec<_>>();
        assert_relative_eq!(a[0].x, 1.0);
        assert_relative_eq!(a[1].z, -1.0);
        assert_relative_eq!(a[2].x, -1.0);

        let a =
            arc(Vec3::X, Vec3::Y, 3, 270.0_f32.to_radians(), &distr::linear).collect::<Vec<_>>();
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
