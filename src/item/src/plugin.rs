use std::marker::PhantomData;

use bevy::{app::PluginGroupBuilder, prelude::*};
use grin_asset::AssetLoadState;
use grin_damage::ContactDamage;
use grin_util::event::Spawnable;

use crate::{
    equip::{equip_items, Handedness, ItemEquipEvent, Models},
    mechanics::{
        combo::ComboStack,
        firing::{Accuracy, Target},
        fx::ItemFxPlugin,
        hitbox::{
            DamageCollisionGroups, GltfHitboxAutoGen, GltfHitboxAutoGenConfig,
            GltfHitboxGenerationPlugin, HitboxManager,
        },
    },
    spawn::ItemSpawnEvent,
};

pub const GLTF_HITBOX_IDENTIFIER: &str = "__HB";

pub trait Item: Component {}

/// I hate this SO MUCH WHAT THE HELL IS THE POINT I MADE THE OTHER CRATE
pub struct SpawnableItem<I: Item> {
    phantom_data: PhantomData<I>,
}

impl<I: Item + Clone> Spawnable for SpawnableItem<I> {
    type Event = ItemSpawnEvent<I>;
}

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum ItemSet {
    Spawn,
}

pub struct MasterItemPlugin;

impl Plugin for MasterItemPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            Update,
            ItemSet::Spawn.run_if(in_state(AssetLoadState::Success)),
        );
    }
}

pub struct ItemPlugin<I: Component> {
    pub phantom_data: PhantomData<I>,
}

impl<I: Component> Default for ItemPlugin<I> {
    fn default() -> Self {
        Self {
            phantom_data: PhantomData::default(),
        }
    }
}

impl<I: Component> Plugin for ItemPlugin<I> {
    fn build(&self, app: &mut App) {
        app.add_event::<ItemSpawnEvent<I>>()
            .add_event::<ItemEquipEvent<I>>()
            .add_systems(Update, equip_items::<I>);
    }
}

pub struct ItemPlugins;

impl PluginGroup for ItemPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(MasterItemPlugin)
            .add(ItemFxPlugin)
            .add(GltfHitboxGenerationPlugin {
                config: GltfHitboxAutoGenConfig(GLTF_HITBOX_IDENTIFIER.into()),
            })
    }
}

#[derive(Component, Default)]
pub struct Weapon;

#[derive(Bundle)]
pub struct WeaponBundle<C: Send + Sync + 'static> {
    /// Generic weapon marker component.
    pub weapon: Weapon,
    /// Weapon target (ignore during init).
    pub target: Target,
    /// Weapon accuracy.
    pub accuracy: Accuracy,
    /// Weapon damage collision detection.
    pub damage_collision_groups: DamageCollisionGroups,
    /// Weapon handedness.
    pub handedness: Handedness,
    /// Hitbox auto-generation setting (default: `Enabled`).
    pub hitbox_gen: GltfHitboxAutoGen,
    /// Weapon combo sequence.
    pub combo_stack: ComboStack<C>,
    /// Associated models.
    pub models: Models,
    /// Associated hitboxes.
    pub hitboxes: HitboxManager,
    /// Contact damage (default: `Disabled`).
    pub contact_damage: ContactDamage,
}

// I love rust it really is my favourite language :) :) <333
impl<C: Send + Sync + 'static> Default for WeaponBundle<C> {
    fn default() -> Self {
        Self {
            combo_stack: ComboStack::<C>::default(), // WHY
            weapon: Weapon::default(),
            target: Target::default(),
            accuracy: Accuracy::default(),
            damage_collision_groups: DamageCollisionGroups::default(),
            handedness: Handedness::default(),
            hitbox_gen: GltfHitboxAutoGen::default(),
            models: Models::default(),
            hitboxes: HitboxManager::default(),
            contact_damage: ContactDamage::default(),
        }
    }
}
