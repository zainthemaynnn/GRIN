use crate::{
    asset::AssetLoadState,
    collider,
    humanoid::{
        Humanoid, HumanoidAssets, HumanoidBuild, HumanoidBundle,
        HumanoidDominantHand,
    },
    item::{smg::SMG, Item, sledge::Sledge},
};

use super::{
    AvatarAssets, AvatarLoadEvent, AvatarSimulationBundle, Character, CharacterSet,
    CharacterSpawnEvent, PlayerCharacter,
};
use bevy::prelude::*;

pub struct EightBallPlugin;

impl Plugin for EightBallPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<CharacterSpawnEvent<EightBall>>()
            .add_system(
                spawn
                    .after(<EightBall as Character>::spawn)
                    .in_set(CharacterSet::Spawn)
                    .in_schedule(OnEnter(AssetLoadState::Success)),
            )
            .add_system(init_humanoid.in_set(CharacterSet::Init));
    }
}

#[derive(Component, Default)]
pub struct EightBallUninit;

#[derive(Component, Default)]
pub struct EightBall;

impl Character for EightBall {
    type StartItem = Sledge;
}

type ItemSpawnEvent = <<EightBall as Character>::StartItem as Item>::SpawnEvent;

pub fn spawn(
    mut commands: Commands,
    assets: Res<AvatarAssets>,
    hum_assets: Res<HumanoidAssets>,
    mut events: EventReader<CharacterSpawnEvent<EightBall>>,
) {
    for _ in events.iter() {
        commands
            .spawn((
                PlayerCharacter,
                HumanoidBundle {
                    skeleton_gltf: hum_assets.skeleton.clone(),
                    face: assets.face_smirk.clone().into(),
                    build: HumanoidBuild::Male,
                    dominant_hand: HumanoidDominantHand::Right,
                    transform: Transform::from_xyz(0.0, 1E-2, 0.0),
                    ..Default::default()
                },
                EightBallUninit,
            ));
    }
}

pub fn init_humanoid(
    mut commands: Commands,
    assets: Res<AvatarAssets>,
    meshes: Res<Assets<Mesh>>,
    humanoid_query: Query<(Entity, &Humanoid), With<EightBallUninit>>,
    mut loaded_events: EventWriter<AvatarLoadEvent>,
    mut weapon_events: EventWriter<ItemSpawnEvent>,
) {
    let Ok((e_humanoid, humanoid)) = humanoid_query.get_single() else {
        return;
    };

    commands
        .spawn((
            MaterialMeshBundle {
                mesh: assets.pizza_shades.clone(),
                material: assets.matte_shades.clone(),
                transform: Transform::from_xyz(0.0, 0.0, -0.525),
                ..Default::default()
            },
            AvatarSimulationBundle::default(),
            collider!(meshes, &assets.pizza_shades),
        ))
        .set_parent(humanoid.head);

    commands
        .entity(e_humanoid)
        .remove::<EightBallUninit>()
        .insert(EightBall::default());

    loaded_events.send_default();

    weapon_events.send(ItemSpawnEvent::new(e_humanoid));
}
