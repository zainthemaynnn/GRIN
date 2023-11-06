use std::time::Duration;

use bevy::prelude::*;
use bevy_asset_loader::prelude::{AssetCollection, LoadingStateAppExt};
use bevy_rapier3d::prelude::*;
use grin_asset::AssetLoadState;
use grin_damage::{impact::Impact, ContactDamage, Damage, DamageEvent, DamageVariant};
use grin_physics::{collider, CollisionGroupExt, CollisionGroupsExt};
use grin_render::sketched::SketchMaterial;
use grin_rig::humanoid::Humanoid;
use grin_util::event::Spawnable;

use crate::{find_item_owner, Equipped};

use super::{
    firing::{self, FireRate, FiringPlugin, FiringType, SemiFireBundle, ShotFired},
    insert_on_lmb,
    melee::{update_hammer_winds, Charging, Swinging, Wind, Winding},
    Active, Item, ItemEquipEvent, ItemPlugin, ItemSet, ItemSpawnEvent, WeaponBundle,
};

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
        app.add_plugins((
            ItemPlugin::<Sledge>::default(),
            FiringPlugin::<Sledge>::from(FiringType::SemiAutomatic),
        ))
        .add_collection_to_loading_state::<_, SledgeAssets>(AssetLoadState::Loading)
        .configure_sets(
            Update,
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
        .add_systems(
            Update,
            (
                spawn.in_set(ItemSet::Spawn),
                (
                    insert_on_lmb::<Sledge, Active>,
                    apply_deferred,
                    wind,
                    charge,
                    swing_or_cancel,
                    unswing,
                    update_hammer_winds,
                )
                    .chain()
                    .in_set(SledgeSystemSet::Input),
                sledge_on_hit.in_set(SledgeSystemSet::Effects),
            ),
        );
    }
}

#[derive(Component, Clone, Default)]
pub struct Sledge;

impl Spawnable for Sledge {
    type Event = ItemSpawnEvent<Sledge>;
}

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
                RigidBody::Dynamic,
                collider!(meshes, &assets.sledge),
                CollisionGroups::from_group_default(Group::PLAYER_PROJECTILE),
                Ccd::enabled(),
                ActiveEvents::COLLISION_EVENTS,
                ActiveHooks::FILTER_CONTACT_PAIRS,
                ColliderMassProperties::default(),
                GravityScale(0.0),
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
    item_query: Query<&Wind, With<Sledge>>,
    mut shot_events: EventReader<ShotFired<Sledge>>,
    parent_query: Query<&Parent>,
    mut animator_query: Query<&mut AnimationPlayer>,
) {
    for ShotFired { entity: e_item, .. } in shot_events.iter() {
        for e_animator in parent_query.iter_ancestors(*e_item) {
            let Ok(mut animator) = animator_query.get_mut(e_animator) else {
                continue;
            };

            let wind = item_query.get(*e_item).unwrap();
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
    item_query: Query<(Entity, &Wind), (With<Sledge>, With<Active>, Without<Charging>)>,
    parent_query: Query<&Parent>,
    mut animator_query: Query<&mut AnimationPlayer>,
) {
    for (e_item, wind) in item_query.iter() {
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
    item_query: Query<(Entity, &Wind, &Winding), (With<Sledge>, Without<Active>)>,
    parent_query: Query<&Parent, Without<Equipped>>,
    parent_query_eq: Query<&Parent, With<Equipped>>,
    mut animator_query: Query<&mut AnimationPlayer>,
) {
    for (e_item, wind, winding) in item_query.iter() {
        for e_animator in parent_query.iter_ancestors(e_item) {
            let Ok(mut animator) = animator_query.get_mut(e_animator) else {
                continue;
            };

            commands.entity(e_item).remove::<(Winding, Charging)>();
            let swing_clip = clips.get(&sledge_assets.swing_animation).unwrap();
            if wind.progress() >= 1.0 {
                animator
                    .start(sledge_assets.swing_animation.clone())
                    .set_speed(4.0);
                commands.entity(e_item).insert((
                    ContactDamage::Once,
                    Damage {
                        ty: DamageVariant::Ballistic,
                        value: 20.0,
                        source: find_item_owner(e_item, &parent_query_eq),
                    },
                    Swinging {
                        duration: swing_clip.duration() / 4.0,
                    },
                ));
            } else if wind.progress() > 0.0 {
                let elapsed = animator.elapsed();
                animator
                    // I almost made an issue about this, then I found a fix in this PR.
                    // which might be stale? I dunno. I'll see about taking over when I'm not lazy.
                    // https://github.com/bevyengine/bevy/pull/5912
                    // update: this got fixed in 0.12. I'll keep it as a piece of history.
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
    item_query: Query<(Entity, &Swinging), With<Sledge>>,
    parent_query: Query<&Parent>,
    mut animator_query: Query<&mut AnimationPlayer>,
) {
    for (e_item, swing) in item_query.iter() {
        for e_animator in parent_query.iter_ancestors(e_item) {
            let Ok(mut animator) = animator_query.get_mut(e_animator) else {
                continue;
            };

            if animator.elapsed() >= swing.duration {
                commands
                    .entity(e_item)
                    .remove::<(Swinging, ContactDamage)>();
                animator
                    .start(sledge_assets.unswing_animation.clone())
                    .set_speed(2.0);
            }
        }
    }
}

pub fn sledge_on_hit(
    mut commands: Commands,
    rapier_context: Res<RapierContext>,
    item_query: Query<&GlobalTransform, With<Sledge>>,
    mut damage_events: EventReader<DamageEvent>,
) {
    for damage_event in damage_events.iter() {
        let DamageEvent::Contact { e_damage, e_hit, .. } = damage_event else {
            continue;
        };
        let Ok(g_item_transform) = item_query.get(*e_damage) else {
            continue;
        };
        let Some(contact_pair) = rapier_context.contact_pair(*e_hit, *e_damage) else {
            continue;
        };
        let Some(contact) = contact_pair.find_deepest_contact() else {
            continue;
        };
        let contact_point = g_item_transform.transform_point(contact.1.local_p1());
        commands.spawn((
            TransformBundle::from_transform(Transform::from_translation(contact_point)),
            Impact::from_burst_radius(2.0),
        ));
    }
}
