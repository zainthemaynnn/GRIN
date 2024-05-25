use std::marker::PhantomData;

use bevy::prelude::*;

#[derive(Event, Clone)]
pub struct ItemSpawnEvent<M> {
    pub parent_entity: Entity,
    pub phantom_data: PhantomData<M>,
}

impl<M> ItemSpawnEvent<M> {
    pub fn new(parent_entity: Entity) -> Self {
        Self {
            parent_entity,
            phantom_data: PhantomData::default(),
        }
    }
}
