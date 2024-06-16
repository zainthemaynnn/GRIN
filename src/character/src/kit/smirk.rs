use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use grin_asset::AssetLoadState;
use grin_damage::hitbox::{GltfHitboxAutoGenTarget, HitboxManager, Hurtboxes};
use grin_render::sketched::SketchMaterial;
use grin_rig::{
    humanoid::{HumanoidBuild, HumanoidBundle, HumanoidDominantHand},
    Idle,
};
use grin_util::event::Spawnable;

use crate::{Character, CharacterSet, GenericHumanoidCharacterPlugin, PlayerCharacter};

pub struct SmirkPlugin;

impl Plugin for SmirkPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<SmirkSpawnEvent>()
            .configure_loading_state(
                LoadingStateConfig::new(AssetLoadState::Loading).load_collection::<SmirkAssets>(),
            )
            .add_plugins(GenericHumanoidCharacterPlugin::<Smirk>::default())
            .add_systems(
                OnEnter(AssetLoadState::Success),
                spawn.in_set(CharacterSet::Spawn),
            )
            .add_systems(Update, init_humanoid.in_set(CharacterSet::Init));
    }
}

#[derive(Resource, AssetCollection)]
pub struct SmirkAssets {
    #[asset(key = "mat.smirk")]
    pub face: Handle<SketchMaterial>,
    #[asset(key = "rig.smirk")]
    pub rig: Handle<Scene>,
    #[asset(key = "anim.idle")]
    pub idle: Handle<AnimationClip>,
}

#[derive(Event, Clone, Default)]
pub struct SmirkSpawnEvent;

#[derive(Component, Default)]
pub struct Smirk;

#[derive(Component, Default)]
pub struct SmirkUninit;

impl Character for Smirk {
    type StartItem = grin_item::library::fist::Fist;
}

impl Spawnable for Smirk {
    type Event = SmirkSpawnEvent;
}

pub fn spawn(
    mut commands: Commands,
    assets: Res<SmirkAssets>,
    mut events: EventReader<<Smirk as Spawnable>::Event>,
) {
    for _ in events.read() {
        commands.spawn((
            SmirkUninit,
            PlayerCharacter,
            HumanoidBundle {
                rig: assets.rig.clone(),
                face: assets.face.clone().into(),
                build: HumanoidBuild::Male,
                dominant_hand: HumanoidDominantHand::Right,
                spatial: SpatialBundle::from_transform(Transform::from_xyz(0.0, 1E-2, 0.0)),
                ..Default::default()
            },
            Idle {
                clip: assets.idle.clone(),
            },
            HitboxManager::<Hurtboxes>::default(),
            GltfHitboxAutoGenTarget::Here,
        ));
    }
}

pub fn init_humanoid(mut commands: Commands, humanoid_query: Query<Entity, With<SmirkUninit>>) {
    let Ok(e_humanoid) = humanoid_query.get_single() else {
        return;
    };

    commands
        .entity(e_humanoid)
        .insert(Smirk::default())
        .remove::<SmirkUninit>();
}
