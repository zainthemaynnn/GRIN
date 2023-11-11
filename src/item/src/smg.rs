use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use grin_asset::AssetLoadState;
use grin_damage::{
    impact::Impact,
    projectiles::{BulletProjectile, ProjectileBundle, ProjectileColor},
    Damage, DamageVariant,
};
use grin_rig::humanoid::Humanoid;
use grin_util::event::Spawnable;
use rand::{distributions::Uniform, Rng};

use crate::{
    aim_on_active, find_item_owner,
    firing::{self, AutoFireBundle, FireRate, FiringPlugin, FiringType, ItemSfx, ShotFired},
    insert_on_lmb, on_hit_render_impact, set_local_mouse_target,
    unaim_on_unactive, Accuracy, Active, AimType, DamageCollisionGroups, Equipped, IdleType, Item,
    ItemEquipEvent, ItemPlugin, ItemSet, ItemSpawnEvent, Muzzle, MuzzleBundle, ProjectileAssets,
    Sfx, Target, WeaponBundle,
};

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
        .add_systems(
            Update,
            (
                spawn.in_set(ItemSet::Spawn),
                (insert_on_lmb::<SMG, Active>, set_local_mouse_target::<SMG>)
                    .chain()
                    .in_set(SMGSystemSet::Input),
                (spawn_bullet, aim_on_active::<SMG>, unaim_on_unactive::<SMG>)
                    .in_set(SMGSystemSet::Fire),
                (|| Impact::from_burst_radius(2.0))
                    .pipe(on_hit_render_impact::<SMG>)
                    .in_set(SMGSystemSet::Effects),
            ),
        );
    }
}

/// To add a firing target, insert a `item::Target` component.
///
/// To fire, set the `item::Active(true)` component.
#[derive(Component, Clone, Default)]
pub struct SMG;

impl Spawnable for SMG {
    type Event = ItemSpawnEvent<SMG>;
}

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
    for ItemSpawnEvent { parent_entity, .. } in spawn_events.read() {
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

#[derive(Component)]
pub struct SMGShot;

pub fn spawn_bullet(
    mut commands: Commands,
    item_query: Query<(&Target, &Accuracy, &DamageCollisionGroups, &Children), With<SMG>>,
    parent_query: Query<&Parent, With<Equipped>>,
    muzzle_query: Query<&GlobalTransform, With<Muzzle>>,
    mut shot_events: EventReader<ShotFired<SMG>>,
) {
    for ShotFired { entity: e_item, .. } in shot_events.read() {
        let (target, accuracy, damage_collision_groups, children) =
            item_query.get(*e_item).unwrap();
        let muzzle_g_transform = muzzle_query.get(*children.first().unwrap()).unwrap();

        let origin = muzzle_g_transform.translation();
        let target = target.transform.translation;
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
            SMGShot,
            BulletProjectile,
            ProjectileBundle {
                color: ProjectileColor::Orange,
                damage: Damage {
                    ty: DamageVariant::Ballistic,
                    value: 5.0,
                    source: find_item_owner(*e_item, &parent_query),
                },
                transform: bullet_transform.with_scale(Vec3::splat(0.15)),
                velocity: Velocity::linear(bullet_transform.forward() * 64.0),
                ccd: Ccd::enabled(),
                collision_groups: damage_collision_groups.into(),
                ..Default::default()
            },
        ));
    }
}
