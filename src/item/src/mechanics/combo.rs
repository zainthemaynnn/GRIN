//! Defines `ComboStack` and related utilities.

use std::{marker::PhantomData, time::Duration};

use bevy::prelude::*;
use grin_physics::PhysicsTime;
use grin_time::scaling::TimeScale;

pub struct ComboClip {
    pub clip: Handle<AnimationClip>,
    pub chain_cd: Duration,
}

#[derive(Component)]
pub struct ComboStack<T> {
    pub sequence: Vec<T>,
    pub t: Duration,
}

impl<T> Default for ComboStack<T> {
    fn default() -> Self {
        Self {
            sequence: Vec::new(),
            t: Duration::ZERO,
        }
    }
}

impl<T> ComboStack<T> {
    pub fn timestep(&mut self, dt: Duration) {
        self.t = self.t.saturating_sub(dt);
        if self.t == Duration::ZERO {
            self.reset();
        }
    }

    pub fn reset(&mut self) {
        self.sequence.clear();
        self.t = Duration::ZERO;
    }

    pub fn push(&mut self, action: T, expiry: Duration) {
        self.sequence.push(action);
        self.t = expiry;
    }
}

pub fn step_combos<T: Send + Sync + 'static>(
    time: Res<PhysicsTime>,
    mut combos: Query<(&mut ComboStack<T>, &TimeScale)>,
) {
    for (mut combo, time_scale) in combos.iter_mut() {
        combo.timestep(time.0.delta().mul_f32(time_scale.into()));
    }
}

pub struct ComboPlugin<T> {
    phantom_data: PhantomData<T>,
}

impl<T> Default for ComboPlugin<T> {
    fn default() -> Self {
        Self {
            phantom_data: PhantomData::default(),
        }
    }
}

impl<T: Send + Sync + 'static> Plugin for ComboPlugin<T> {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, step_combos::<T>);
    }
}
