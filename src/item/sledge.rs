use std::time::Duration;

use crate::{
    asset::AssetLoadState,
    character::Player,
    collider,
    collisions::{CollisionGroupExt, CollisionGroupsExt},
    damage::{ContactDamage, Damage, DamageVariant},
    humanoid::Humanoid,
    item::melee::Charging,
    render::sketched::SketchMaterial,
};

use super::{
    firing::{self, FireRate, FiringPlugin, FiringType, SemiFireBundle, ShotFired},
    insert_on_lmb, insert_on_rmb,
    melee::{update_hammer_winds, Swinging, Wind, Winding},
    set_local_mouse_target, Aiming, Item, ItemEquipEvent, ItemPlugin, ItemSet, ItemSpawnEvent, Sfx,
    WeaponBundle,
};
pub use super::{Active, Target};
use bevy::prelude::*;
use bevy_asset_loader::prelude::{AssetCollection, LoadingStateAppExt};
use bevy_rapier3d::prelude::*;

pub struct SledgePlugin;

#[derive(Resource, AssetCollection)]
pub struct SledgeAssets {
    #[asset(key = "mesh.sledge")]
    pub sledge: Handle<Mesh>,
    #[asset(key = "mat.body_gray")]
    pub sledge_material: Handle<SketchMaterial>,
    #[asset(key = "anim.hammer.charge")]
    pub charge_animation: Handle<AnimationClip>,
    #[asset(key = "anim.hammer.swing")]
    pub swing_animation: Handle<AnimationClip>,
    #[asset(key = "anim.hammer.unswing")]
    pub unswing_animation: Handle<AnimationClip>,
    #[asset(key = "anim.hammer.wind")]
    pub wind_animation: Handle<AnimationClip>,
}

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum SledgeSystemSet {
    Input,
    Fire,
    Effects,
}

impl Plugin for SledgePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(ItemPlugin::<Sledge>::default())
            .add_plugin(FiringPlugin::<Sledge>::from(FiringType::SemiAutomatic))
            .add_collection_to_loading_state::<_, SledgeAssets>(AssetLoadState::Loading)
            .configure_sets(
                (
                    SledgeSystemSet::Input
                        .run_if(in_state(AssetLoadState::Success))
                        .before(firing::semi_fire::<Sledge>),
                    SledgeSystemSet::Fire
                        .run_if(in_state(AssetLoadState::Success))
                        .after(firing::semi_fire::<Sledge>),
                    SledgeSystemSet::Effects.run_if(in_state(AssetLoadState::Success)),
                )
                    .chain(),
            )
            .add_system(spawn.in_set(ItemSet::Spawn))
            .add_systems(
                (
                    set_local_mouse_target::<Sledge>,
                    insert_on_lmb::<Sledge, Active>,
                    insert_on_rmb::<Sledge, Aiming>,
                    apply_system_buffers,
                    wind,
                    charge,
                    apply_system_buffers,
                    swing_or_cancel,
                    unswing,
                    update_hammer_winds,
                )
                    .chain()
                    .in_set(SledgeSystemSet::Input),
            );
    }
}

#[derive(Component, Default)]
pub struct Sledge;

impl Item for Sledge {
    type SpawnEvent = ItemSpawnEvent<Sledge>;
    type EquipEvent = ItemEquipEvent<Sledge>;
}

pub fn spawn(
    mut commands: Commands,
    assets: Res<SledgeAssets>,
    meshes: Res<Assets<Mesh>>,
    humanoid_query: Query<&Humanoid>,
    mut spawn_events: EventReader<ItemSpawnEvent<Sledge>>,
    mut equip_events: EventWriter<ItemEquipEvent<Sledge>>,
) {
    for ItemSpawnEvent { parent_entity, .. } in spawn_events.iter() {
        let humanoid = humanoid_query.get(*parent_entity).unwrap();

        let item_entity = commands
            .spawn((
                Sledge::default(),
                Wind::new(1.0),
                WeaponBundle::default(),
                MaterialMeshBundle {
                    mesh: assets.sledge.clone(),
                    material: assets.sledge_material.clone(),
                    ..Default::default()
                },
                SemiFireBundle {
                    fire_rate: FireRate(2.0),
                    ..Default::default()
                },
                RigidBody::KinematicPositionBased,
                collider!(meshes, &assets.sledge),
                CollisionGroups::from_group_default(Group::PLAYER_PROJECTILE),
                Sensor,
                Damage {
                    ty: DamageVariant::Ballistic,
                    value: 20.0,
                    source: None,
                },
            ))
            .set_parent(humanoid.dominant_hand())
            .id();

        equip_events.send(ItemEquipEvent::new(*parent_entity, item_entity));
    }
}

/// Pulls the hammer back.
pub fn wind(
    mut commands: Commands,
    sledge_assets: Res<SledgeAssets>,
    clips: Res<Assets<AnimationClip>>,
    weapon_query: Query<&Wind, With<Sledge>>,
    mut shot_events: EventReader<ShotFired<Sledge>>,
    parent_query: Query<&Parent>,
    mut animator_query: Query<&mut AnimationPlayer>,
) {
    for ShotFired { entity: e_item, .. } in shot_events.iter() {
        for e_animator in parent_query.iter_ancestors(*e_item) {
            let Ok(mut animator) = animator_query.get_mut(e_animator) else {
                continue;
            };

            let wind = weapon_query.get(*e_item).unwrap();
            let wind_clip = clips.get(&sledge_assets.wind_animation).unwrap();
            animator
                .start_with_transition(
                    sledge_assets.wind_animation.clone(),
                    Duration::from_secs_f32(0.1),
                )
                .set_speed(wind_clip.duration() / wind.max);
            commands.entity(*e_item).insert(Winding {
                duration: wind_clip.duration(),
            });
        }
    }
}

/// At maximum charge, plays the hammer charge animation.
pub fn charge(
    mut commands: Commands,
    sledge_assets: Res<SledgeAssets>,
    weapon_query: Query<(Entity, &Wind), (With<Sledge>, With<Active>, Without<Charging>)>,
    parent_query: Query<&Parent>,
    mut animator_query: Query<&mut AnimationPlayer>,
) {
    for (e_item, wind) in weapon_query.iter() {
        if wind.progress() >= 1.0 {
            for e_animator in parent_query.iter_ancestors(e_item) {
                let Ok(mut animator) = animator_query.get_mut(e_animator) else {
                    continue;
                };

                animator
                    .start(sledge_assets.charge_animation.clone())
                    .repeat();
                commands.entity(e_item).insert(Charging);
            }
        }
    }
}

/// If `Wind.progress() >= 1.0`, begins swinging.
/// Otherwise if a `Wind` is in progress, stops it.
pub fn swing_or_cancel(
    mut commands: Commands,
    sledge_assets: Res<SledgeAssets>,
    clips: Res<Assets<AnimationClip>>,
    weapon_query: Query<
        (Entity, &Wind, &Winding, Option<&Player>),
        (With<Sledge>, Without<Active>),
    >,
    parent_query: Query<&Parent>,
    mut animator_query: Query<&mut AnimationPlayer>,
) {
    for (e_item, wind, winding, plr) in weapon_query.iter() {
        for e_animator in parent_query.iter_ancestors(e_item) {
            let Ok(mut animator) = animator_query.get_mut(e_animator) else {
                continue;
            };

            commands.entity(e_item).remove::<(Winding, Charging)>();
            let group = match plr {
                Some(..) => Group::PLAYER_PROJECTILE,
                None => Group::ENEMY_PROJECTILE,
            };
            let swing_clip = clips.get(&sledge_assets.swing_animation).unwrap();
            if wind.progress() >= 1.0 {
                animator
                    .start(sledge_assets.swing_animation.clone())
                    .set_speed(4.0);
                commands.entity(e_item).insert((
                    CollisionGroups::from_group_default(group),
                    ContactDamage,
                    Swinging {
                        duration: swing_clip.duration(),
                    },
                ));
            } else if wind.progress() > 0.0 {
                let elapsed = animator.elapsed();
                animator
                    // I almost made an issue about this, then I found a fix in this PR.
                    // which might be stale? I dunno. I'll see about taking over when I'm not lazy.
                    // https://github.com/bevyengine/bevy/pull/5912
                    .set_elapsed(-winding.duration + elapsed)
                    .set_speed(-4.0);
            }
        }
    }
}

/// At the end of a swing, returns to idle.
pub fn unswing(
    mut commands: Commands,
    sledge_assets: Res<SledgeAssets>,
    weapon_query: Query<(Entity, &Swinging), With<Sledge>>,
    parent_query: Query<&Parent>,
    mut animator_query: Query<&mut AnimationPlayer>,
) {
    for (e_item, swing) in weapon_query.iter() {
        for e_animator in parent_query.iter_ancestors(e_item) {
            let Ok(mut animator) = animator_query.get_mut(e_animator) else {
                continue;
            };

            if animator.elapsed() >= swing.duration {
                commands.entity(e_item).remove::<Swinging>();
                animator.start(sledge_assets.unswing_animation.clone());
            }
        }
    }
}
