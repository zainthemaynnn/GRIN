use std::marker::PhantomData;

use bevy::prelude::*;

use super::{Active, MuzzleFlashEvent};

#[derive(Default)]
pub struct FiringPlugin<T: Component> {
    pub ty: FiringType,
    pub phantom_data: PhantomData<T>,
}

#[derive(Default)]
pub enum FiringType {
    #[default]
    SemiAutomatic,
    Automatic,
}

impl<T: Component> Plugin for FiringPlugin<T> {
    fn build(&self, app: &mut App) {
        app.add_event::<ShotFired<T>>()
            .add_systems(Update, (send_muzzle_flash::<T>, update_cooldown::<T>));

        match self.ty {
            FiringType::SemiAutomatic => {
                app.add_systems(
                    Update,
                    (
                        semi_fire::<T>.after(update_cooldown::<T>),
                        play_sfx_discrete::<T>,
                    ),
                );
            }
            FiringType::Automatic => {
                app.add_event::<ShotsBegan<T>>()
                    .add_event::<ShotsEnded<T>>()
                    .add_systems(
                        Update,
                        (
                            auto_fire::<T>.after(update_cooldown::<T>),
                            play_sfx_continuous::<T>,
                        ),
                    );
            }
        }
    }
}

impl<T: Component> From<FiringType> for FiringPlugin<T> {
    fn from(firing_type: FiringType) -> Self {
        Self {
            ty: firing_type,
            phantom_data: PhantomData::default(),
        }
    }
}

#[derive(Bundle, Default)]
pub struct SemiFireBundle {
    pub fire_rate: FireRate,
    pub cooldown: Cooldown,
}

#[derive(Bundle, Default)]
pub struct AutoFireBundle {
    pub fire_rate: FireRate,
    pub cooldown: Cooldown,
    pub firing: AutoFire,
}

#[derive(Component)]
pub struct FireRate(pub f32);

impl Default for FireRate {
    fn default() -> Self {
        Self(1.0)
    }
}

#[derive(Component, Default)]
pub struct Cooldown(pub f32);

#[derive(Component, Default)]
pub struct AutoFire(pub bool);

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

pub fn update_cooldown<T: Component>(
    time: Res<Time>,
    mut query: Query<(&mut Cooldown, &FireRate), With<T>>,
) {
    for (mut cooldown, fire_rate) in query.iter_mut() {
        if cooldown.0 < fire_rate.0 {
            cooldown.0 += time.delta_seconds();
        }
    }
}

pub fn semi_fire<T: Component>(
    mut query: Query<(Entity, &mut Cooldown, &FireRate, Option<Ref<Active>>), With<T>>,
    mut shot_events: EventWriter<ShotFired<T>>,
) {
    for (entity, mut cooldown, fire_rate, active) in query.iter_mut() {
        if let Some(active) = active {
            if active.is_added() && cooldown.0 >= fire_rate.0 {
                shot_events.send(ShotFired {
                    entity,
                    phantom_data: PhantomData::default(),
                });
                cooldown.0 = 0.0;
            }
        }
    }
}

pub fn auto_fire<T: Component>(
    mut query: Query<
        (
            Entity,
            &mut Cooldown,
            &FireRate,
            Option<&Active>,
            &mut AutoFire,
        ),
        With<T>,
    >,
    mut shot_events: EventWriter<ShotFired<T>>,
    mut shots_began: EventWriter<ShotsBegan<T>>,
    mut shots_ended: EventWriter<ShotsEnded<T>>,
) {
    for (entity, mut cooldown, fire_rate, active, mut firing) in query.iter_mut() {
        if active.is_some() {
            if cooldown.0 >= fire_rate.0 {
                if !firing.0 {
                    firing.0 = true;
                    shots_began.send(ShotsBegan {
                        entity,
                        phantom_data: PhantomData::default(),
                    });
                }

                // need to use loops to account for frame lag
                // (might need to fire multiple at once if it spiked)
                while cooldown.0 >= fire_rate.0 {
                    cooldown.0 -= fire_rate.0;
                    shot_events.send(ShotFired {
                        entity,
                        phantom_data: PhantomData::default(),
                    });
                }
            }
        } else {
            if firing.0 {
                firing.0 = false;
                shots_ended.send(ShotsEnded {
                    entity,
                    phantom_data: PhantomData::default(),
                });
            }
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
    audio_query: Query<&ItemSfx, With<T>>,
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
    sfx_query: Query<&ItemSfx, With<T>>,
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

        commands
            .get_or_spawn(*entity)
            .insert(AudioBundle {
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
        commands
            .get_or_spawn(*entity)
            .remove::<AudioBundle>();
    }
}
