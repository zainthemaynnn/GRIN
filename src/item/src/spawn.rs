use std::marker::PhantomData;

use bevy::{ecs::system::SystemParam, prelude::*};

use crate::equip::{ItemEquipEventSlot, UntypedItemEquipEvent};

#[derive(Event, Clone)]
pub struct ItemSpawnEvent<I: Component> {
    /// Entity to equip to. If `Some`, the item spawner will automatically attempt to equip
    /// to this entity. If `None`, the item is treated as a pickup.
    pub parent_entity: Option<Entity>,
    /// Item pickup transform.
    pub transform: Transform,
    pub phantom_data: PhantomData<I>,
}

impl<I: Component> Default for ItemSpawnEvent<I> {
    fn default() -> Self {
        Self {
            parent_entity: None,
            transform: Transform::default(),
            phantom_data: PhantomData::default(),
        }
    }
}

#[derive(SystemParam)]
pub struct ItemSpawnerParams<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub equip_events: EventWriter<'w, UntypedItemEquipEvent>,
}

/// Wrapper for a read-only system returning a `Bundle`, which describes the item properties at spawn.
/// This bundle will be used as a template when responding to `ItemSpawnEvent`s. The system wrapper
/// also sets the `TransformBundle` of the entity and manages auto-equipping with `parent_entity`.
///
/// Note: `Events<ItemSpawnEvent<I>>` cannot be used as a system param of `spawn_fn`.
pub fn item_spawner<I, B, F, Marker>(
    mut spawn_fn: F,
) -> impl FnMut(In<F::In>, EventReader<ItemSpawnEvent<I>>, ParamSet<(F::Param, ItemSpawnerParams)>) -> ()
where
    I: Component,
    B: Bundle,
    F: SystemParamFunction<Marker, In = (), Out = B>,
{
    move |In(spawn_fn_in), mut spawn_events, mut params| {
        for ItemSpawnEvent {
            parent_entity,
            transform,
            ..
        } in spawn_events.read()
        {
            let bundle = spawn_fn.run(spawn_fn_in, params.p0());

            let ItemSpawnerParams {
                mut commands,
                mut equip_events,
            } = params.p1();

            let e_item = commands
                .spawn(bundle)
                .insert(TransformBundle::from_transform(*transform))
                .id();
            info!("Spawning item {:?}", e_item);

            if let Some(&e_parent) = parent_entity.as_ref() {
                equip_events.send(UntypedItemEquipEvent {
                    parent_entity: e_parent,
                    item_entity: e_item,
                    slot: ItemEquipEventSlot::Auto,
                });
            }
        }
    }
}
