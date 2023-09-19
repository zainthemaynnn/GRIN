//! This module is for the basic Screamer, but other Screamer derivations use
//! some definitions from this module.

use std::time::Duration;

use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_enum_filter::prelude::*;
use bevy_landmass::Agent;
use bevy_mod_inverse_kinematics::IkConstraint;
use bevy_rapier3d::prelude::*;
use grin_derive::Cooldown;
use itertools::Itertools;

use crate::{
    asset::AssetLoadState,
    bt,
    character::PlayerCharacter,
    damage::{
        projectiles::{BulletProjectile, ProjectileBundle, ProjectileColor},
        Damage, DamageVariant,
    },
    map::NavMesh,
    sound::TrackedSpatialAudioBundle,
    util::{event::Spawnable, query::gltf_path_search, vectors::Vec3Ext},
};

use super::{
    blocking_cooldown,
    bt::{
        tree::CompositeNode, AiModel, BehaviorIteration, BehaviorSet, Brain, EnumBehaviorPlugin,
        Verdict,
    },
    movement::{
        match_desired_velocity, propagate_attack_target_to_agent_target, zero_velocity,
        AttackTarget, IkProc, IkProcs,
    },
    protective_cooldown, set_closest_attack_target, AiSet, EnemyAgentBundle,
};

#[derive(Component, Cooldown)]
#[cooldown(duration = 2.0)]
pub struct BassCannonCooldown(pub Timer);

#[derive(Component, Cooldown)]
#[cooldown(duration = 1.0)]
pub struct BassCannonAim(pub Timer);

#[derive(Component, Cooldown)]
#[cooldown(duration = 40.0 / 60.0)]
pub struct BassCannonSelfStun(pub Timer);

pub struct ScreamerPlugin;

impl Plugin for ScreamerPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<ScreamerSpawnEvent>()
            .add_collection_to_loading_state::<_, ScreamerAssets>(AssetLoadState::Loading)
            .add_plugins(EnumBehaviorPlugin::<ScreamerAi>::default())
            .insert_resource(AiModel {
                bt: bt! {
                    Composite(CompositeNode::Sequence) {
                        Leaf(ScreamerAi::Track),
                        Leaf(ScreamerAi::Target),
                        Composite(CompositeNode::Selector) {
                            Composite(CompositeNode::Sequence) {
                                Leaf(ScreamerAi::BassCooldownCheck),
                                Leaf(ScreamerAi::EndChase),
                                Composite(CompositeNode::Sequence) {
                                    Leaf(ScreamerAi::AimBegin),
                                    Leaf(ScreamerAi::AimCheck),
                                },
                                Leaf(ScreamerAi::BassCannon),
                                Leaf(ScreamerAi::BassCannonSelfStun),
                                Leaf(ScreamerAi::SetIdle),
                            },
                            Leaf(ScreamerAi::Chase),
                        },
                    },
                },
            })
            .add_systems(Update, spawn.in_set(AiSet::Spawn))
            .add_systems(PreUpdate, load.in_set(AiSet::Load))
            .add_systems(
                BehaviorIteration,
                (
                    protective_cooldown::<Screamer, Enum!(ScreamerAi::BassCooldownCheck), BassCannonCooldown>,
                    blocking_cooldown::<Screamer, Enum!(ScreamerAi::AimCheck), BassCannonAim>,
                    blocking_cooldown::<Screamer, Enum!(ScreamerAi::BassCannonSelfStun), BassCannonSelfStun>,
                    set_closest_attack_target::<Screamer, Enum!(ScreamerAi::Track), PlayerCharacter>,
                    propagate_attack_target_to_agent_target::<Screamer, Enum!(ScreamerAi::Target)>,
                    match_desired_velocity::<Screamer, Enum!(ScreamerAi::Chase)>,
                    zero_velocity::<Screamer, Enum!(ScreamerAi::EndChase)>,
                    set_idle::<Enum!(ScreamerAi::SetIdle)>,
                    aim_begin::<Enum!(ScreamerAi::AimBegin)>,
                    bass_cannon::<Enum!(ScreamerAi::BassCannon)>,
                )
                    .in_set(BehaviorSet::Act),
            );
    }
}

#[derive(Component, Default)]
pub struct Screamer;

impl Spawnable for Screamer {
    type Event = ScreamerSpawnEvent;
}

#[derive(Event, Clone, Default)]
pub struct ScreamerSpawnEvent {
    pub transform: Transform,
}

#[derive(Component)]
pub struct ScreamerParts {
    pub armature: Entity,
}

#[derive(Component, EnumFilter, Clone, Copy, Debug, Default)]
pub enum ScreamerAi {
    #[default]
    Empty,
    Track,
    Target,
    Chase,
    EndChase,
    BassCooldownCheck,
    AimBegin,
    AimCheck,
    BassCannon,
    BassCannonSelfStun,
    SetIdle,
}

#[derive(Resource, AssetCollection)]
pub struct ScreamerAssets {
    #[asset(key = "rig.screamer")]
    pub skeleton: Handle<Scene>,
    #[asset(key = "anim.screamer.idle.armed")]
    pub idle: Handle<AnimationClip>,
    #[asset(key = "anim.screamer.aim")]
    pub aim: Handle<AnimationClip>,
    #[asset(key = "anim.screamer.bassready")]
    pub bass_ready: Handle<AnimationClip>,
    #[asset(key = "anim.screamer.bass")]
    pub bass: Handle<AnimationClip>,
    #[asset(key = "anim.screamer.scream")]
    pub scream: Handle<AnimationClip>,
    #[asset(key = "sfx.stomp")]
    pub stomp: Handle<AudioSource>,
    #[asset(key = "sfx.bass_cannon")]
    pub bass_sfx: Handle<AudioSource>,
}

pub fn spawn(
    mut commands: Commands,
    assets: Res<ScreamerAssets>,
    mut events: EventReader<ScreamerSpawnEvent>,
) {
    for ScreamerSpawnEvent { transform } in events.iter() {
        commands.spawn((
            Screamer,
            SceneBundle {
                scene: assets.skeleton.clone(),
                transform: *transform,
                ..Default::default()
            },
        ));
    }
}

pub fn load(
    mut commands: Commands,
    assets: Res<ScreamerAssets>,
    nav_mesh: Res<NavMesh>,
    screamer_query: Query<(Entity, &GlobalTransform), (Added<Children>, With<Screamer>)>,
    mut animator_query: Query<&mut AnimationPlayer>,
    children_query: Query<&Children>,
    name_query: Query<&Name>,
    g_transform_query: Query<&GlobalTransform>,
) {
    // glad I'm not one of those 8-space guys...
    for (e_screamer, g_transform) in screamer_query.iter() {
        let e_root = children_query.get(e_screamer).unwrap()[0];

        let e_armature = gltf_path_search(
            &EntityPath {
                parts: vec!["Armature".into()],
            },
            e_root,
            &children_query,
            &name_query,
        )
        .unwrap();

        let mut animator = animator_query.get_mut(e_armature).unwrap();
        animator.play(assets.idle.clone()).repeat();

        let e_legs = gltf_path_search(
            &EntityPath {
                parts: vec!["Legs".into()],
            },
            e_root,
            &children_query,
            &name_query,
        )
        .unwrap();

        let procs = ["Left", "Right"]
            .into_iter()
            .map(|side| {
                let e_upper_leg = gltf_path_search(
                    &EntityPath {
                        parts: vec![format!("{side}UpperLeg").into()],
                    },
                    e_legs,
                    &children_query,
                    &name_query,
                )
                .unwrap();

                let e_foot = gltf_path_search(
                    &EntityPath {
                        parts: vec![
                            format!("{side}LowerLeg").into(),
                            format!("{side}Foot").into(),
                        ],
                    },
                    e_upper_leg,
                    &children_query,
                    &name_query,
                )
                .unwrap();

                let gt_upper_leg = g_transform_query.get(e_upper_leg).unwrap();
                let gt_foot = g_transform_query.get(e_foot).unwrap();

                // this gets locked to the default foot position
                let home = {
                    let ik_transform = gt_foot.reparented_to(g_transform);
                    commands
                        .spawn((
                            TransformBundle::from_transform(ik_transform),
                            RigidBody::KinematicPositionBased,
                            Velocity::default(),
                        ))
                        .id()
                };
                commands.entity(e_screamer).add_child(home);

                // this goes in global space
                let target = commands
                    .spawn(TransformBundle::from_transform(gt_foot.compute_transform()))
                    .id();

                // this goes some ways away collinear with the upper leg
                let pole_target = {
                    let mut ik_transform = gt_upper_leg.reparented_to(g_transform);
                    // arbitrary scale
                    ik_transform.translation += ik_transform.local_y() * 4.0;
                    commands
                        .spawn(TransformBundle::from_transform(ik_transform))
                        .id()
                };
                commands.entity(e_screamer).add_child(pole_target);

                commands.entity(e_foot).insert(IkConstraint {
                    target,
                    pole_target: Some(pole_target),
                    pole_angle: -std::f32::consts::FRAC_PI_2,
                    chain_length: 2,
                    iterations: 20,
                    enabled: true,
                });

                IkProc::new(home, target)
            })
            .collect_vec();

        commands.entity(e_screamer).insert((
            EnemyAgentBundle::<ScreamerAi> {
                agent: Agent {
                    radius: 1.5,
                    max_velocity: 16.0,
                },
                ..EnemyAgentBundle::from_archipelago(nav_mesh.archipelago)
            },
            ScreamerParts {
                armature: e_armature,
            },
            BassCannonCooldown::default(),
            BassCannonAim::default(),
            BassCannonSelfStun::default(),
            IkProcs {
                procs,
                scare_distance: 1.0,
                step_duration: 0.1,
                step_height: 0.5,
                audio: Some(assets.stomp.clone()),
                active_proc: 0,
            },
        ));
    }
}

pub fn aim_begin<T: Component>(
    assets: Res<ScreamerAssets>,
    mut agent_query: Query<(&mut Brain, &ScreamerParts), (With<Screamer>, With<T>)>,
    mut animator_query: Query<&mut AnimationPlayer>,
) {
    for (mut brain, parts) in agent_query.iter_mut() {
        let mut animator = animator_query.get_mut(parts.armature).unwrap();
        animator.play_with_transition(assets.bass_ready.clone(), Duration::from_secs_f32(0.2));
        brain.write_verdict(Verdict::Success);
    }
}

pub fn bass_cannon<T: Component>(
    mut commands: Commands,
    assets: Res<ScreamerAssets>,
    mut agent_query: Query<
        (Entity, &mut Brain, &ScreamerParts, &AttackTarget),
        (With<Screamer>, With<T>),
    >,
    mut animator_query: Query<&mut AnimationPlayer>,
    g_transform_query: Query<&GlobalTransform>,
) {
    for (e_screamer, mut brain, parts, AttackTarget(e_target)) in agent_query.iter_mut() {
        let mut animator = animator_query.get_mut(parts.armature).unwrap();
        animator.play(assets.bass.clone());

        let (origin, target) = (
            g_transform_query.get(parts.armature).unwrap(),
            g_transform_query.get(*e_target).unwrap(),
        );
        let bullet_transform = Transform::from_translation(origin.translation())
            .looking_at(target.translation().with_y(origin.translation().y), Vec3::Y);

        commands.spawn(TrackedSpatialAudioBundle {
            source: assets.bass_sfx.clone(),
            settings: PlaybackSettings::DESPAWN,
            ..Default::default()
        });

        commands.spawn((
            BulletProjectile,
            ProjectileBundle {
                color: ProjectileColor::Red,
                damage: Damage {
                    ty: DamageVariant::Ballistic,
                    value: 5.0,
                    source: Some(e_screamer),
                },
                transform: bullet_transform.with_scale(Vec3::splat(3.0)),
                velocity: Velocity::linear(bullet_transform.forward() * 24.0),
                ccd: Ccd::enabled(),
                ..ProjectileBundle::enemy_default()
            },
        ));

        brain.write_verdict(Verdict::Success);
    }
}

pub fn set_idle<T: Component>(
    assets: Res<ScreamerAssets>,
    mut agent_query: Query<(&mut Brain, &ScreamerParts), (With<Screamer>, With<T>)>,
    mut animator_query: Query<&mut AnimationPlayer>,
) {
    for (mut brain, parts) in agent_query.iter_mut() {
        let mut animator = animator_query.get_mut(parts.armature).unwrap();
        animator.play_with_transition(assets.idle.clone(), Duration::from_secs_f32(0.2));
        brain.write_verdict(Verdict::Success);
    }
}
