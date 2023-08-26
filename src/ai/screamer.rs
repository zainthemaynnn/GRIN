//! This module is for the basic Screamer, but other Screamer derivations use
//! some definitions from this module.

use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_landmass::Agent;
use bevy_mod_inverse_kinematics::IkConstraint;
use bevy_rapier3d::prelude::*;
use itertools::Itertools;

use crate::{
    asset::AssetLoadState,
    character::PlayerCharacter,
    item::Target,
    map::NavMesh,
    util::{event::Spawnable, query::gltf_path_search},
};

use super::{
    movement::{
        match_desired_velocity, propagate_attack_target_to_agent_target,
        update_biped_procedural_walk_cycle, IkProc, IkProcs,
    },
    propagate_attack_target_to_weapon_target, set_closest_attack_target, AISet, EnemyAgentBundle,
};

pub struct ScreamerPlugin;

impl Plugin for ScreamerPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<ScreamerSpawnEvent>()
            .add_collection_to_loading_state::<_, ScreamerAssets>(AssetLoadState::Loading)
            .add_systems(
                Update,
                (
                    spawn.in_set(AISet::Spawn),
                    init_rig.in_set(AISet::Spawn),
                    set_closest_attack_target::<Screamer, PlayerCharacter>.in_set(AISet::Target),
                    propagate_attack_target_to_weapon_target::<Screamer>.in_set(AISet::ActionStart),
                    propagate_attack_target_to_agent_target::<Screamer>.in_set(AISet::ActionStart),
                    match_desired_velocity::<Screamer>.in_set(AISet::Act),
                    update_biped_procedural_walk_cycle::<Screamer>.in_set(AISet::Act),
                ),
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

#[derive(Resource, AssetCollection)]
pub struct ScreamerAssets {
    #[asset(key = "rig.screamer")]
    pub skeleton: Handle<Scene>,
    #[asset(key = "anim.screamer.idle.armed")]
    pub idle: Handle<AnimationClip>,
    #[asset(key = "sfx.stomp")]
    pub stomp: Handle<AudioSource>,
}

pub fn spawn(
    mut commands: Commands,
    nav_mesh: Res<NavMesh>,
    assets: Res<ScreamerAssets>,
    mut events: EventReader<ScreamerSpawnEvent>,
) {
    for ScreamerSpawnEvent { transform } in events.iter() {
        commands.spawn((
            Screamer,
            Target::default(),
            RigidBody::KinematicVelocityBased,
            Velocity::default(),
            SceneBundle {
                scene: assets.skeleton.clone(),
                transform: *transform,
                ..Default::default()
            },
            EnemyAgentBundle {
                agent: Agent {
                    radius: 1.5,
                    max_velocity: 16.0,
                },
                ..EnemyAgentBundle::from_archipelago(nav_mesh.archipelago)
            },
        ));
    }
}

pub fn init_rig(
    mut commands: Commands,
    assets: Res<ScreamerAssets>,
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

        commands.entity(e_screamer).insert(IkProcs {
            procs,
            scare_distance: 1.0,
            step_duration: 0.1,
            step_height: 0.5,
            audio: Some(assets.stomp.clone()),
            active_proc: 0,
        });
    }
}
