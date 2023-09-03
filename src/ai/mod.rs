pub mod boombox;
pub mod bt;
pub mod dummy;
pub mod movement;
pub mod screamer;

use bevy::{app::PluginGroupBuilder, prelude::*};
use bevy_landmass::{
    Agent, AgentDesiredVelocity, AgentTarget, AgentVelocity, ArchipelagoRef, LandmassSystemSet,
};
use bevy_rapier3d::prelude::*;

use crate::{
    damage::{DamageBuffer, Dead, Health, Resist},
    humanoid::{Humanoid, HumanoidPartType},
    item::Target,
    map::MapLoadState,
    physics::{CollisionGroupExt, CollisionGroupsExt},
    time::Rewind,
};

use self::{
    boombox::BoomBoxPlugin,
    dummy::DummyPlugin,
    movement::{AttackTarget, PathBehavior},
    screamer::ScreamerPlugin,
};

#[derive(SystemSet, Hash, Debug, Eq, PartialEq, Copy, Clone)]
pub enum AISet {
    /// Spawns AI's.
    Spawn,
    SpawnFlush,
    /// Setup required for `ActionStart`.
    Target,
    TargetFlush,
    /// Queue actions for `Act`. This is where most of the thinking should happen.
    ActionStart,
    ActionStartFlush,
    /// Perform actions from `ActionStart`.
    Act,
}

pub struct BaseAIPlugin;

impl Plugin for BaseAIPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            Update,
            (
                AISet::Spawn.run_if(in_state(MapLoadState::Success)),
                AISet::SpawnFlush,
                LandmassSystemSet::SyncExistence,
                AISet::Target,
                AISet::TargetFlush,
                AISet::ActionStart,
                AISet::ActionStartFlush,
                LandmassSystemSet::Output,
                AISet::Act,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                apply_deferred.in_set(AISet::SpawnFlush),
                apply_deferred.in_set(AISet::TargetFlush),
                apply_deferred.in_set(AISet::ActionStartFlush),
                apply_deferred
                    .after(LandmassSystemSet::Output)
                    .before(AISet::Act),
            ),
        );
    }
}

pub struct AIPlugins;

impl PluginGroup for AIPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(BaseAIPlugin)
            .add(BoomBoxPlugin)
            .add(DummyPlugin)
            .add(ScreamerPlugin)
    }
}

pub fn set_closest_attack_target<T: Component, E: Component>(
    mut commands: Commands,
    mut self_query: Query<(Entity, &GlobalTransform), (With<T>, Without<Rewind>, Without<Dead>)>,
    target_query: Query<(Entity, &GlobalTransform), With<E>>,
) {
    for (e_agent, src_transform) in self_query.iter_mut() {
        let mut new_target = None;
        let mut target_distance = f32::MAX;
        for (e_target, dst_transform) in target_query.iter() {
            let distance = src_transform
                .translation()
                .distance(dst_transform.translation());
            if distance < target_distance {
                new_target = Some(AttackTarget(e_target));
                target_distance = distance;
            }
        }

        if let Some(t) = new_target {
            commands.entity(e_agent).insert(t);
            trace!("Target: {:?}", t);
        } else {
            commands.entity(e_agent).remove::<AttackTarget>();
            trace!("Target removed.");
        }
    }
}

pub fn propagate_attack_target_to_weapon_target<T: Component>(
    mut agent_query: Query<(&AttackTarget, &mut Target), (With<T>, Without<Rewind>, Without<Dead>)>,
    transform_query: Query<&Transform, Without<T>>,
) {
    for (AttackTarget(e_target), mut target) in agent_query.iter_mut() {
        *target = Target {
            transform: *transform_query.get(*e_target).unwrap(),
            distance: 1.0,
        };
    }
}

pub fn configure_humanoid_physics<T: Component>(
    mut commands: Commands,
    humanoid_query: Query<(Entity, &Humanoid), (Added<Humanoid>, With<T>)>,
) {
    for (e_humanoid, humanoid) in humanoid_query.iter() {
        commands
            .entity(e_humanoid)
            .insert(RigidBody::KinematicVelocityBased);

        for e_part in humanoid.parts(HumanoidPartType::HITBOX) {
            commands.entity(e_part).insert((
                DamageBuffer::default(),
                CollisionGroups::from_group_default(Group::ENEMY),
            ));
        }

        for e_part in humanoid.parts(HumanoidPartType::HANDS) {
            commands
                .entity(e_part)
                .insert(CollisionGroups::new(Group::ENEMY, Group::empty()));
        }
    }
}

#[derive(Bundle)]
pub struct EnemyAgentBundle<A: Action> {
    pub health: Health,
    pub resist: Resist,
    pub brain: Brain,
    pub action: A,
    pub path_behavior: PathBehavior,
    pub archipelago_ref: ArchipelagoRef,
    pub agent: Agent,
    pub velocity: AgentVelocity,
    pub desired_velocity: AgentDesiredVelocity,
    pub agent_target: AgentTarget,
    pub rapier_velocity: Velocity,
    pub rapier_body: RigidBody,
}

impl<A: Action> EnemyAgentBundle<A> {
    pub fn from_archipelago(archipelago: Entity) -> Self {
        Self {
            health: Health::default(),
            resist: Resist::default(),
            brain: Brain::default(),
            action: A::no_op(),
            path_behavior: PathBehavior::default(),
            archipelago_ref: ArchipelagoRef(archipelago),
            agent: Agent {
                radius: 0.5,
                max_velocity: 1.0,
            },
            velocity: AgentVelocity::default(),
            desired_velocity: AgentDesiredVelocity::default(),
            agent_target: AgentTarget::default(),
            rapier_velocity: Velocity::default(),
            rapier_body: RigidBody::KinematicVelocityBased,
        }
    }
}

/// Pretty much any wrapper component around `Timer`.
pub trait Cooldown: Component {
    fn timer(&self) -> &Timer;
    fn timer_mut(&mut self) -> &mut Timer;
}

fn cooldown_win_lose<T: Component, C: Cooldown>(
    time: &PhysicsTime,
    agent_query: &mut Query<(&mut Brain, &mut C), With<T>>,
    win: Verdict,
    lose: Verdict,
) {
    for (mut brain, mut cooldown) in agent_query.iter_mut() {
        brain.write_verdict(
            if cooldown.timer_mut().tick(time.0.delta()).just_finished() {
                win
            } else {
                lose
            },
        );
    }
}

/// Updates cooldown `C`. Writes `Verdict::Success` if ready, `Verdict::Failure` otherwise.
pub fn protective_cooldown<T: Component, C: Cooldown>(
    time: Res<PhysicsTime>,
    mut agent_query: Query<(&mut Brain, &mut C), With<T>>,
) {
    cooldown_win_lose(&time, &mut agent_query, Verdict::Success, Verdict::Failure);
}

/// Updates cooldown `C`. Writes `Verdict::Success` if ready, `Verdict::Running` otherwise.
pub fn blocking_cooldown<T: Component, C: Cooldown>(
    time: Res<PhysicsTime>,
    mut agent_query: Query<(&mut Brain, &mut C), With<T>>,
) {
    cooldown_win_lose(&time, &mut agent_query, Verdict::Success, Verdict::Running);
}
