use bevy::{app::PluginGroupBuilder, prelude::*};
use bevy_enum_filter::EnumFilter;
use grin_derive::TypedEvents;

use super::fist::FistPlugin;

pub struct ItemLibrary;

impl PluginGroup for ItemLibrary {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>().add(FistPlugin)
    }
}

#[derive(Component, EnumFilter, TypedEvents, Default)]
pub enum ItemIdentifier {
    #[default]
    Fist,
}
