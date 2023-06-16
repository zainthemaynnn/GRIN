use bevy::prelude::*;

use super::Active;

#[derive(Component)]
pub struct SingleGrip {
    pub grip: Vec3,
}

#[derive(Component)]
pub struct DoubleGrip {
    pub single: Vec3,
    pub double: Vec3,
}

impl DoubleGrip {
    pub fn hammer() -> Self {
        Self {
            single: Vec3::ZERO,
            double: Vec3::new(0.0, -0.5, 0.0),
        }
    }
}

/// Wind-up progress for a melee weapon.
#[derive(Component)]
pub struct Wind {
    /// Current charge amount in seconds.
    pub charge: f32,
    /// Time to reach max charge in seconds.
    pub max: f32,
}

impl Wind {
    pub fn new(max: f32) -> Self {
        Self { charge: 0.0, max }
    }

    pub fn progress(&self) -> f32 {
        self.charge / self.max
    }
}

impl Default for Wind {
    fn default() -> Self {
        Self::new(1.0)
    }
}

#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct Winding {
    pub duration: f32,
}

#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct Charging;

#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct Swinging {
    pub duration: f32,
}

pub fn update_hammer_winds(time: Res<Time>, mut wind_query: Query<(&mut Wind, Option<&Winding>)>) {
    for (mut wind, active) in wind_query.iter_mut() {
        if active.is_some() {
            wind.charge += time.delta_seconds();
        } else {
            wind.charge = 0.0;
        }
    }
}
