use crate::{
    asset::AssetLoadState,
    character::Player,
    damage::{
        projectiles::{BulletProjectile, ProjectileBundle, ProjectileColor},
        Damage, DamageVariant,
    },
    humanoid::Humanoid,
    physics::{CollisionGroupExt, CollisionGroupsExt},
};

use super::{
    aim_on_active,
    firing::{self, AutoFireBundle, FireRate, FiringPlugin, FiringType, ItemSfx, ShotFired},
    insert_on_lmb, set_local_mouse_target, unaim_on_unactive, Accuracy, AimType, IdleType, Item,
    ItemEquipEvent, ItemPlugin, ItemSet, ItemSpawnEvent, Muzzle, MuzzleBundle, ProjectileAssets,
    Sfx, WeaponBundle,
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
        app.add_plugins((
            ItemPlugin::<SMG>::default(),
            FiringPlugin::<SMG>::from(FiringType::Automatic),
        ))
        .configure_sets(
            Update,
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
        .add_systems(Update, spawn.in_set(ItemSet::Spawn))
        .add_systems(
            Update,
            (set_local_mouse_target::<SMG>, insert_on_lmb::<SMG, Active>)
                .chain()
                .in_set(SMGSystemSet::Input),
        )
        .add_systems(
            Update,
            (spawn_bullet, aim_on_active::<SMG>, unaim_on_unactive::<SMG>)
                .in_set(SMGSystemSet::Fire),
        );
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
    sfx: Res<Sfx>,
    humanoid_query: Query<&Humanoid>,
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
                WeaponBundle::default(),
                MaterialMeshBundle {
                    mesh: assets.gun.clone(),
                    material: assets.gun_material.clone(),
                    transform: Transform::from_xyz(0.0, 0.0, -0.15),
                    ..Default::default()
                },
                AutoFireBundle {
                    fire_rate: FireRate(0.1),
                    ..Default::default()
                },
                ItemSfx {
                    on_fire: sfx.uzi.clone(),
                },
                IdleType::Idle,
                AimType::RangedSingle,
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
            (-8.0 / accuracy.0).to_radians(),
            (8.0 / accuracy.0).to_radians(),
        );

        let fwd = (target - origin).normalize();
        let mut bullet_transform =
            Transform::from_translation(origin).looking_to(fwd, fwd.any_orthogonal_vector());
        bullet_transform.rotate(Quat::from_euler(
            EulerRot::YXZ,
            rand::thread_rng().sample(distr),
            0.0,
            0.0,
        ));
        commands.spawn((
            BulletProjectile,
            ProjectileBundle {
                color: ProjectileColor::Orange,
                damage: Damage {
                    ty: DamageVariant::Ballistic,
                    value: 5.0,
                    source: None,
                },
                transform: bullet_transform.with_scale(Vec3::splat(0.15)),
                velocity: Velocity::linear(bullet_transform.forward() * 64.0),
                collision_groups: CollisionGroups::from_group_default(group),
                ccd: Ccd::enabled(),
                ..Default::default()
            },
        ));
    }
}
