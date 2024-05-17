use bevy::{math::cubic_splines::CubicCurve, prelude::*};
use bevy_landmass::{Agent, AgentDesiredVelocity, AgentTarget, AgentVelocity};
use bevy_rapier3d::prelude::*;
use grin_damage::Dead;
use grin_physics::PhysicsTime;
use grin_time::{
    scaling::{RawVelocity, TimeScale},
    Rewind,
};
use grin_util::{numbers::MulStack, vectors::Vec3Ext};

use super::bt::{Brain, Verdict};

/// Proportional constant for the angular velocity P controller.
pub const AGENT_ANGULAR_VELOCITY_P: f32 = 1.0;

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

pub fn propagate_attack_target_to_agent_target<T: Component, A: Component>(
    time: Res<PhysicsTime>,
    mut agent_query: Query<
        (
            &mut Brain,
            &mut Agent,
            &mut AgentTarget,
            &mut RawVelocity,
            &Velocity,
            &Transform,
            &AttackTarget,
            &PathBehavior,
        ),
        (With<T>, With<A>, Without<Rewind>, Without<Dead>),
    >,
    transform_query: Query<&Transform, Without<T>>,
) {
    let dt = time.0.delta_seconds();
    for (
        mut brain,
        mut agent,
        mut agent_target,
        mut raw_velocity,
        velocity,
        transform,
        AttackTarget(e_target),
        path_behavior,
    ) in agent_query.iter_mut()
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
                new_transform.translation += direction.normalize() * radial_velocity * dt;
                new_transform
                    .translate_around(target.translation, Quat::from_rotation_y(angular * dt));

                // I'm extending the target point by an arbitrary length tangent to the circle
                // the real fix would be to add an `AgentTarget::Velocity`,
                // but I'm not that responsible
                let ofst = (new_transform.translation - transform.translation).normalize_or_zero();

                AgentTarget::Point(transform.translation + ofst * 64.0)
            }
        };

        let angle_diff = Quat::from_rotation_arc(
            transform.forward().xz_flat().normalize(),
            direction.normalize(),
        );
        let (axis, mut angle_diff) = angle_diff.to_axis_angle();
        // we do a little trolling
        if axis == Vec3::NEG_Y {
            angle_diff *= -1.0;
        }

        raw_velocity.0.angvel = Vec3::Y * AGENT_ANGULAR_VELOCITY_P * angle_diff;
        dbg!("next");
        dbg!(raw_velocity.0.angvel);
        dbg!(velocity.angvel);

        brain.write_verdict(Verdict::Success);
    }
}

// TODO: this has frame lag. order it after `LandmassSystemSet::Output`.
pub fn match_desired_velocity<T: Component, A: Component>(
    mut agent_query: Query<
        (
            &mut Brain,
            &mut Velocity,
            &mut AgentVelocity,
            &AgentDesiredVelocity,
        ),
        (With<T>, With<A>, Without<Rewind>, Without<Dead>),
    >,
) {
    for (mut brain, mut velocity, mut agent_velocity, desired_velocity) in agent_query.iter_mut() {
        velocity.linvel = desired_velocity.velocity();
        agent_velocity.0 = velocity.linvel;
        brain.write_verdict(Verdict::Success);
    }
}

pub fn zero_velocity<T: Component, A: Component>(
    mut agent_query: Query<
        (&mut Brain, &mut Velocity, &mut AgentVelocity),
        (With<T>, With<A>, Without<Rewind>, Without<Dead>),
    >,
) {
    for (mut brain, mut velocity, mut agent_velocity) in agent_query.iter_mut() {
        velocity.linvel = Vec3::ZERO;
        agent_velocity.0 = velocity.linvel;
        brain.write_verdict(Verdict::Success);
    }
}

/// It's really just what it sounds like.
#[derive(Component, Default)]
pub struct AgentVelocityMultiplier {
    pub mulstack: MulStack,
}

impl From<&AgentVelocityMultiplier> for f32 {
    fn from(value: &AgentVelocityMultiplier) -> Self {
        f32::from(&value.mulstack)
    }
}

// this guy explains everything. thanks!
// https://weaverdev.io/projects/bonehead-procedural-animation/
// note that I didn't do the batch thing. I've modified this for bipeds.
// might do another implementation if I ever get >2 legs, which is likely.

/// A system of IK constraints, to synchronize stepping maneouvers.
///
/// This one will ensure that each proc steps in order, regardless of which is out
/// of range, until all procs are in range.
#[derive(Component)]
pub struct IkProcs {
    /// List of IK constraints.
    pub procs: Vec<IkProc>,
    /// Maximum distance for any of the IK procs before triggering.
    pub scare_distance: f32,
    /// Duration of IK step.
    pub step_duration: f32,
    /// Y displacement at peak of IK step.
    pub step_height: f32,
    /// Sound when the step lands.
    pub audio: Option<Handle<AudioSource>>,
    /// Which proc should step next.
    pub active_proc: usize,
}

impl IkProcs {
    /// Whether any procs are active.
    pub fn stepping(&self) -> bool {
        self.procs.iter().find(|p| p.step_state.is_some()).is_some()
    }

    /// Whether all procs are in range (do not need stepping).
    pub fn all_in_range(&self, g_transform_query: &Query<&GlobalTransform>) -> bool {
        self.procs
            .iter()
            .find(|proc| {
                g_transform_query
                    .get(proc.home)
                    .unwrap()
                    .translation()
                    .distance(g_transform_query.get(proc.target).unwrap().translation())
                    > self.scare_distance
            })
            .is_none()
    }

    /// Updates all active procs.
    pub fn step_all(
        &mut self,
        dt: f32,
        commands: &mut Commands,
        transform_query: &mut Query<&mut Transform>,
    ) {
        for proc in self.procs.iter_mut() {
            proc.step(dt, commands, transform_query, self.audio.as_ref());
        }
    }
}

/// Represents one IK constraint.
pub struct IkProc {
    /// "Rest" position for IK.
    pub home: Entity,
    /// Target position for IK.
    pub target: Entity,
    /// Step motion path, if currently stepping.
    pub step_state: Option<StepState>,
}

impl IkProc {
    pub fn new(home: Entity, target: Entity) -> Self {
        Self {
            home,
            target,
            step_state: None,
        }
    }

    /// Begins a step depending on current transforms + velocities.
    pub fn begin_step(
        &mut self,
        step_height: f32,
        step_duration: f32,
        transform_query: &Query<&GlobalTransform>,
        velocity_query: &Query<&Velocity>,
    ) {
        let g_home_transform = transform_query.get(self.home).unwrap();
        let g_target_transform = transform_query.get(self.target).unwrap();

        // configure motion path
        let (_, src_rotation, src_translation) = g_target_transform.to_scale_rotation_translation();
        let (_, mut dst_rotation, mut dst_translation) =
            g_home_transform.to_scale_rotation_translation();

        // use velocity prediction so that the constraint will reach the
        // intended target by the end of its step duration
        let velocity = velocity_query.get(self.home).unwrap();
        let rot_predict = Quat::from_rotation_y(velocity.angvel.y * step_duration);
        dst_translation += rot_predict * velocity.linvel * step_duration;
        dst_rotation *= Quat::from_rotation_y(velocity.angvel.y * step_duration);

        // use the midpoint with added step height for the middle two points
        let center = src_translation.lerp(dst_translation, 0.5) + Vec3::Y * step_height;
        self.step_state = Some(StepState {
            curve: CubicBezier::new([[src_translation, center, center, dst_translation]])
                .to_curve(),
            quat0: src_rotation,
            quat1: dst_rotation,
            t: 0.0,
        });
    }

    /// Updates the step if active.
    pub fn step(
        &mut self,
        dt: f32,
        commands: &mut Commands,
        transform_query: &mut Query<&mut Transform>,
        audio: Option<&Handle<AudioSource>>,
    ) {
        let Some(step_state) = &mut self.step_state else {
            return;
        };

        let mut target_transform = transform_query.get_mut(self.target).unwrap();
        *target_transform = step_state.step(dt);

        if step_state.done() {
            self.step_state = None;

            // play sound
            if let Some(audio) = audio {
                commands.spawn((
                    AudioBundle {
                        source: audio.clone(),
                        settings: PlaybackSettings::DESPAWN,
                        ..Default::default()
                    },
                    TransformBundle::from_transform(target_transform.clone()),
                ));
            };
        }
    }
}

/// IK step motion path.
pub struct StepState {
    /// Translation path.
    pub curve: CubicCurve<Vec3>,
    /// Initial rotation.
    pub quat0: Quat,
    /// Final rotation.
    pub quat1: Quat,
    /// Completion of path, within `[0.0, 1.0]`.
    pub t: f32,
}

impl StepState {
    /// Target position after `dt`.
    pub fn step(&mut self, dt: f32) -> Transform {
        self.t = (self.t + dt).min(1.0);
        Transform {
            translation: self.curve.position(self.t),
            rotation: self.quat0.lerp(self.quat1, self.t),
            ..Default::default()
        }
    }

    /// Whether the step has finished.
    pub fn done(&self) -> bool {
        self.t == 1.0
    }
}

pub fn update_biped_procedural_walk_cycle(
    mut commands: Commands,
    time: Res<PhysicsTime>,
    mut agent_query: Query<(&mut IkProcs, &TimeScale)>,
    mut transform_query: Query<&mut Transform>,
    g_transform_query: Query<&GlobalTransform>,
    velocity_query: Query<&Velocity>,
) {
    for (mut ik_procs, time_scale) in agent_query.iter_mut() {
        // update active `IkProc`s
        // note: this works for multiple steps at a time, although really only one should
        // be active at a time for bipeds
        let dt = (time.0.delta_seconds() * f32::from(time_scale)) / ik_procs.step_duration;
        ik_procs.step_all(dt, &mut commands, &mut transform_query);

        if !ik_procs.stepping() && !ik_procs.all_in_range(&g_transform_query) {
            // copy these cause borrow checker
            let IkProcs {
                active_proc,
                step_height,
                step_duration,
                ..
            };

            ik_procs.procs[active_batch].begin_step(
                step_height,
                step_duration,
                &g_transform_query,
                &velocity_query,
            );

            // configure next proc for stepping
            ik_procs.active_proc = (ik_procs.active_proc + 1) % 2;
        }
    }
}
