use bevy::{app::PluginGroupBuilder, prelude::*};

use super::fist::FistPlugin;

pub struct ItemLibrary;

impl PluginGroup for ItemLibrary {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>().add(FistPlugin)
    }
}
