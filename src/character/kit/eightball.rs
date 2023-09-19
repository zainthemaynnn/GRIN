use bevy::{prelude::*, render::view::RenderLayers};
use bevy_rapier3d::prelude::*;

use crate::{
    asset::AssetLoadState,
    character::{AvatarAssets, AvatarLoadEvent, Character, CharacterSet, Player, PlayerCharacter},
    collider,
    humanoid::{Humanoid, HumanoidAssets, HumanoidBuild, HumanoidBundle, HumanoidDominantHand},
    item::{smg::SMG, Item},
    physics::CollisionGroupsExt,
    render::RenderLayer,
    util::event::Spawnable,
};

pub struct EightBallPlugin;

impl Plugin for EightBallPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<EightBallSpawnEvent>()
            .add_systems(
                OnEnter(AssetLoadState::Success),
                spawn.in_set(CharacterSet::Spawn),
            )
            .add_systems(Update, init_humanoid.in_set(CharacterSet::Init));
    }
}

#[derive(Event, Clone, Default)]
pub struct EightBallSpawnEvent;

#[derive(Component, Default)]
pub struct EightBall;

#[derive(Component, Default)]
pub struct EightBallUninit;

impl Character for EightBall {
    type StartItem = SMG;
}

impl Spawnable for EightBall {
    type Event = EightBallSpawnEvent;
}

type ItemSpawnEvent = <<EightBall as Character>::StartItem as Item>::SpawnEvent;

pub fn spawn(
    mut commands: Commands,
    assets: Res<AvatarAssets>,
    hum_assets: Res<HumanoidAssets>,
    mut events: EventReader<<EightBall as Spawnable>::Event>,
) {
    dbg!("echeck");
    for _ in events.iter() {
        dbg!("event");
        commands.spawn((
            PlayerCharacter,
            HumanoidBundle {
                skeleton_gltf: hum_assets.skeleton.clone(),
                face: assets.face_smirk.clone().into(),
                build: HumanoidBuild::Male,
                dominant_hand: HumanoidDominantHand::Right,
                spatial: SpatialBundle::from_transform(Transform::from_xyz(0.0, 1E-2, 0.0)),
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
            Player,
            MaterialMeshBundle {
                mesh: assets.pizza_shades.clone(),
                material: assets.matte_shades.clone(),
                transform: Transform::from_xyz(0.0, 0.0, -0.525),
                ..Default::default()
            },
            RenderLayers::from_layers(&[RenderLayer::STANDARD as u8, RenderLayer::AVATAR as u8]),
            collider!(meshes, &assets.pizza_shades),
            RigidBody::Fixed,
            CollisionGroups::from_group_default(Group::NONE),
        ))
        .set_parent(humanoid.head);

    commands
        .entity(e_humanoid)
        .insert(EightBall::default())
        .remove::<EightBallUninit>();

    loaded_events.send_default();

    weapon_events.send(ItemSpawnEvent::new(e_humanoid));
}
