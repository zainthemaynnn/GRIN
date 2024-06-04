use std::time::Duration;

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use grin_physics::PhysicsTime;
use grin_render::blaze::Blaze;

use crate::hit::{ContactDamage, Damage};

#[derive(Component, Default)]
pub struct DotRegion {
    pub timer: Timer,
    pub radius: f32,
}

pub fn tick_dot(
    mut commands: Commands,
    time: Res<PhysicsTime>,
    mut dot_query: Query<(Entity, &mut DotRegion, &mut Transform)>,
) {
    for (e_dot, mut dot, mut transform) in dot_query.iter_mut() {
        if dot.timer.tick(time.0.delta()).just_finished() {
            commands.entity(e_dot).despawn();
        }
        transform.scale = Vec3::new(dot.radius, 1.0, dot.radius);
    }
}

impl From<&DotRegion> for Blaze {
    fn from(dot: &DotRegion) -> Self {
        Self {
            radius: dot.radius,
            ..Default::default()
        }
    }
}

pub fn sync_blaze_to_dot(mut dot_query: Query<(&mut Blaze, &DotRegion), Changed<DotRegion>>) {
    for (mut blaze, dot) in dot_query.iter_mut() {
        blaze.radius = dot.radius;
    }
}

/// This is really just a sensor collider with contact damage added to it, which mimics a
/// DOT damage area effect. The bundle makes it a bit easier and handles region expiration
/// time with the `DotRegion` component.
#[derive(Bundle)]
pub struct DotRegionBundle {
    pub dot_region: DotRegion,
    pub damage: Damage,
    /// This should generally be `ContactDamage::Debounce`, unless something sus is happening.
    pub contact_damage: ContactDamage,
    pub collider: Collider,
    pub collision_groups: CollisionGroups,
    pub sensor: Sensor,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
}

impl Default for DotRegionBundle {
    fn default() -> Self {
        // "wow, it's been a while. now that my first exam season is over I can get back to writing rust."
        // "oh, right. rust."
        Self {
            // radius gets auto-synced in `tick_dot`
            collider: Collider::cylinder(Self::DEFAULT_DOT_HALF_HEIGHT, 1.0),
            contact_damage: ContactDamage::Debounce(Duration::from_millis(
                Self::DEFAULT_DOT_DEBOUNCE_MILLIS,
            )),
            dot_region: DotRegion::default(),
            damage: Damage::default(),
            collision_groups: CollisionGroups::default(),
            sensor: Sensor::default(),
            transform: Transform::default(),
            global_transform: GlobalTransform::default(),
        }
    }
}

impl DotRegionBundle {
    pub const DEFAULT_DOT_HALF_HEIGHT: f32 = 1.0;
    pub const DEFAULT_DOT_DEBOUNCE_MILLIS: u64 = 100;
}
