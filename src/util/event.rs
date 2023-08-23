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
