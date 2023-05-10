use crate::{
    asset::AssetLoadState,
    humanoid::{HumanoidAssets, HumanoidBuilder},
    item::{smg::SMG, Item},
};

use super::{AvatarAssets, AvatarSimulationBundle, Character, CharacterSet, CharacterSpawnEvent};
use bevy::prelude::*;

pub struct EightBallPlugin;

impl Plugin for EightBallPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<CharacterSpawnEvent<EightBall>>()
            .add_system(
                spawn
                    .in_set(CharacterSet::Spawn)
                    .in_schedule(OnEnter(AssetLoadState::Success)),
            );
    }
}

#[derive(Component, Default)]
pub struct EightBall;

impl Character for EightBall {
    type StartItem = SMG;
}

type ItemSpawnEvent = <<EightBall as Character>::StartItem as Item>::SpawnEvent;

pub fn spawn(
    mut commands: Commands,
    hum_assets: Res<HumanoidAssets>,
    meshes: Res<Assets<Mesh>>,
    assets: Res<AvatarAssets>,
    mut events: EventReader<CharacterSpawnEvent<EightBall>>,
    mut weapon_events: EventWriter<ItemSpawnEvent>,
) {
    for _ in events.iter() {
        let mut humanoid = HumanoidBuilder::new_player(&mut commands, &hum_assets, &meshes);
        let shades = commands
            .spawn((
                MaterialMeshBundle {
                    mesh: assets.pizza_shades.clone(),
                    material: assets.matte_shades.clone(),
                    transform: Transform::from_xyz(0.0, 0.0, -0.525),
                    ..Default::default()
                },
                AvatarSimulationBundle::default(),
            ))
            .id();
        commands
            .get_or_spawn(humanoid.head)
            .push_children(&[shades]);
        commands
            .get_or_spawn(humanoid.body)
            .insert(EightBall::default());
        humanoid
            .with_face(assets.face_smirk.clone())
            .build(&mut commands);
        weapon_events.send(ItemSpawnEvent::new(humanoid.body));
    }
}
