use bevy::prelude::*;
use bevy_rapier3d::prelude::{CollisionGroups, Group};

use crate::{
    asset::AssetLoadState,
    character::{Character, CharacterSet, CharacterSpawnEvent, PlayerCharacter},
    collisions::{CollisionGroupExt, CollisionGroupsExt},
    damage::{Dead, Health, HealthBundle},
    humanoid::{Humanoid, HumanoidAssets, HumanoidBundle, HumanoidPartType},
    item::{smg::SMG, Active, Aiming, Equipped, Item, Target},
    time::Rewind,
};

use super::{
    movement::{move_to_target, CircularVelocity, MoveTarget, MovementBundle, PathBehavior},
    propagate_item_target, propagate_move_target, set_closest_target,
};

#[derive(SystemSet, Hash, Debug, Eq, PartialEq, Copy, Clone)]
pub enum DummySet {
    Setup,
    Propagate,
    Act,
}

pub struct DummyPlugin;

impl Plugin for DummyPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<CharacterSpawnEvent<Dummy>>()
            .configure_sets(
                Update,
                (DummySet::Setup, DummySet::Propagate, DummySet::Act).chain(),
            )
            .add_systems(
                Update,
                (
                    spawn.in_set(CharacterSet::Spawn),
                    init_humanoid.in_set(CharacterSet::Spawn),
                    set_closest_target::<Dummy, PlayerCharacter>.in_set(DummySet::Setup),
                    propagate_move_target::<Dummy>.in_set(DummySet::Propagate),
                    propagate_item_target::<Dummy>.in_set(DummySet::Propagate),
                    move_to_target::<Dummy>.in_set(DummySet::Act),
                    //fire.in_set(DummySet::Act),
                )
                    .run_if(in_state(AssetLoadState::Success)),
            );
    }
}

#[derive(Component, Default)]
pub struct Dummy;

impl Character for Dummy {
    type StartItem = SMG;
}

type SMGSpawnEvent = <<Dummy as Character>::StartItem as Item>::SpawnEvent;

pub fn spawn<'w, 's>(
    mut commands: Commands<'w, 's>,
    hum_assets: Res<HumanoidAssets>,
    mut events: EventReader<CharacterSpawnEvent<Dummy>>,
) {
    for _ in events.iter() {
        commands.spawn((
            DummyUninit,
            Target::default(),
            Equipped::default(),
            HealthBundle {
                health: Health(100.0),
                ..Default::default()
            },
            MovementBundle {
                path_behavior: PathBehavior::Strafe {
                    radial_velocity: 0.0,
                    circular_velocity: CircularVelocity::Linear(1.0),
                },
                target: MoveTarget::default(),
            },
            CollisionGroups::from_group_default(Group::ENEMY),
            HumanoidBundle {
                skeleton_gltf: hum_assets.skeleton.clone(),
                transform: Transform::from_xyz(10.0, 0.0, 0.0),
                ..Default::default()
            },
        ));
    }
}

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct DummyUninit;

pub fn init_humanoid(
    mut commands: Commands,
    humanoid_query: Query<(Entity, &Humanoid), With<DummyUninit>>,
    mut weapon_events: EventWriter<SMGSpawnEvent>,
) {
    let Ok((e_humanoid, humanoid)) = humanoid_query.get_single() else {
        return;
    };

    commands
        .entity(e_humanoid)
        .remove::<DummyUninit>()
        .insert(Dummy::default());

    for e_part in humanoid.parts(HumanoidPartType::HITBOX) {
        commands
            .entity(e_part)
            .insert(CollisionGroups::from_group_default(Group::ENEMY));
    }

    weapon_events.send(SMGSpawnEvent::new(e_humanoid));
}

pub fn fire(
    mut commands: Commands,
    time: Res<Time>,
    dummy_query: Query<&Equipped, (With<Dummy>, Without<Rewind>, Without<Dead>)>,
    weapon_query: Query<Entity>,
) {
    for Equipped(equipped) in dummy_query.iter() {
        for item in equipped {
            let e_item = weapon_query.get(*item).unwrap();
            if (time.elapsed_seconds() / 5.0) as u32 % 2 == 0 {
                commands.entity(e_item).insert((Active, Aiming));
            } else {
                commands.entity(e_item).remove::<(Active, Aiming)>();
            }
        }
    }
}
