use crate::{
    asset::AssetLoadState,
    character::Player,
    collisions::{CollisionGroupExt, CollisionGroupsExt},
    damage::{Damage, DamageVariant, ProjectileBundle},
    humanoid::Humanoid,
    render::sketched::NoOutline,
};

use super::{
    aim_single,
    firing::{self, AutoFireBundle, FireRate, FiringPlugin, FiringType, ItemSfx, ShotFired},
    set_local_mouse_target, set_on_lmb, set_on_rmb, Accuracy, Aiming, Item, ItemEquipEvent,
    ItemPlugin, ItemSet, ItemSpawnEvent, Muzzle, MuzzleBundle, ProjectileAssets, Sfx, WeaponBundle,
};
pub use super::{Active, Target};
use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use rand::{distributions::Uniform, Rng};

pub struct SMGPlugin;

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum SMGSystemSet {
    Input,
    Fire,
    Effects,
}

impl Plugin for SMGPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(ItemPlugin::<SMG>::default())
            .add_plugin(FiringPlugin::<SMG>::from(FiringType::Automatic))
            .configure_sets(
                (
                    SMGSystemSet::Input
                        .run_if(in_state(AssetLoadState::Success))
                        .before(firing::auto_fire::<SMG>),
                    SMGSystemSet::Fire
                        .run_if(in_state(AssetLoadState::Success))
                        .after(firing::auto_fire::<SMG>),
                    SMGSystemSet::Effects.run_if(in_state(AssetLoadState::Success)),
                )
                    .chain(),
            )
            .add_system(spawn.in_set(ItemSet::Spawn))
            .add_systems(
                (
                    set_local_mouse_target::<SMG>,
                    set_on_lmb::<SMG, Active>,
                    set_on_rmb::<SMG, Aiming>,
                )
                    .chain()
                    .in_set(SMGSystemSet::Input),
            )
            .add_systems((spawn_bullet, aim_single::<SMG>).in_set(SMGSystemSet::Fire));
    }
}

/// To add a firing target, insert a `item::Target` component.
///
/// To fire, set the `item::Active(true)` component.
#[derive(Component, Default)]
pub struct SMG;

impl Item for SMG {
    type SpawnEvent = ItemSpawnEvent<SMG>;
    type EquipEvent = ItemEquipEvent<SMG>;
}

pub fn spawn(
    mut commands: Commands,
    assets: Res<ProjectileAssets>,
    humanoid_query: Query<&Humanoid>,
    sfx: Res<Sfx>,
    mut spawn_events: EventReader<ItemSpawnEvent<SMG>>,
    mut equip_events: EventWriter<ItemEquipEvent<SMG>>,
) {
    for ItemSpawnEvent { parent_entity, .. } in spawn_events.iter() {
        let Ok(humanoid) = humanoid_query.get(*parent_entity) else {
            println!("The parent entity did not have a `Humanoid`. Only `Humanoid`s are supported for `SMG`.");
            continue;
        };

        let item_entity = commands
            .spawn((
                SMG::default(),
                WeaponBundle {
                    material_mesh: MaterialMeshBundle {
                        mesh: assets.gun.clone(),
                        material: assets.gun_material.clone(),
                        transform: Transform::from_xyz(0.0, 0.0, -0.15),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                AutoFireBundle {
                    fire_rate: FireRate(0.1),
                    ..Default::default()
                },
                ItemSfx {
                    on_fire: sfx.uzi.clone(),
                },
            ))
            .with_children(|parent| {
                parent.spawn(MuzzleBundle {
                    transform: Transform::from_xyz(0.0, 0.0, -0.15),
                    ..Default::default()
                });
            })
            .set_parent(humanoid.dominant_hand())
            .id();

        equip_events.send(ItemEquipEvent::new(*parent_entity, item_entity));
    }
}

pub fn spawn_bullet(
    mut commands: Commands,
    weapon_query: Query<(&Target, &Accuracy, &Children, Option<&Player>), With<SMG>>,
    muzzle_query: Query<&GlobalTransform, With<Muzzle>>,
    mut shot_events: EventReader<ShotFired<SMG>>,
    projectile_assets: Res<ProjectileAssets>,
) {
    for ShotFired { entity, .. } in shot_events.iter() {
        let (target, accuracy, children, plr) = weapon_query.get(*entity).unwrap();
        let muzzle_g_transform = muzzle_query.get(*children.first().unwrap()).unwrap();

        let origin = muzzle_g_transform.translation();
        let target = target.transform.translation;
        let group = match plr {
            Some(..) => Group::PLAYER_PROJECTILE,
            None => Group::ENEMY_PROJECTILE,
        };
        let distr = Uniform::new_inclusive(
            (-5.0 / accuracy.0).to_radians(),
            (5.0 / accuracy.0).to_radians(),
        );

        let fwd = (target - origin).normalize();
        let mut bullet_transform =
            Transform::from_translation(origin).looking_to(fwd, fwd.any_orthogonal_vector());
        bullet_transform.rotate(Quat::from_euler(
            EulerRot::XYZ,
            rand::thread_rng().sample(distr),
            rand::thread_rng().sample(distr),
            0.0,
        ));
        commands.spawn((
            ProjectileBundle {
                damage: Damage {
                    ty: DamageVariant::Ballistic,
                    value: 5.0,
                    source: None,
                },
                collision_groups: CollisionGroups::from_group_default(group),
                material_mesh: MaterialMeshBundle {
                    mesh: projectile_assets.bullet_5cm.clone(),
                    material: projectile_assets.bullet_material.clone(),
                    transform: bullet_transform,
                    ..Default::default()
                },
                collider: Collider::ball(0.05),
                velocity: Velocity::linear(bullet_transform.forward() * 100.0),
                ..Default::default()
            },
            // you know you're new to making games
            // when you spend an hour realizing this isn't enabled already
            Ccd::enabled(),
            NoOutline,
        ));
    }
}
