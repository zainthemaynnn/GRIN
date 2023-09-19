use bevy::prelude::*;

use crate::{
    asset::AssetLoadState,
    character::{AvatarAssets, AvatarLoadEvent, Character, CharacterSet, PlayerCharacter},
    humanoid::{Humanoid, HumanoidAssets, HumanoidBuild, HumanoidBundle, HumanoidDominantHand},
    item::{sledge::Sledge, Item},
    util::event::Spawnable,
};

pub struct GrinPlugin;

impl Plugin for GrinPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<GrinSpawnEvent>()
            .add_systems(
                OnEnter(AssetLoadState::Success),
                spawn.in_set(CharacterSet::Spawn),
            )
            .add_systems(Update, init_humanoid.in_set(CharacterSet::Init));
    }
}

#[derive(Event, Clone, Default)]
pub struct GrinSpawnEvent;

#[derive(Component, Default)]
pub struct GrinUninit;

#[derive(Component, Default)]
pub struct Grin;

impl Character for Grin {
    type StartItem = Sledge;
}

impl Spawnable for Grin {
    type Event = GrinSpawnEvent;
}

type ItemSpawnEvent = <<Grin as Character>::StartItem as Item>::SpawnEvent;

pub fn spawn(
    mut commands: Commands,
    assets: Res<AvatarAssets>,
    hum_assets: Res<HumanoidAssets>,
    mut events: EventReader<<Grin as Spawnable>::Event>,
) {
    for _ in events.iter() {
        commands.spawn((
            PlayerCharacter,
            HumanoidBundle {
                skeleton_gltf: hum_assets.skeleton.clone(),
                face: assets.face_grin.clone().into(),
                build: HumanoidBuild::Male,
                dominant_hand: HumanoidDominantHand::Right,
                spatial: SpatialBundle::from_transform(Transform::from_xyz(0.0, 1E-2, 0.0)),
                ..Default::default()
            },
            GrinUninit,
        ));
    }
}

pub fn init_humanoid(
    mut commands: Commands,
    humanoid_query: Query<Entity, With<GrinUninit>>,
    mut loaded_events: EventWriter<AvatarLoadEvent>,
    mut weapon_events: EventWriter<ItemSpawnEvent>,
) {
    let Ok(e_humanoid) = humanoid_query.get_single() else {
        return;
    };

    commands
        .entity(e_humanoid)
        .insert(Grin::default())
        .remove::<GrinUninit>();

    loaded_events.send_default();

    weapon_events.send(ItemSpawnEvent::new(e_humanoid));
}
