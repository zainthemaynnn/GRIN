use bevy::prelude::*;
use bevy_landmass::{Agent, AgentDesiredVelocity, AgentTarget, AgentVelocity};

use crate::{damage::Dead, physics::PhysicsTime, time::Rewind, util::vectors::Vec3Ext};

#[derive(Bundle, Default)]
pub struct MovementBundle {
    pub path_behavior: PathBehavior,
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

#[derive(Component, Clone, Copy, Debug)]
pub struct AttackTarget(pub Entity);

pub fn move_to_target<T: Component>(
    time: Res<PhysicsTime>,
    mut agent_query: Query<
        (
            &mut Transform,
            &mut Agent,
            &mut AgentTarget,
            &AttackTarget,
            &PathBehavior,
        ),
        (With<T>, Without<Rewind>, Without<Dead>),
    >,
    transform_query: Query<&Transform, Without<T>>,
) {
    for (mut transform, mut agent, mut agent_target, AttackTarget(e_target), path_behavior) in
        agent_query.iter_mut()
    {
        let target = transform_query.get(*e_target).unwrap();

        let direction = (target.translation - transform.translation).xz_flat();
        *agent_target = match *path_behavior {
            PathBehavior::Beeline { velocity } => {
                agent.max_velocity = velocity;
                AgentTarget::Entity(*e_target)
            }
            PathBehavior::Strafe {
                circular_velocity,
                radial_velocity,
            } => {
                let angular = match circular_velocity {
                    CircularVelocity::Linear(v) => v / direction.length(),
                    CircularVelocity::Angular(v) => v,
                };
                agent.max_velocity = radial_velocity.hypot(angular);

                let mut new_transform = transform.clone();
                new_transform.translation +=
                    direction.normalize() * radial_velocity * time.0.delta_seconds();
                new_transform.translate_around(
                    target.translation,
                    Quat::from_rotation_y(angular * time.0.delta_seconds()),
                );

                // I'm extending the target point by an arbitrary length tangent to the circle
                // the real fix would be to add an `AgentTarget::Velocity`,
                // but I'm not that responsible
                let ofst = (new_transform.translation - transform.translation).normalize_or_zero();

                AgentTarget::Point(transform.translation + ofst * 64.0)
            }
        };

        transform.look_to(direction, Vec3::Y);
    }
}

pub fn follow_velocity<T: Component>(
    time: Res<PhysicsTime>,
    mut agent_query: Query<
        (&mut Transform, &mut AgentVelocity, &AgentDesiredVelocity),
        (With<T>, Without<Rewind>, Without<Dead>),
    >,
) {
    for (mut transform, mut velocity, desired_velocity) in agent_query.iter_mut() {
        velocity.0 = desired_velocity.velocity();
        transform.translation += desired_velocity.velocity() * time.0.delta_seconds();
    }
}
