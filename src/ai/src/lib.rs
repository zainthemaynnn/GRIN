pub mod boombox;
pub mod bt;
pub mod dummy;
//pub mod metalhead;
pub mod movement;
pub mod screamer;

use bevy::{app::PluginGroupBuilder, prelude::*};
use bevy_landmass::{
    Agent, AgentDesiredVelocity, AgentTarget, AgentVelocity, ArchipelagoRef, LandmassSystemSet, LandmassPlugin,
};
use bevy_mod_inverse_kinematics::InverseKinematicsPlugin;
use bevy_rapier3d::prelude::*;
use grin_damage::{DamageBuffer, Dead, Health, Resist};
use grin_map::MapLoadState;
use grin_physics::{CollisionGroupExt, CollisionGroupsExt, PhysicsTime};
use grin_rig::humanoid::{Humanoid, HumanoidPartType};
use grin_time::Rewind;
use grin_util::event::Spawnable;

use self::{
    boombox::BoomBoxPlugin,
    bt::{Action, Brain, MasterBehaviorPlugin, Verdict},
    dummy::DummyPlugin,
    movement::{update_biped_procedural_walk_cycle, AttackTarget, PathBehavior},
    screamer::ScreamerPlugin,
};

#[derive(SystemSet, Hash, Debug, Eq, PartialEq, Copy, Clone)]
pub enum AiSet {
    /// Run behavior trees.
    RunTrees,
    /// Spawn new NPC's.
    Spawn,
    /// Load new NPC models (preupdate).
    Load,
}

pub struct MasterAiPlugin;

impl Plugin for MasterAiPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            Update,
            (
                LandmassSystemSet::SyncExistence,
                AiSet::RunTrees,
                LandmassSystemSet::SyncValues,
            )
                .chain(),
        )
        .configure_sets(
            PreUpdate,
            AiSet::Load.run_if(in_state(MapLoadState::Success)),
        )
        .configure_sets(
            Update,
            AiSet::Spawn
                .after(AiSet::RunTrees)
                .run_if(in_state(MapLoadState::Success)),
        )
        .add_plugins((MasterBehaviorPlugin, LandmassPlugin, InverseKinematicsPlugin))
        .add_systems(Update, (update_biped_procedural_walk_cycle,));
    }
}

pub struct AiPlugins;

impl PluginGroup for AiPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(MasterAiPlugin)
            .add(BoomBoxPlugin)
            .add(DummyPlugin)
            .add(ScreamerPlugin)
    }
}

pub fn set_closest_attack_target<T: Component, A: Component, E: Component>(
    mut commands: Commands,
    mut agent_query: Query<
        (Entity, &mut Brain, &GlobalTransform),
        (With<T>, With<A>, Without<Rewind>, Without<Dead>),
    >,
    target_query: Query<(Entity, &GlobalTransform), With<E>>,
) {
    for (e_agent, mut brain, src_transform) in agent_query.iter_mut() {
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
            brain.write_verdict(Verdict::Success);
        } else {
            commands.entity(e_agent).remove::<AttackTarget>();
            brain.write_verdict(Verdict::Failure);
        }
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

/// Enemy metadata.
#[derive(Debug)]
pub struct EnemyMeta {
    /// Enemy display name.
    pub name: &'static str,
    /// Enemy description.
    pub description: &'static str,
    /// Enemy flavor text.
    pub flavor: Flavor,
}

/// Some flavor text. I'm thinking one per playable character.
/// 
/// Depends on how many jokes I can come up with. Might be hard...
#[derive(Debug, Default)]
pub struct Flavor {
    pub grin: &'static str,
    pub smirk: &'static str,
    pub grizz: &'static str,
    pub meh: &'static str,
}

pub trait EnemyAgent: Spawnable {
    const DESCRIPTION: EnemyMeta;
}

pub struct DefaultAgentSpawnEvent {
    pub transform: Transform,
}

#[derive(Bundle)]
pub struct EnemyAgentBundle<A: Action> {
    pub health: Health,
    pub resist: Resist,
    pub damage_buffer: DamageBuffer,
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
            damage_buffer: DamageBuffer::default(),
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

fn cooldown_win_lose<T: Component, C: Cooldown, A: Component>(
    time: &PhysicsTime,
    agent_query: &mut Query<(&mut Brain, &mut C), (With<T>, With<A>)>,
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
pub fn protective_cooldown<T: Component, A: Component, C: Cooldown>(
    time: Res<PhysicsTime>,
    mut agent_query: Query<(&mut Brain, &mut C), (With<T>, With<A>)>,
) {
    cooldown_win_lose(&time, &mut agent_query, Verdict::Success, Verdict::Failure);
}

/// Updates cooldown `C`. Writes `Verdict::Success` if ready, `Verdict::Running` otherwise.
pub fn blocking_cooldown<T: Component, A: Component, C: Cooldown>(
    time: Res<PhysicsTime>,
    mut agent_query: Query<(&mut Brain, &mut C), (With<T>, With<A>)>,
) {
    cooldown_win_lose(&time, &mut agent_query, Verdict::Success, Verdict::Running);
}
