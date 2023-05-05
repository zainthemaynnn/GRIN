use crate::{
    humanoid::{HumanoidAssets, HumanoidBuilder},
    weapon::smg::SMG,
};

use super::{AvatarAssets, AvatarSimulationBundle, Character, CharacterSpawnEvent};
use bevy::prelude::*;

pub struct EightBallPlugin;

impl Plugin for EightBallPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<CharacterSpawnEvent<EightBall>>();
    }
}

#[derive(Component, Default)]
pub struct EightBall;

impl Character for EightBall {
    type StartItem = SMG;
}

pub fn spawn(
    mut commands: Commands,
    hum_assets: Res<HumanoidAssets>,
    meshes: Res<Assets<Mesh>>,
    assets: Res<AvatarAssets>,
    mut events: EventReader<CharacterSpawnEvent<EightBall>>,
) {
    for _ in events.iter() {
        let shades = commands
            .spawn((
                EightBall::default(),
                MaterialMeshBundle {
                    mesh: assets.pizza_shades.clone(),
                    material: assets.matte_shades.clone(),
                    transform: Transform::from_xyz(0.0, 0.0, -0.525),
                    ..Default::default()
                },
                AvatarSimulationBundle::default(),
            ))
            .id();
        let mut humanoid = HumanoidBuilder::new_player(&mut commands, &hum_assets, &meshes);
        commands
            .get_or_spawn(humanoid.head)
            .push_children(&[shades]);
        humanoid
            .with_face(assets.face_smirk.clone())
            .build(&mut commands);
    }
}
