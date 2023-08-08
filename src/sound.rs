//! This was supposed to make audio simpler.
//! Apparently they did exactly that in bevy 0.11 like a month afterwards.
//!
//! Usually I check whether a feature is coming up soon so I don't redo something. I guess I forgot.
//! Anyways, this is still a bit useful, so I'll give it a cute little module.
use bevy::prelude::*;

use crate::character::AvatarLoadState;

/// One meter in terms of game units when calculating the inverse square law for audio.
// TODO: need to find good number for this
pub const AUDIO_SCALE: f32 = 16.0;

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            move_spatial_audio.run_if(in_state(AvatarLoadState::Loaded)),
        );
    }
}

/// Location to receive spatial audio. `0` is the distance between L/R ears.
/// There should only be one existing at a time.
#[derive(Component)]
pub struct Ears(pub f32);

/// Automatically binds `SpatialAudioSink.listener` to the global transform of `sound::Ears`,
/// and `SpatialAudioSink.emitter` to the global transform of this entity.
///
/// It's essentially a simpler but more restrictive version of `SpatialSettings`.
#[derive(Component, Default)]
pub struct TrackedSpatialSettings;

#[derive(Bundle, Default)]
pub struct TrackedSpatialAudioBundle {
    pub source: Handle<AudioSource>,
    pub settings: PlaybackSettings,
    pub spatial: TrackedSpatialSettings,
}

/// Updates the listeners/emitters of `LocalizedSound`.
pub fn move_spatial_audio(
    emitter_query: Query<(&SpatialAudioSink, &GlobalTransform), With<TrackedSpatialSettings>>,
    listener_query: Query<(&Ears, &GlobalTransform)>,
) {
    let (Ears(gap), listener) = listener_query.single();
    let listener = listener
        .compute_transform()
        .with_scale(Vec3::splat(AUDIO_SCALE));
    let gap = *gap / AUDIO_SCALE;
    for (audio, emitter) in emitter_query.iter() {
        audio.set_listener_position(listener, gap);
        audio.set_emitter_position(emitter.compute_transform().translation / AUDIO_SCALE);
    }
}
