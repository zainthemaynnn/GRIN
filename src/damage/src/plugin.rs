use bevy::{app::PluginGroupBuilder, prelude::*};

use crate::{
    health::HealthPlugin,
    hit::ContactDamagePlugin,
    hitbox::{GltfHitboxAutoGenConfig, GltfHitboxGenerationPlugin},
    impact::ImpactPlugin,
    projectiles::ProjectilePlugin,
};

/// Health and damage calculations.
pub struct DamagePlugins;

impl PluginGroup for DamagePlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(ProjectilePlugin)
            .add(ContactDamagePlugin)
            .add(HealthPlugin)
            .add(ImpactPlugin)
            .add(GltfHitboxGenerationPlugin {
                config: GltfHitboxAutoGenConfig(String::from("__HB")),
            })
    }
}

#[derive(SystemSet, Debug, Hash, Eq, PartialEq, Copy, Clone)]
pub enum DamageSet {
    /// `DamageBuffer`s are empty in this stage.
    Add,
    /// `DamageBuffer`s are propagated in this stage.
    Propagate,
    /// `Resist` is applied in this stage.
    Resist,
    /// `DamageBuffer`s are cleared in this stage.
    Clear,
    /// Things die in this stage.
    Kill,
}
