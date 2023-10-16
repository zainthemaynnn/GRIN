use bevy::{ecs::system::BoxedSystem, prelude::*};

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
