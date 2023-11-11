use bevy::{ecs::system::BoxedSystem, prelude::*};
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
    for TweenCompleted { entity, user_data } in events.read() {
        if *user_data == TweenCompletedEvent::Despawn as u64 {
            commands.get_or_spawn(*entity).despawn_recursive();
        }
    }
}

pub trait Spawnable {
    type Event: Event + Clone;

    fn spawn_with(event: Self::Event) -> BoxedSystem {
        // TODO: can I do this without cloning event? beats me...
        Box::new(IntoSystem::into_system(
            move |mut events: EventWriter<Self::Event>| {
                events.send(event.clone());
            },
        ))
    }
}

pub trait DefaultSpawnable<E: Event + Clone + Default>: Spawnable<Event = E> {
    fn spawn_default() -> BoxedSystem {
        Box::new(IntoSystem::into_system(
            move |mut events: EventWriter<Self::Event>| events.send_default(),
        ))
    }
}

impl<E: Event + Clone + Default, T: Spawnable<Event = E>> DefaultSpawnable<E> for T {}
