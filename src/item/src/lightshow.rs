use std::time::Duration;

use grin_asset::AssetLoadState,
use grin_damage::{
        projectiles::{BulletProjectile, ProjectileBundle, ProjectileColor},
        Damage, DamageVariant,
    };
use grin_physics::{CollisionGroupExt, CollisionGroupsExt};
use grin_rig::humanoid::Humanoid;
use grin_render::{
    duoquad::{render_duoquads, DuoQuad, DuoQuadBundle, DuoQuadRadiusLens},
    sketched::{NoOutline, SketchMaterial},
};
use grin_util::{color::rand_spectrum, tween::TweenCompletedEvent};

use super::{
    aim_single,
    firing::{self, AutoFireBundle, FireRate, FiringPlugin, FiringType, ItemSfx, ShotFired},
    insert_on_lmb, insert_on_rmb, set_local_mouse_target, Accuracy, Aiming, Item, ItemEquipEvent,
    ItemPlugin, ItemSet, ItemSpawnEvent, Muzzle, MuzzleBundle, ProjectileAssets, Sfx, WeaponBundle,
};
pub use super::{Active, Target};
use bevy::{pbr::NotShadowCaster, prelude::*};
use bevy_rapier3d::prelude::*;
use bevy_tweening::{Animator, EaseFunction, Tween};
use rand::{distributions::Uniform, Rng};

pub struct LightshowPlugin;

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum LightshowSystemSet {
    Input,
    Fire,
    Effects,
}

impl Plugin for LightshowPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ItemPlugin::<Lightshow>::default(),
            FiringPlugin::<Lightshow>::from(FiringType::Automatic),
        ))
            .configure_sets(
                Update,
                (
                    LightshowSystemSet::Input
                        .run_if(in_state(AssetLoadState::Success))
                        .before(firing::auto_fire::<Lightshow>),
                    LightshowSystemSet::Fire
                        .run_if(in_state(AssetLoadState::Success))
                        .after(firing::auto_fire::<Lightshow>),
                    LightshowSystemSet::Effects.run_if(in_state(AssetLoadState::Success)),
                )
                    .chain(),
            )
            .add_systems(Update, spawn.in_set(ItemSet::Spawn))
            .add_systems(
                Update,
                (
                    set_local_mouse_target::<Lightshow>,
                    insert_on_lmb::<Lightshow, Active>,
                    insert_on_rmb::<Lightshow, Aiming>,
                    align_lasers.before(render_duoquads),
                )
                    .chain()
                    .in_set(LightshowSystemSet::Input),
            )
            .add_systems(
                Update,
                (spawn_bullet, aim_single::<Lightshow>).in_set(LightshowSystemSet::Fire),
            );
    }
}

/// To add a firing target, insert a `item::Target` component.
///
/// To fire, set the `item::Active(true)` component.
#[derive(Component, Default)]
pub struct Lightshow;

impl Item for Lightshow {
    type SpawnEvent = ItemSpawnEvent<Lightshow>;
    type EquipEvent = ItemEquipEvent<Lightshow>;
}

pub fn spawn(
    mut commands: Commands,
    assets: Res<ProjectileAssets>,
    humanoid_query: Query<&Humanoid>,
    sfx: Res<Sfx>,
    mut spawn_events: EventReader<ItemSpawnEvent<Lightshow>>,
    mut equip_events: EventWriter<ItemEquipEvent<Lightshow>>,
) {
    for ItemSpawnEvent { parent_entity, .. } in spawn_events.read() {
        let humanoid = humanoid_query.get(*parent_entity).unwrap();

        let item_entity = commands
            .spawn((
                Lightshow::default(),
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
                /*ItemSfx {
                    on_fire: sfx.uzi.clone(),
                },*/
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
    weapon_query: Query<(&Target, &Accuracy, &Children, Option<&Player>), With<Lightshow>>,
    muzzle_query: Query<&GlobalTransform, With<Muzzle>>,
    mut shot_events: EventReader<ShotFired<Lightshow>>,
    projectile_assets: Res<ProjectileAssets>,
    mut materials: ResMut<Assets<SketchMaterial>>,
    fallback: Res<FallbackImage>,
) {
    for ShotFired { entity, .. } in shot_events.read() {
        let (target, accuracy, children, plr) = weapon_query.get(*entity).unwrap();
        let muzzle_g_transform = muzzle_query.get(*children.first().unwrap()).unwrap();

        let origin = muzzle_g_transform.translation();
        let target = target.transform.translation;
        let group = match plr {
            Some(..) => Group::PLAYER_PROJECTILE,
            None => Group::ENEMY_PROJECTILE,
        };

        let mut rng = rand::thread_rng();
        let distr = Uniform::new_inclusive(
            (-0.5 / accuracy.0).to_radians(),
            (0.5 / accuracy.0).to_radians(),
        );
        let dir = Quat::from_euler(
            EulerRot::XYZ,
            rand::thread_rng().sample(distr),
            rand::thread_rng().sample(distr),
            0.0,
        )
        .mul_vec3(target - origin);

        commands.spawn((
            DuoQuadBundle {
                duoquad: DuoQuad {
                    origin,
                    target: origin + dir,
                    radius: 0.2,
                },
                material: materials.add(SketchMaterial {
                    base_color: rand_spectrum(&mut rng),
                    base_color_texture: Some(fallback.texture.clone()),
                    unlit: true,
                    double_sided: true,
                    cull_mode: None,
                    ..Default::default()
                }),
                ..Default::default()
            },
            Animator::new(
                Tween::new(
                    EaseFunction::QuadraticOut,
                    Duration::from_secs(1),
                    DuoQuadRadiusLens {
                        start: 0.2,
                        end: 0.0,
                    },
                )
                .with_completed_event(TweenCompletedEvent::Despawn as u64),
            ),
            NotShadowCaster,
        ));
    }
}

pub fn align_lasers(
    muzzle_query: Query<&GlobalTransform, With<Muzzle>>,
    mut duoquad_query: Query<&mut DuoQuad>,
) {
    let Ok(transform) = muzzle_query.get_single() else {
        return;
    };
    for mut duoquad in duoquad_query.iter_mut() {
        duoquad.origin = transform.translation();
    }
}
