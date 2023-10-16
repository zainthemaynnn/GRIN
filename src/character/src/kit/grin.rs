use bevy::prelude::*;

use crate::{AvatarAssets, Character, CharacterSet, PlayerCharacter, GenericHumanoidCharacterPlugin};
use grin_asset::AssetLoadState;

use grin_rig::humanoid::{
    HumanoidAssets, HumanoidBuild, HumanoidBundle, HumanoidDominantHand,
};
use grin_util::event::Spawnable;

pub struct GrinPlugin;

impl Plugin for GrinPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<GrinSpawnEvent>()
            .add_plugins(GenericHumanoidCharacterPlugin::<Grin>::default())
            .add_systems(
                OnEnter(AssetLoadState::Success),
                spawn.in_set(CharacterSet::Spawn),
            );
    }
}

#[derive(Event, Clone, Default)]
pub struct GrinSpawnEvent;

#[derive(Component, Default)]
pub struct Grin;

impl Character for Grin {
    type StartItem = grin_item::sledge::Sledge;
}

impl Spawnable for Grin {
    type Event = GrinSpawnEvent;
}

pub fn spawn(
    mut commands: Commands,
    assets: Res<AvatarAssets>,
    hum_assets: Res<HumanoidAssets>,
    mut events: EventReader<<Grin as Spawnable>::Event>,
) {
    for _ in events.iter() {
        commands.spawn((
            Grin::default(),
            PlayerCharacter,
            HumanoidBundle {
                skeleton_gltf: hum_assets.skeleton.clone(),
                face: assets.face_grin.clone().into(),
                build: HumanoidBuild::Male,
                dominant_hand: HumanoidDominantHand::Right,
                spatial: SpatialBundle::from_transform(Transform::from_xyz(0.0, 1E-2, 0.0)),
                ..Default::default()
            },
        ));
    }
}
