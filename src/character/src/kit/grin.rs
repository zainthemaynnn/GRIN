use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use grin_asset::AssetLoadState;
use grin_render::sketched::SketchMaterial;
use grin_rig::humanoid::{Humanoid, HumanoidBuild, HumanoidBundle, HumanoidDominantHand};
use grin_util::event::Spawnable;

use crate::{Character, CharacterSet, GenericHumanoidCharacterPlugin, PlayerCharacter};

pub struct GrinPlugin;

impl Plugin for GrinPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<GrinSpawnEvent>()
            .configure_loading_state(
                LoadingStateConfig::new(AssetLoadState::Loading).load_collection::<GrinAssets>(),
            )
            .add_plugins(GenericHumanoidCharacterPlugin::<Grin>::default())
            .add_systems(
                OnEnter(AssetLoadState::Success),
                spawn.in_set(CharacterSet::Spawn),
            )
            .add_systems(Update, init_humanoid.in_set(CharacterSet::Init));
    }
}

#[derive(Resource, AssetCollection)]
pub struct GrinAssets {
    #[asset(key = "mat.grin")]
    pub face: Handle<SketchMaterial>,
    #[asset(key = "rig.grin")]
    pub rig: Handle<Scene>,
}

#[derive(Event, Clone, Default)]
pub struct GrinSpawnEvent;

#[derive(Component, Default)]
pub struct Grin;

#[derive(Component)]
pub struct GrinUninit;

impl Character for Grin {
    type StartItem = grin_item::library::fist::Fist;
}

impl Spawnable for Grin {
    type Event = GrinSpawnEvent;
}

pub fn spawn(
    mut commands: Commands,
    assets: Res<GrinAssets>,
    mut events: EventReader<<Grin as Spawnable>::Event>,
) {
    for _ in events.read() {
        commands.spawn((
            GrinUninit,
            PlayerCharacter,
            HumanoidBundle {
                rig: assets.rig.clone(),
                face: assets.face.clone().into(),
                build: HumanoidBuild::Male,
                dominant_hand: HumanoidDominantHand::Right,
                spatial: SpatialBundle::from_transform(Transform::from_xyz(0.0, 1E-2, 0.0)),
                ..Default::default()
            },
        ));
    }
}

pub fn init_humanoid(
    mut commands: Commands,
    humanoid_query: Query<Entity, (With<GrinUninit>, With<Humanoid>)>,
) {
    let Ok(e_humanoid) = humanoid_query.get_single() else {
        return;
    };

    commands
        .entity(e_humanoid)
        .insert(Grin::default())
        .remove::<GrinUninit>();
}

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct ManualFreeze;

/// Whether this object can be frozen. It will also be treated as the "root" object when frozen.
#[derive(Component)]
pub struct FreezeTargettable;
