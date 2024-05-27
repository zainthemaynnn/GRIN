use std::marker::PhantomData;

use bevy::prelude::*;
use grin_rig::humanoid::{Humanoid, HumanoidDominantHand};

use crate::equip::{Handedness, SlotAlignment, UntypedItemEquipEvent};

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

// I don't know what the hell I just cooked
/// Wrapper for a read-only system returning a `Bundle`, which describes the item properties at spawn.
/// This bundle will be used as a template when responding to `ItemSpawnEvent`s. The system wrapper
/// also sets the `TransformBundle` of the entity and manages auto-equipping with `parent_entity`.
pub fn item_spawner<I: Component, B: Bundle, Marker: 'static>(
    spawn_fn: impl IntoSystem<(), B, Marker, System = impl ReadOnlySystem<In = (), Out = B>>,
) -> impl System<In = (), Out = ()> {
    let mut spawn_sys = IntoSystem::into_system(spawn_fn);

    IntoSystem::into_system(
        move |world: &World,
              mut commands: Commands,
              humanoid_query: Query<&HumanoidDominantHand, With<Humanoid>>,
              handedness_query: Query<&Handedness>,
              mut spawn_events: EventReader<ItemSpawnEvent<I>>,
              mut equip_events: EventWriter<UntypedItemEquipEvent>| {
            for ItemSpawnEvent {
                parent_entity,
                transform,
                ..
            } in spawn_events.read()
            {
                let e_item = commands
                    /* using `run_readonly` got me like
                    ⠀⠀⠀⠀⠀⠀⠀⠀⣀⣴⣶⣿⣿⣿⣿⣿⣿⣿⣶⣦⣀⠀⠀⠀⠀⠀⠀⠀
                    ⠀⠀⠀⠀⠀⠀⣤⣾⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣄⠀⠀⠀⠀⠀
                    ⠀⠀⠀⠀⢀⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣧⠀⠀⠀⢠
                    ⠀⠀⠀⠀⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣟⣛⣻⣿⣿⣟⣿⣿⣿⣷⠀⠀⠀
                    ⠀⠀⠀⠀⣿⣿⣿⣿⣿⣿⣿⣿⣿⣫⣽⣾⣻⣾⣿⣿⣿⣿⡿⣿⣿⠀⠀⠀
                    ⠀⠀⠀⢰⣿⣿⣻⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⠻⡿⠿⠟⠛⣟⣿⣽⠀⠀⠀
                    ⠀⠀⠀⠸⣿⣿⣿⣷⣿⣿⣿⣿⡿⠍⠈⠀⠁⣴⡆⠀⠀⠠⢭⣮⣿⡶⠀⠀
                    ⠀⡴⠲⣦⢽⣿⣿⣿⣿⣿⣟⣩⣨⣀⡄⣐⣾⣿⣿⣇⠠⣷⣶⣿⣿⡠⠁⠀
                    ⠀⠃⢀⡄⠀⢻⣿⣿⣿⣿⣽⢿⣿⣯⣾⣿⣿⣿⣿⣿⢿⣿⣿⡟⣿⠀⠀⠀
                    ⠀⠀⠣⠧⠀⢿⣿⣿⣿⣿⣿⣿⣿⣿⠟⢸⣿⠿⠿⠿⣧⠙⣿⣿⡿⠀⠀⠀
                    ⠀⠀⠀⠁⠼⣒⡿⣿⣿⣿⣿⣿⣿⣿⣠⣬⠀⠀⠀⠀⣾⣷⡈⣿⡇⠀⠀⠀
                    ⠀⠀⠀⠀⠀⠉⢳⣿⣿⣿⣿⣿⣿⣿⢟⠗⠼⠖⠒⠔⠉⠉⠻⣿⠇⠀⠀⠀
                    ⠀⠀⠀⠀⠀⠀⠈⣻⡿⣿⣿⣿⣿⡿⡀⣤⡄⠸⣰⣾⡒⣷⣴⣿⠀⠀⠀⠀
                    ⠀⠀⠀⠀⠀⠀⠂⢸⡗⡄⠘⠭⣭⣷⣿⣮⣠⣌⣫⣿⣷⣿⣿⠃⠀⠈⠀⠀
                    ⠀⠀⠀⠀⠀⠈⠀⢸⣿⣾⣷⣦⡿⣿⣿⣿⡿⢻⠞⣹⣿⣿⠏⠀⠀⠀⠀⠀
                    ⠀⠀⠀⠀⠀⢘⠀⠘⢻⡿⢿⣋⣤⣤⠌⠉⠛⠛⠀⠈⠉⠁⠀⠀⠀⠀⠀⡀*/
                    .spawn(spawn_sys.run_readonly((), world))
                    .insert(TransformBundle::from_transform(*transform))
                    .id();

                if let Some(&e_parent) = parent_entity.as_ref() {
                    let Ok(handedness) = handedness_query.get(e_item) else {
                        error!("Missing item `Handedness`.");
                        continue;
                    };

                    let Ok(dominant) = humanoid_query.get(e_parent) else {
                        error!("Attempted humanoid equip to non-humanoid.");
                        continue;
                    };

                    equip_events.send(UntypedItemEquipEvent {
                        parent_entity: e_parent,
                        item_entity: e_item,
                        slot: match handedness {
                            Handedness::Double => SlotAlignment::Double,
                            Handedness::Single => match dominant {
                                HumanoidDominantHand::Left => SlotAlignment::Left,
                                HumanoidDominantHand::Right => SlotAlignment::Right,
                            },
                        },
                    });
                }
            }
        },
    )
}
