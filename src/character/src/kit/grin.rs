use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_rapier3d::prelude::*;
use grin_asset::AssetLoadState;
use grin_input::camera::LookInfo;
use grin_physics::{CollisionGroupExt, CollisionGroupsExt};
use grin_render::sketched::SketchMaterial;
use grin_rig::humanoid::{Humanoid, HumanoidBuild, HumanoidBundle, HumanoidDominantHand};
use grin_time::scaling::TimeScale;
use grin_util::{event::Spawnable, query::PotentialAncestorIter};

use crate::{Character, CharacterSet, GenericHumanoidCharacterPlugin, PlayerCharacter};

pub struct GrinPlugin;

impl Plugin for GrinPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<GrinSpawnEvent>()
            .add_collection_to_loading_state::<_, GrinAssets>(AssetLoadState::Loading)
            .add_plugins(GenericHumanoidCharacterPlugin::<Grin>::default())
            .add_systems(
                OnEnter(AssetLoadState::Success),
                spawn.in_set(CharacterSet::Spawn),
            )
            .add_systems(Update, init_humanoid.in_set(CharacterSet::Init))
            .add_systems(Update, freeze);
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

pub fn freeze(
    mut commands: Commands,
    keys: Res<Input<KeyCode>>,
    look_info: Res<LookInfo>,
    rapier_context: Res<RapierContext>,
    mut time_query: Query<(&mut TimeScale, Option<&ManualFreeze>)>,
    parent_query: Query<&Parent, ()>,
    freezeable_query: Query<(), With<FreezeTargettable>>,
) {
    if !keys.just_pressed(KeyCode::F) {
        return;
    };

    rapier_context.intersections_with_ray(
        look_info.mouse_ray.origin,
        look_info.mouse_ray.direction,
        bevy_rapier3d::prelude::Real::MAX,
        false,
        // TODO?: will probably need a separate dedicated collision group at some point
        QueryFilter::new().groups(CollisionGroups::from_group_default(
            Group::PLAYER_PROJECTILE,
        )),
        |e_hit, _| {
            commands.entity(e_hit).log_components();

            let Some(e_target) =
                PotentialAncestorIter::new(&parent_query, &freezeable_query, e_hit).next()
            else {
                return false;
            };

            let Ok((mut time_scale, manual_freeze)) = time_query.get_mut(e_target) else {
                return false;
            };

            if manual_freeze.is_some() {
                commands.entity(e_target).remove::<ManualFreeze>();
                if time_scale.unscale_by(0.0).is_err() {
                    warn!("Disabled manual time stop while time was already resumed.");
                }
            } else {
                commands.entity(e_target).insert(ManualFreeze);
                time_scale.scale_by(0.0);
            }

            true
        },
    );
}
