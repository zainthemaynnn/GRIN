use bevy::prelude::*;
use bevy_tweening::TweenCompleted;

pub struct TweenEventPlugin;

impl Plugin for TweenEventPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, despawn_tweens);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum TweenCompletedEvent {
    Despawn,
}

pub fn despawn_tweens(mut commands: Commands, mut events: EventReader<TweenCompleted>) {
    for TweenCompleted { entity, user_data } in events.iter() {
        if *user_data == TweenCompletedEvent::Despawn as u64 {
            commands.get_or_spawn(*entity).despawn_recursive();
        }
    }
}
