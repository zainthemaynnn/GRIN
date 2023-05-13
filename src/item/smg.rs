use crate::{
    asset::AssetLoadState,
    character::Player,
    collisions::{CollisionGroupExt, CollisionGroupsExt},
    damage::{Damage, DamageVariant, ProjectileBundle},
    humanoid::Humanoid,
    render::sketched::NoOutline,
    sound::{InitLocalizedSound, LocalizedSound},
    util::vectors::Vec3Ext,
};

use super::{
    aim_single, set_local_mouse_target, set_on_lmb, set_on_rmb, Aiming, Item, ItemEquipEvent,
    ItemPlugin, ItemSet, ItemSpawnEvent, Muzzle, MuzzleBundle, MuzzleFlashEvent, ProjectileAssets,
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
        app.add_event::<ShotEvent>()
            .add_event::<ShotsBegan>()
            .add_event::<ShotsEnded>()
            .add_plugin(ItemPlugin::<SMG>::default())
            .configure_sets(
                (
                    SMGSystemSet::Input.run_if(in_state(AssetLoadState::Success)),
                    SMGSystemSet::Fire.run_if(in_state(AssetLoadState::Success)),
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
                    apply_system_buffers,
                    fire,
                )
                    .chain()
                    .in_set(SMGSystemSet::Input),
            )
            .add_systems((spawn_bullet, aim_single::<SMG>).in_set(SMGSystemSet::Fire))
            .add_systems((send_muzzle_flash, play_sfx).in_set(SMGSystemSet::Effects));
    }
}

/// To add a firing target, insert a `item::Target` component.
///
/// To fire, insert the `item::Active(true)` component.
#[derive(Component)]
pub struct SMG {
    firing: bool,
    fire_rate: f32,
    cooldown: f32,
}

impl Default for SMG {
    fn default() -> Self {
        Self {
            firing: false,
            fire_rate: 0.1,
            cooldown: 0.0,
        }
    }
}

impl Item for SMG {
    type SpawnEvent = ItemSpawnEvent<SMG>;
    type EquipEvent = ItemEquipEvent<SMG>;
}

pub fn spawn(
    mut commands: Commands,
    assets: Res<ProjectileAssets>,
    mut spawn_events: EventReader<ItemSpawnEvent<SMG>>,
    mut equip_events: EventWriter<ItemEquipEvent<SMG>>,
    humanoid_query: Query<&Humanoid>,
) {
    for ItemSpawnEvent { parent_entity, .. } in spawn_events.iter() {
        let Ok(humanoid) = humanoid_query.get(*parent_entity) else {
            println!("The parent entity did not have a `Humanoid`. Only `Humanoid`s are supported for `SMG`.");
            continue;
        };

        let item_entity = commands
            .spawn((
                SMG::default(),
                Target::default(),
                Active::default(),
                Aiming::default(),
                WeaponBundle {
                    material_mesh: MaterialMeshBundle {
                        mesh: assets.gun.clone(),
                        material: assets.gun_material.clone(),
                        transform: Transform::from_xyz(0.0, 0.0, -0.15),
                        ..Default::default()
                    },
                    ..Default::default()
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

        equip_events.send(ItemEquipEvent::<SMG>::new(*parent_entity, item_entity));
    }
}

pub struct ShotEvent(Entity);

pub struct ShotsBegan(Entity);

pub struct ShotsEnded(Entity);

pub fn fire(
    mut commands: Commands,
    mut weapon_query: Query<(&mut SMG, &Target, &Active, Option<&Player>, &Children)>,
    mut shot_events: EventWriter<ShotEvent>,
    mut shots_began: EventWriter<ShotsBegan>,
    mut shots_ended: EventWriter<ShotsEnded>,
    time: Res<Time>,
) {
    for (mut item, target, active, player, children) in weapon_query.iter_mut() {
        for child in children.iter() {
            let mut e = commands.get_or_spawn(*child);
            e.insert(*target);
            if let Some(player) = player {
                e.insert(*player);
            }
        }
        if item.cooldown < item.fire_rate {
            item.cooldown += time.delta_seconds();
        }

        if active.0 {
            if item.cooldown >= item.fire_rate {
                if !item.firing {
                    item.firing = true;
                    shots_began.send_batch(children.iter().map(|e| ShotsBegan(*e)));
                }

                // need to use loops to account for frame lag
                // (might need to fire multiple at once if it spiked)
                while item.cooldown >= item.fire_rate {
                    item.cooldown -= item.fire_rate;
                    shot_events.send_batch(children.iter().map(|e| ShotEvent(*e)));
                }
            }
        } else {
            if item.firing {
                item.firing = false;
                shots_ended.send_batch(children.iter().map(|e| ShotsEnded(*e)));
            }
        }
    }
}

pub fn spawn_bullet(
    mut commands: Commands,
    muzzle_query: Query<(&GlobalTransform, &Target, Option<&Player>), With<Muzzle>>,
    mut shot_events: EventReader<ShotEvent>,
    projectile_assets: Res<ProjectileAssets>,
) {
    let distr = Uniform::new_inclusive(-5.0_f32.to_radians(), 5.0_f32.to_radians());

    for ShotEvent(entity) in shot_events.iter() {
        let Ok((g_transform, target, plr)) = muzzle_query.get(*entity) else {
            return;
        };

        let origin = g_transform.translation();
        let target = target.transform.translation;
        let group = match plr {
            Some(_) => Group::PLAYER_PROJECTILE,
            None => Group::ENEMY_PROJECTILE,
        };

        let fwd = (target - origin).normalize();
        let mut bullet_transform = Transform::from_translation(origin).looking_to(fwd, fwd.perp());
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

pub fn send_muzzle_flash(
    mut shot_events: EventReader<ShotEvent>,
    mut muzzle_flash_events: EventWriter<MuzzleFlashEvent>,
) {
    shot_events
        .iter()
        .for_each(|ShotEvent(e)| muzzle_flash_events.send(MuzzleFlashEvent(*e)));
}

pub fn play_sfx(
    mut commands: Commands,
    sfx: Res<Sfx>,
    audio_query: Query<&mut LocalizedSound>,
    audio_sinks: Res<Assets<SpatialAudioSink>>,
    mut shots_began: EventReader<ShotsBegan>,
    mut shots_ended: EventReader<ShotsEnded>,
) {
    for ShotsBegan(entity) in shots_began.iter() {
        if let Ok(sound) = audio_query.get(*entity) {
            if let Some(sound_sink) = audio_sinks.get(&sound.0) {
                sound_sink.stop();
            }
        }

        commands
            .get_or_spawn(*entity)
            .insert(InitLocalizedSound(sfx.uzi.clone(), PlaybackSettings::LOOP));
    }

    for ShotsEnded(entity) in shots_ended.iter() {
        let Ok(sound) = audio_query.get(*entity) else {
            return;
        };
        if let Some(sound_sink) = audio_sinks.get(&sound.0) {
            sound_sink.stop();
        }
    }
}
