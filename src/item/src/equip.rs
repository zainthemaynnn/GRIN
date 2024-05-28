use std::marker::PhantomData;

use bevy::{prelude::*, utils::HashMap};
use grin_rig::humanoid::{Humanoid, HumanoidDominantHand};
use grin_util::event::UntypedEvent;

use crate::{library::plugin::ItemIdentifier, plugin::ItemSet};

pub struct EquipPlugin;

impl Plugin for EquipPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<UntypedItemEquipEvent>().add_systems(
            PostUpdate,
            (convert_untyped_events, equip_items).in_set(ItemSet::Equip),
        );
    }
}

/// Event that can be used to equip items to a player or NPC.
#[derive(Event, Clone)]
pub struct UntypedItemEquipEvent {
    /// Entity that the item is being equipped to.
    pub parent_entity: Entity,
    /// Item to be equipped.
    pub item_entity: Entity,
    /// Item slots that will be occupied by this item. If using `Manual`, this should always be:
    ///
    /// - `SlotAlignment::Left` or `SlotAlignment::Right` for `Handedness::Single` items
    /// - `SlotAlignment::Double` for `Handedness::Double` items
    /// - `SlotAlignment::None` for items that are being unequipped
    ///
    /// There are no compile checks. You have been warned.
    pub slot: ItemEquipEventSlot,
}

#[derive(Copy, Clone, Default)]
pub enum ItemEquipEventSlot {
    Manual {
        alignment: SlotAlignment,
    },
    #[default]
    Auto,
}

impl UntypedEvent for UntypedItemEquipEvent {
    type TypedEvent<I> = ItemEquipEvent<I>;

    fn typed<I>(&self) -> Self::TypedEvent<I> {
        ItemEquipEvent {
            parent_entity: self.parent_entity,
            item_entity: self.item_entity,
            slot: self.slot,
            phantom_data: PhantomData::default(),
        }
    }
}

/// The typed equivalent of `UntypedEquipEvent`. The generic corresponds to the item being
/// equipped.
///
/// If sending an equip event, prefer using the untyped version over this version. The reason
/// behind this is that untyped events will be converted to typed events and sent again,
/// while the reverse does not apply. The typed event is generally used for internal
/// item implementations.
#[derive(Event, Clone)]
pub struct ItemEquipEvent<I> {
    pub parent_entity: Entity,
    pub item_entity: Entity,
    pub slot: ItemEquipEventSlot,
    pub phantom_data: PhantomData<I>,
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

#[derive(Component, Clone, Debug, Default)]
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

// I don't know if this is a good idea TBH, but whatever. the exclusive world access seems
// necessary due to the sheer number of event writers that exist. the alternative is
// to make separate systems for each item which all read the event queue, but I *assume*
// that would be even slower.
// TODO: now that I know how to use `SystemParam`, I think this can be done better,
// but it will need some more macro generation.
/// Consumes all `UntypedItemEquipEvent`s, sending `ItemEquipEvent<T>` where `T` is the
/// corresponding item identifier type.
pub fn convert_untyped_events(world: &mut World) {
    world.resource_scope::<Events<UntypedItemEquipEvent>, _>(|world, events| {
        // TODO?: figure out if I can batch send? it's difficult cause of borrowing rules.
        let mut reader = events.get_reader();
        for ev in reader.read(&events) {
            world.send_event(
                world
                    .get::<ItemIdentifier>(ev.item_entity)
                    .unwrap()
                    .typed_event(ev),
            );
        }
    });
}

/// Updates the `Equipped` component when sending `ItemEquippedEvent`s.
pub fn equip_items(
    mut commands: Commands,
    mut events: EventReader<UntypedItemEquipEvent>,
    mut humanoid_query: Query<(&Humanoid, &mut Equipped)>,
    mut item_query: Query<(&ItemIdentifier, &Models, &Handedness, &mut SlotAlignment)>,
) {
    for UntypedItemEquipEvent {
        parent_entity,
        item_entity,
        slot: event_slot,
    } in events.read()
    {
        let Ok((humanoid, mut equipped)) = humanoid_query.get_mut(*parent_entity) else {
            error!("Equipped item to non-humanoid.");
            continue;
        };

        let Ok((item_id, models, handedness, mut slot_alignment)) =
            item_query.get_mut(*item_entity)
        else {
            error!("Missing equipment-related components.");
            continue;
        };

        let slot = match event_slot {
            ItemEquipEventSlot::Manual { alignment } => *alignment,
            ItemEquipEventSlot::Auto => match handedness {
                Handedness::Double => SlotAlignment::Double,
                Handedness::Single => match humanoid.dominant_hand_type {
                    HumanoidDominantHand::Left => SlotAlignment::Left,
                    HumanoidDominantHand::Right => SlotAlignment::Right,
                },
            },
        };

        *slot_alignment = slot;

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

        info!(
            "Equipped {:?} ({:?}) to {:?}.",
            parent_entity, item_id, item_entity,
        );
    }
}
