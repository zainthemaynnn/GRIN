use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use crate::util::vectors::normalize_y;

#[derive(Bundle, Default)]
pub struct MovementBundle {
    pub path_behavior: PathBehavior,
    pub target: MoveTarget,
}

/// Describes how an agent moves towards the target.
#[derive(Component)]
pub enum PathBehavior {
    /// Moves towards the target in a straight line.
    Beeline {
        /// Velocity towards the target.
        velocity: f32,
    },
    /// Pivots around the target.
    Strafe {
        /// Velocity towards the target.
        radial_velocity: f32,
        /// Velocity around the target.
        /// When this is equal to zero it becomes equivalent to `PathBehavior::Beeline`.
        circular_velocity: CircularVelocity,
    },
}

impl Default for PathBehavior {
    fn default() -> Self {
        Self::Beeline { velocity: 1.0 }
    }
}

/// Describes velocity around a circle.
/// Negative indicates clockwise, postive indicates counter-clockwise.
#[derive(Copy, Clone)]
pub enum CircularVelocity {
    /// Velocity based on m/s.
    Linear(f32),
    /// Velocity based on rad/s.
    Angular(f32),
}

#[derive(Component, Default)]
pub struct MoveTarget(pub Transform);

pub fn move_to_target<T: Component>(
    mut query: Query<
        (
            &mut KinematicCharacterController,
            &mut Transform,
            &MoveTarget,
            &PathBehavior,
        ),
        With<T>,
    >,
    time: Res<Time>,
) {
    for (mut controller, mut transform, MoveTarget(target), path_behavior) in query.iter_mut() {
        let t = time.delta_seconds();
        let direction = normalize_y(target.translation - transform.translation);
        let translation = match *path_behavior {
            PathBehavior::Beeline { velocity } => {
                (target.translation - transform.translation) * velocity * t
            }
            PathBehavior::Strafe {
                circular_velocity,
                radial_velocity,
            } => {
                let mut new_transform = transform.clone();
                new_transform.translation += direction.normalize() * radial_velocity * t;
                let angular = match circular_velocity {
                    CircularVelocity::Linear(v) => v / direction.length(),
                    CircularVelocity::Angular(v) => v,
                };
                new_transform
                    .translate_around(target.translation, Quat::from_rotation_y(angular * t));
                new_transform.translation - transform.translation
            }
        };
        controller.translation = Some(translation);
        transform.look_to(direction, Vec3::Y);
    }
}
