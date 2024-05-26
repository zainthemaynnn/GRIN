use std::{marker::PhantomData, time::Duration};

use bevy::{prelude::*, utils::HashSet};
use bevy_enum_filter::{Enum, EnumFilter};
use grin_physics::PhysicsTime;
use grin_time::scaling::TimeScale;

use super::fx::MuzzleFlashEvent;

/// Commonly used for AI or weapon targetting.
#[derive(Component, Debug, Copy, Clone)]
pub struct Target {
    pub transform: Transform,
    pub distance: f32,
}

impl Default for Target {
    fn default() -> Self {
        Self {
            transform: Transform::default(),
            distance: std::f32::MAX,
        }
    }
}

impl Target {
    pub fn from_pair(origin: Vec3, target: Vec3) -> Self {
        Self {
            transform: Transform::from_translation(target),
            distance: target.distance(origin),
        }
    }
}

/// Whether the item is being "used."
#[derive(Component, Debug, Copy, Clone, Eq, PartialEq, Default)]
#[component(storage = "SparseSet")]
pub struct Active;

/// For most items, affects the accuracy of projectiles in different ways. A higher number is better.
///
/// `1.0` is the default. Can't go below zero.
#[derive(Component, Debug, Copy, Clone, PartialEq)]
pub struct Accuracy(pub f32);

impl Default for Accuracy {
    fn default() -> Self {
        Self(1.0)
    }
}

impl From<f32> for Accuracy {
    fn from(value: f32) -> Self {
        Self(value)
    }
}

#[derive(Component, Debug, Default, Hash, Eq, PartialEq)]
pub enum FiringBehavior {
    #[default]
    SemiAutomatic,
    Automatic,
}

#[derive(Default)]
pub struct FiringPlugin<T: Component> {
    pub supported_modes: HashSet<FiringBehavior>,
    pub phantom_data: PhantomData<T>,
}

impl<T: Component> Plugin for FiringPlugin<T> {
    fn build(&self, app: &mut App) {
        app.add_event::<ShotFired<T>>()
            .add_systems(Update, (send_muzzle_flash::<T>, step_cooldowns::<T>));

        for ty in &self.supported_modes {
            match ty {
                FiringBehavior::SemiAutomatic => {
                    app.add_systems(
                        Update,
                        (
                            semi_fire::<T>.after(step_cooldowns::<T>),
                            play_sfx_discrete::<T>,
                        ),
                    );
                }
                FiringBehavior::Automatic => {
                    app.add_event::<ShotsBegan<T>>()
                        .add_event::<ShotsEnded<T>>()
                        .add_systems(
                            Update,
                            (
                                auto_fire::<T>.after(step_cooldowns::<T>),
                                play_sfx_continuous::<T>,
                            ),
                        );
                }
            }
        }
    }
}

impl<T: Component> From<HashSet<FiringBehavior>> for FiringPlugin<T> {
    fn from(supported_modes: HashSet<FiringBehavior>) -> Self {
        Self {
            supported_modes,
            phantom_data: PhantomData::default(),
        }
    }
}

#[derive(Component)]
pub struct FireRate(pub Duration);

impl Default for FireRate {
    fn default() -> Self {
        Self(Duration::from_millis(1000))
    }
}

#[derive(Component, Default)]
pub struct ShotCooldown(pub Duration);

impl ShotCooldown {
    pub fn ready(&self) -> bool {
        self.0.is_zero()
    }

    pub fn step(&mut self, dt: Duration) {
        self.0 = self.0.saturating_sub(dt);
    }

    pub fn reset(&mut self, duration: Duration) {
        self.0 = duration
    }
}

#[derive(Component)]
pub struct ItemSfx {
    pub on_fire: Handle<AudioSource>,
}

#[derive(Event)]
pub struct ShotFired<T: Component> {
    pub entity: Entity,
    pub phantom_data: PhantomData<T>,
}

#[derive(Event)]
pub struct ShotsBegan<T: Component> {
    pub entity: Entity,
    pub phantom_data: PhantomData<T>,
}

#[derive(Event)]
pub struct ShotsEnded<T: Component> {
    pub entity: Entity,
    pub phantom_data: PhantomData<T>,
}

#[derive(Component, EnumFilter, Debug, Default)]
pub enum FiringMode {
    #[default]
    SemiAuto,
    Auto {
        firing: bool,
    },
}

pub fn step_cooldowns<T: Component>(
    time: Res<PhysicsTime>,
    mut query: Query<(&mut ShotCooldown, &TimeScale), With<T>>,
) {
    for (mut cooldown, time_scale) in query.iter_mut() {
        cooldown.step(time.0.delta().mul_f32(f32::from(time_scale)));
    }
}

pub fn semi_fire<T: Component>(
    mut query: Query<
        (Entity, &mut ShotCooldown, &FireRate),
        (With<T>, With<Enum!(FiringMode::SemiAuto)>, Added<Active>),
    >,
    mut shot_events: EventWriter<ShotFired<T>>,
) {
    for (entity, mut cooldown, fire_rate) in query.iter_mut() {
        if cooldown.ready() {
            shot_events.send(ShotFired {
                entity,
                phantom_data: PhantomData::default(),
            });
            cooldown.reset(fire_rate.0);
        }
    }
}

pub fn auto_fire<T: Component>(
    mut query: Query<
        (
            Entity,
            &mut ShotCooldown,
            &FireRate,
            Option<&Active>,
            &mut FiringMode,
        ),
        (With<T>, With<Enum!(FiringMode::Auto)>),
    >,
    mut shot_events: EventWriter<ShotFired<T>>,
    mut shots_began: EventWriter<ShotsBegan<T>>,
    mut shots_ended: EventWriter<ShotsEnded<T>>,
) {
    for (entity, mut cooldown, fire_rate, active, mut firing_mode) in query.iter_mut() {
        let FiringMode::Auto { ref mut firing } = *firing_mode else {
            continue;
        };

        if active.is_some() && !*firing && cooldown.ready() {
            *firing = true;
            shots_began.send(ShotsBegan {
                entity,
                phantom_data: PhantomData::default(),
            });
        } else if active.is_none() && *firing {
            *firing = false;
            shots_ended.send(ShotsEnded {
                entity,
                phantom_data: PhantomData::default(),
            });
        }

        if *firing && cooldown.ready() {
            shot_events.send(ShotFired {
                entity,
                phantom_data: PhantomData::default(),
            });
            cooldown.reset(fire_rate.0);
        }
    }
}

pub fn send_muzzle_flash<T: Component>(
    mut shot_fired: EventReader<ShotFired<T>>,
    mut muzzle_flash_events: EventWriter<MuzzleFlashEvent>,
) {
    shot_fired
        .read()
        .for_each(|ShotFired { entity, .. }| muzzle_flash_events.send(MuzzleFlashEvent(*entity)));
}

pub fn play_sfx_discrete<T: Component>(
    mut commands: Commands,
    audio_query: Query<&ItemSfx, (With<T>, With<Enum!(FiringMode::SemiAuto)>)>,
    mut shot_fired: EventReader<ShotFired<T>>,
) {
    for ShotFired { entity, .. } in shot_fired.read() {
        let Ok(ItemSfx { on_fire }) = audio_query.get(*entity) else {
            continue;
        };

        commands.get_or_spawn(*entity).with_children(|parent| {
            parent.spawn(AudioBundle {
                source: on_fire.clone(),
                settings: PlaybackSettings::default().with_spatial(true),
                ..Default::default()
            });
        });
    }
}

pub fn play_sfx_continuous<T: Component>(
    mut commands: Commands,
    sfx_query: Query<&ItemSfx, (With<T>, With<Enum!(FiringMode::Auto)>)>,
    sink_query: Query<&mut SpatialAudioSink>,
    mut shots_began: EventReader<ShotsBegan<T>>,
    mut shots_ended: EventReader<ShotsEnded<T>>,
) {
    for ShotsBegan { entity, .. } in shots_began.read() {
        let Ok(ItemSfx { on_fire }) = sfx_query.get(*entity) else {
            continue;
        };

        if let Ok(sound) = sink_query.get(*entity) {
            sound.stop();
        }

        commands.get_or_spawn(*entity).insert(AudioBundle {
            source: on_fire.clone(),
            settings: PlaybackSettings::LOOP,
            ..Default::default()
        });
    }

    for ShotsEnded { entity, .. } in shots_ended.read() {
        if let Ok(sound) = sink_query.get(*entity) {
            sound.stop();
            commands.get_or_spawn(*entity).remove::<SpatialAudioSink>();
        }
        commands.get_or_spawn(*entity).remove::<AudioBundle>();
    }
}
