use std::marker::PhantomData;

use bevy::{app::PluginGroupBuilder, prelude::*};
use bevy_enum_filter::prelude::AddEnumFilter;
use bevy_rapier3d::prelude::*;
use grin_asset::AssetLoadState;
use grin_damage::{
    hit::ContactDamage,
    hitbox::{GltfHitboxAutoGenTemplate, HitboxManager},
};
use grin_physics::{CollisionGroupExt, CollisionGroupsExt};

use crate::{
    equip::{EquipPlugin, GltfHitboxAutoGen, Handedness, ItemEquipEvent, Models},
    library::plugin::ItemIdentifier,
    mechanics::{
        combo::ComboStack,
        firing::{Accuracy, FireRate, FiringMode, ShotCooldown, Target},
        fx::ItemFxPlugin,
    },
    spawn::ItemSpawnEvent,
};

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum ItemSet {
    Spawn,
    Equip,

    Input,
    Fire,
    Effects,
}

pub struct MasterItemPlugin;

impl Plugin for MasterItemPlugin {
    fn build(&self, app: &mut App) {
        app.add_enum_filter::<ItemIdentifier>()
            .add_enum_filter::<FiringMode>()
            .configure_sets(
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
    pub collision_groups: CollisionGroups,
    /// Weapon handedness.
    pub handedness: Handedness,
    /// Hitbox auto-generation setting (default: `Enabled`).
    pub hitbox_gen: GltfHitboxAutoGen,
    /// Hitbox auto-generation template.
    pub hitbox_template: GltfHitboxAutoGenTemplate,
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
            weapon: Weapon::default(),
            identifier: ItemIdentifier::default(),
            target: Target::default(),
            accuracy: Accuracy::default(),
            collision_groups: CollisionGroups::from_group_default(Group::PLAYER_PROJECTILE),
            handedness: Handedness::default(),
            hitbox_gen: GltfHitboxAutoGen::default(),
            hitbox_template: GltfHitboxAutoGenTemplate::Hitbox,
            combo_stack: ComboStack::<C>::default(),
            models: Models::default(),
            hitboxes: HitboxManager::default(),
            contact_damage: ContactDamage::default(),
            fire_rate: FireRate::default(),
            cooldown: ShotCooldown::default(),
            firing_mode: FiringMode::default(),
        }
    }
}
