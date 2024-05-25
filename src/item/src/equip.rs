use std::marker::PhantomData;

use bevy::{prelude::*, utils::HashMap};
use grin_rig::humanoid::{Humanoid, HumanoidDominantHand};

use crate::spawn::ItemSpawnEvent;

#[derive(Event, Clone)]
pub struct ItemEquipEvent<M> {
    pub parent_entity: Entity,
    pub item_entity: Entity,
    pub slot: SlotAlignment,
    pub phantom_data: PhantomData<M>,
}

impl<M> ItemEquipEvent<M> {
    pub fn new(parent_entity: Entity, item_entity: Entity, slot: SlotAlignment) -> Self {
        Self {
            parent_entity,
            item_entity,
            slot,
            phantom_data: PhantomData::default(),
        }
    }
}

/// Keeps references to currently bound items.
#[derive(Component)]
pub struct Equipped {
    pub left: Entity,
    pub right: Entity,
}

/// Corresponding item slot. `None` means the item is not equipped.
#[derive(Component, Clone, Copy, Debug, Default)]
pub enum SlotAlignment {
    Left,
    Right,
    Double,
    #[default]
    None,
}

/// Location for item to be parented on a rig. If it's a single-handed item,
/// the model loader defaults to `Hand` and `Offhand` is ignored.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum Grip {
    Head,
    Body,
    Hand,
    Offhand,
}

/// Whether the item is single or double-handed.
#[derive(Component, Clone, Copy, Debug, Default)]
pub enum Handedness {
    #[default]
    Single,
    Double,
}

#[derive(Component, Default)]
pub struct Models {
    pub targets: HashMap<Grip, Entity>,
}

impl From<HashMap<Grip, Entity>> for Models {
    fn from(value: HashMap<Grip, Entity>) -> Self {
        Self { targets: value }
    }
}

/// Spawns multiple scenes and binds multiple `Grips` at once, returning as a `Models`.
#[macro_export]
macro_rules! models {
    ( $commands:expr, $( ($key:expr, $handle:expr) ),* $(,)?) => {
        {
            use bevy::prelude::*;
            use bevy::utils::HashMap;

            let mut map = HashMap::new();
            $(
                map.insert($key, $commands.spawn(SceneBundle {
                    scene: $handle,
                    ..Default::default()
                }).id());
            )*
            Models::from(map)
        }
    }
}

/// Updates the `Equipped` component when sending `ItemEquippedEvent`s.
pub fn equip_items<M: Send + Sync + 'static>(
    mut commands: Commands,
    mut events: EventReader<ItemEquipEvent<M>>,
    mut humanoid_query: Query<(&Humanoid, &mut Equipped)>,
    mut item_query: Query<(&Models, &mut SlotAlignment)>,
) {
    for ItemEquipEvent {
        parent_entity,
        item_entity,
        slot,
        ..
    } in events.read()
    {
        let Ok((humanoid, mut equipped)) = humanoid_query.get_mut(*parent_entity) else {
            error!("Equipped item to non-humanoid.");
            continue;
        };

        let Ok((models, mut slot_alignment)) = item_query.get_mut(*item_entity) else {
            error!("Missing equipment-related components.");
            continue;
        };

        *slot_alignment = *slot;

        info!("Equipped item {:?} to {:?}.", parent_entity, item_entity);

        if let Some(&e_model) = models.targets.get(&Grip::Head) {
            commands.entity(e_model).set_parent(humanoid.head);
        }

        if let Some(&e_model) = models.targets.get(&Grip::Body) {
            commands.entity(e_model).set_parent(humanoid.body);
        }

        match slot {
            SlotAlignment::Left => {
                if let Some(&e_model) = models.targets.get(&Grip::Hand) {
                    commands.entity(e_model).set_parent(humanoid.lhand);
                }
                equipped.left = *item_entity;
            }
            SlotAlignment::Right => {
                if let Some(&e_model) = models.targets.get(&Grip::Hand) {
                    commands.entity(e_model).set_parent(humanoid.rhand);
                }
                equipped.right = *item_entity;
            }
            SlotAlignment::Double => {
                if let Some(&e_model) = models.targets.get(&Grip::Hand) {
                    commands
                        .entity(e_model)
                        .set_parent(humanoid.dominant_hand());
                }
                if let Some(&e_model) = models.targets.get(&Grip::Offhand) {
                    commands.entity(e_model).set_parent(humanoid.off_hand());
                }
                equipped.left = *item_entity;
                equipped.right = *item_entity;
            }
            SlotAlignment::None => unimplemented!(), // may use this for unequip?
        }
    }
}

/// Automatically sends an equip event to the player character.
pub fn auto_equip_to_humanoid<T: Component>(
    In(e_item): In<Entity>,
    humanoid_query: Query<&HumanoidDominantHand, With<Humanoid>>,
    handedness_query: Query<&Handedness>,
    mut spawn_events: EventReader<ItemSpawnEvent<T>>,
    mut equip_events: EventWriter<ItemEquipEvent<T>>,
) {
    for ItemSpawnEvent {
        parent_entity: e_parent,
        ..
    } in spawn_events.read()
    {
        let Ok(handedness) = handedness_query.get(e_item) else {
            error!("Missing item `Handedness`.");
            continue;
        };

        let Ok(dominant) = humanoid_query.get(*e_parent) else {
            error!("Attempted humanoid equip to non-humanoid.");
            continue;
        };

        equip_events.send(ItemEquipEvent::new(
            *e_parent,
            e_item,
            // jeez man... too many hand-related components...
            match handedness {
                Handedness::Double => SlotAlignment::Double,
                Handedness::Single => match dominant {
                    HumanoidDominantHand::Left => SlotAlignment::Left,
                    HumanoidDominantHand::Right => SlotAlignment::Right,
                },
            },
        ));
    }
}
