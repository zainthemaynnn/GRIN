use bevy::prelude::Vec3;

pub trait Vec3Ext {
    /// Creates an arbitrary perpendicular vector.
    fn perp(&self) -> Self;
}

impl Vec3Ext for Vec3 {
    fn perp(&self) -> Self {
        if self.z.abs() < self.x.abs() {
            Vec3::new(self.y, -self.x, 0.0)
        } else {
            Vec3::new(0.0, -self.z, self.y)
        }
    }
}

#[inline]
pub fn normalize_y(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z)
}
