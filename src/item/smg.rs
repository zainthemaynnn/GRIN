use crate::{
    asset::AssetLoadState, character::Player, render::sketched::NoOutline, util::vectors::Vec3Ext,
};

use super::{
    activate_on_lmb, insert_local_mouse_target, Bullet, Item, ItemSpawnEvent, Muzzle, MuzzleBundle,
    MuzzleFlashEvent, ProjectileAssets, ProjectileBundle, Sfx, WeaponBundle,
};
pub use super::{Active, Target};
use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use rand::{distributions::Uniform, Rng};

pub struct SMGPlugin;

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum SMGSystemSet {
    Deploy,
    Input,
    Fire,
    Effects,
}

impl Plugin for SMGPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<ItemSpawnEvent<SMG>>()
            .add_event::<ShotEvent>()
            .add_event::<ShotsBegan>()
            .add_event::<ShotsEnded>()
            .configure_sets(
                (
                    SMGSystemSet::Deploy.run_if(in_state(AssetLoadState::Success)),
                    SMGSystemSet::Input.run_if(in_state(AssetLoadState::Success)),
                    SMGSystemSet::Fire.run_if(in_state(AssetLoadState::Success)),
                    SMGSystemSet::Effects.run_if(in_state(AssetLoadState::Success)),
                )
                    .chain(),
            )
            .add_system(spawn.in_set(SMGSystemSet::Deploy))
            .add_systems(
                (
                    insert_local_mouse_target::<SMG>,
                    activate_on_lmb::<SMG>,
                    apply_system_buffers,
                    fire,
                )
                    .chain()
                    .in_set(SMGSystemSet::Input),
            )
            .add_system(spawn_bullet.in_set(SMGSystemSet::Fire))
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
}

pub fn spawn(
    mut commands: Commands,
    assets: Res<ProjectileAssets>,
    mut events: EventReader<<SMG as Item>::SpawnEvent>,
) {
    for event in events.iter() {
        commands
            .spawn((
                SMG::default(),
                Player,
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
            .set_parent(event.parent_entity);
    }
}

pub struct ShotEvent(Entity);

pub struct ShotsBegan(Entity);

pub struct ShotsEnded(Entity);

pub fn fire(
    mut commands: Commands,
    mut weapon_query: Query<(&mut SMG, &Target, &Active, &Children)>,
    mut shot_events: EventWriter<ShotEvent>,
    mut shots_began: EventWriter<ShotsBegan>,
    mut shots_ended: EventWriter<ShotsEnded>,
    time: Res<Time>,
) {
    for (mut item, target, active, children) in weapon_query.iter_mut() {
        for child in children.iter() {
            commands.get_or_spawn(*child).insert(*target);
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
    muzzle_query: Query<(&GlobalTransform, &Target), With<Muzzle>>,
    mut shot_events: EventReader<ShotEvent>,
    projectile_assets: Res<ProjectileAssets>,
) {
    let distr = Uniform::new_inclusive(-5.0_f32.to_radians(), 5.0_f32.to_radians());

    for ShotEvent(entity) in shot_events.iter() {
        let Ok((g_transform, target)) = muzzle_query.get(*entity) else {
            return;
        };

        let origin = g_transform.translation();
        let target = target.transform.translation;

        let fwd = (target - origin).normalize();
        let mut bullet_transform = Transform::from_translation(origin).looking_to(fwd, fwd.perp());
        bullet_transform.rotate(Quat::from_euler(
            EulerRot::XYZ,
            rand::thread_rng().sample(distr),
            rand::thread_rng().sample(distr),
            0.0,
        ));
        commands.spawn((
            Bullet,
            NoOutline,
            ProjectileBundle {
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

// TODO: streamline
pub fn play_sfx(
    sfx: Res<Sfx>,
    audio: Res<Audio>,
    mut audio_query: Query<&mut Handle<AudioSink>>,
    audio_sinks: Res<Assets<AudioSink>>,
    mut shots_began: EventReader<ShotsBegan>,
    mut shots_ended: EventReader<ShotsEnded>,
) {
    for ShotsBegan(entity) in shots_began.iter() {
        let Ok(mut audio_sink_handle) = audio_query.get_mut(*entity) else {
            return;
        };
        *audio_sink_handle = audio_sinks
            .get_handle(audio.play_with_settings(sfx.uzi.clone(), PlaybackSettings::LOOP));
    }

    for ShotsEnded(entity) in shots_ended.iter() {
        let Ok(audio_sink_handle) = audio_query.get(*entity) else {
            return;
        };
        if let Some(sfx) = audio_sinks.get(&audio_sink_handle) {
            sfx.stop();
        }
    }
}
