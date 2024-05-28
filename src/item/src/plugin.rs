use std::marker::PhantomData;

use bevy::{app::PluginGroupBuilder, prelude::*};
use grin_asset::AssetLoadState;
use grin_damage::ContactDamage;

use crate::{
    equip::{EquipPlugin, Handedness, ItemEquipEvent, Models, SlotAlignment},
    library::plugin::ItemIdentifier,
    mechanics::{
        combo::ComboStack,
        firing::{Accuracy, FireRate, FiringMode, ShotCooldown, Target},
        fx::ItemFxPlugin,
        hitbox::{
            DamageCollisionGroups, GltfHitboxAutoGen, GltfHitboxAutoGenConfig,
            GltfHitboxGenerationPlugin, HitboxManager,
        },
    },
    spawn::ItemSpawnEvent,
};

pub const GLTF_HITBOX_IDENTIFIER: &str = "__HB";

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum ItemSet {
    Spawn,
    Equip,
}

pub struct MasterItemPlugin;

impl Plugin for MasterItemPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            Update,
            ItemSet::Spawn.run_if(in_state(AssetLoadState::Success)),
        )
        .configure_sets(
            PostUpdate,
            ItemSet::Equip.run_if(in_state(AssetLoadState::Success)),
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
            .add_event::<ItemEquipEvent<I>>();
    }
}

pub struct ItemPlugins;

impl PluginGroup for ItemPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(MasterItemPlugin)
            .add(EquipPlugin)
            .add(ItemFxPlugin)
            .add(GltfHitboxGenerationPlugin {
                config: GltfHitboxAutoGenConfig(GLTF_HITBOX_IDENTIFIER.into()),
            })
    }
}

#[derive(Component, Copy, Clone, Debug, Default)]
pub struct Weapon;

#[derive(Bundle, Clone)]
pub struct WeaponBundle<C: Send + Sync + 'static> {
    /// Generic weapon marker component.
    pub weapon: Weapon,
    /// Item ID.
    pub identifier: ItemIdentifier,
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
    /// Weapon fire rate.
    pub fire_rate: FireRate,
    /// Weapon attack cooldown.
    pub cooldown: ShotCooldown,
    /// Weapon firing mode (default: `SemiAuto`).
    pub firing_mode: FiringMode,
}

// I love rust it really is my favourite language :) :) <333
impl<C: Send + Sync + 'static> Default for WeaponBundle<C> {
    fn default() -> Self {
        Self {
            combo_stack: ComboStack::<C>::default(), // WHY
            weapon: Weapon::default(),
            identifier: ItemIdentifier::default(),
            target: Target::default(),
            accuracy: Accuracy::default(),
            damage_collision_groups: DamageCollisionGroups::default(),
            handedness: Handedness::default(),
            hitbox_gen: GltfHitboxAutoGen::default(),
            models: Models::default(),
            hitboxes: HitboxManager::default(),
            contact_damage: ContactDamage::default(),
            fire_rate: FireRate::default(),
            cooldown: ShotCooldown::default(),
            firing_mode: FiringMode::default(),
        }
    }
}
