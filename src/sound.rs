use bevy::prelude::*;

use crate::character::AvatarLoadState;

/// One meter in terms of game units when calculating the inverse square law for audio.
// TODO: need to find good number for this
pub const AUDIO_SCALE: f32 = 16.0;

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            (
                SoundSet::Play.run_if(in_state(AvatarLoadState::Loaded)),
                SoundSet::Translate.run_if(in_state(AvatarLoadState::Loaded)),
            )
                .chain(),
        )
        .add_systems(
            (
                play_localized_audio.in_set(SoundSet::Play),
                apply_system_buffers,
                move_localized_audio.in_set(SoundSet::Translate),
            )
                .chain(),
        );
    }
}

#[derive(SystemSet, Debug, Hash, Eq, PartialEq, Copy, Clone)]
pub enum SoundSet {
    Play,
    Translate,
}

/// Location to receive spatial audio. `0` is the distance between L/R ears.
/// There should only be one existing at a time.
#[derive(Component)]
pub struct Ears(pub f32);

/// Insert to begin playback of a `LocalizedSound`. Removed on use.
#[derive(Component)]
pub struct InitLocalizedSound(pub Handle<AudioSource>, pub PlaybackSettings);

/// Spatial audio that updates positional data every frame.
#[derive(Component)]
pub struct LocalizedSound(pub Handle<SpatialAudioSink>);

/// Uses `InitLocalizedSound` to play `LocalizedSound`.
pub fn play_localized_audio(
    mut commands: Commands,
    emitter_query: Query<(Entity, &InitLocalizedSound, &GlobalTransform)>,
    listener_query: Query<(&Ears, &GlobalTransform)>,
    audio_sinks: Res<Assets<SpatialAudioSink>>,
    audio: Res<Audio>,
) {
    let (Ears(gap), listener) = listener_query.single();
    let listener_scaled = listener
        .compute_transform()
        .with_translation(listener.translation() / AUDIO_SCALE);
    for (entity, sound, emitter) in emitter_query.iter() {
        let handle = audio.play_spatial_with_settings(
            sound.0.clone(),
            sound.1,
            listener_scaled,
            *gap / AUDIO_SCALE,
            emitter.translation() / AUDIO_SCALE,
        );
        commands
            .get_or_spawn(entity)
            .remove::<InitLocalizedSound>()
            .insert(LocalizedSound(audio_sinks.get_handle(handle)));
    }
}

/// Updates the listeners/emitters of `LocalizedSound`.
pub fn move_localized_audio(
    emitter_query: Query<(&LocalizedSound, &GlobalTransform)>,
    listener_query: Query<(&Ears, &GlobalTransform)>,
    audio_sinks: Res<Assets<SpatialAudioSink>>,
) {
    let (Ears(gap), listener) = listener_query.single();
    let listener_scaled = listener
        .compute_transform()
        .with_translation(listener.translation() / AUDIO_SCALE);
    for (sound, emitter) in emitter_query.iter() {
        if let Some(sink) = audio_sinks.get(&sound.0) {
            sink.set_listener_position(listener_scaled, *gap / AUDIO_SCALE);
            sink.set_emitter_position(emitter.translation() / AUDIO_SCALE);
        }
    }
}
