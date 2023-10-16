//! This was supposed to make audio simpler.
//! Apparently they did exactly that in bevy 0.11 like a month afterwards.
//!
//! Usually I check whether a feature is coming up soon so I don't redo something. I guess I forgot.
//! Anyways, this is still a bit useful, so I'll give it a cute little module.
use std::{f32::consts::PI, marker::PhantomData};

use bevy::prelude::*;

/// One meter in terms of game units when calculating the inverse square law for audio.
// TODO: need to find good number for this
pub const AUDIO_SCALE: f32 = 16.0;

pub struct SoundPlugin<T: Component> {
    phantom_data: PhantomData<T>,
}

impl<T: Component> Default for SoundPlugin<T> {
    fn default() -> Self {
        Self {
            phantom_data: PhantomData::default(),
        }
    }
}

impl<T: Component> Plugin for SoundPlugin<T> {
    fn build(&self, app: &mut App) {
        // TODO: figure out some jank system ordering... because the audio system set is not pub...
        // as usual...
        app.add_systems(PostUpdate, move_spatial_audio::<T>);
    }
}

/// Location to receive spatial audio. `0` is the distance between L/R ears.
/// There should only be one existing at a time.
#[derive(Component)]
pub struct Ears(pub f32);

/// Automatically binds `SpatialAudioSink.listener.translation` to `sound::Ears`,
/// `SpatialAudioSink.listener.rotation` to `character::camera::PlayerCamera`,
/// and `SpatialAudioSink.emitter` to this entity.
///
/// It's essentially a simpler but more restrictive version of `SpatialSettings`.
#[derive(Component, Default)]
pub struct TrackedSpatialSettings;

#[derive(Bundle)]
pub struct TrackedSpatialAudioBundle {
    pub source: Handle<AudioSource>,
    pub settings: PlaybackSettings,
    /// This just needs to be here to insert the `SpatialAudioSink`.
    /// Its properties are irrelevant.
    pub spatial: SpatialSettings,
    pub tracked: TrackedSpatialSettings,
}

impl Default for TrackedSpatialAudioBundle {
    fn default() -> Self {
        Self {
            source: Handle::default(),
            settings: PlaybackSettings::default(),
            spatial: SpatialSettings::new(Transform::default(), 0.1, Vec3::default()),
            tracked: TrackedSpatialSettings::default(),
        }
    }
}

/// Scales transform translation to `AUDIO_SCALE`.
#[inline]
fn scale_transform(t: &GlobalTransform) -> Transform {
    t.compute_transform()
        .with_translation(t.translation() / AUDIO_SCALE)
}

pub fn move_spatial_audio<T: Component>(
    camera_query: Query<&GlobalTransform, With<T>>,
    emitter_query: Query<(&SpatialAudioSink, &GlobalTransform), With<TrackedSpatialSettings>>,
    listener_query: Query<(&Ears, &GlobalTransform)>,
) {
    let Ok(camera) = camera_query.get_single().map(|t| t.compute_transform()) else {
        return;
    };

    let Ok((Ears(gap), listener)) = listener_query.get_single() else {
        return;
    };

    // stereo is backwards. I dunno if this is bevy's fault or mine, but I can't find the issue
    // for the life of me, so...
    let listener = scale_transform(listener)
        .with_rotation(camera.rotation * Quat::from_axis_angle(camera.up(), PI));
    let gap = *gap / AUDIO_SCALE;

    for (audio, emitter) in emitter_query.iter() {
        let emitter = scale_transform(emitter);
        audio.set_listener_position(listener, gap);
        audio.set_emitter_position(emitter.translation);
    }
}
