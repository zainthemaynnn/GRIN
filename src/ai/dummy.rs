use bevy::prelude::*;

use crate::{
    asset::AssetLoadState,
    character::{Character, CharacterSet, CharacterSpawnEvent, PlayerCharacter},
    humanoid::{HumanoidAssets, HumanoidBuilder},
    item::{smg::SMG, Active, Aiming, Equipped, Item, Target},
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
            .configure_sets((DummySet::Setup, DummySet::Propagate, DummySet::Act).chain())
            .add_systems(
                (
                    spawn.in_set(CharacterSet::Spawn),
                    set_closest_target::<Dummy, PlayerCharacter>.in_set(DummySet::Setup),
                    propagate_move_target::<Dummy>.in_set(DummySet::Propagate),
                    propagate_item_target::<Dummy>.in_set(DummySet::Propagate),
                    move_to_target::<Dummy>.in_set(DummySet::Act),
                    fire.in_set(DummySet::Act),
                )
                    .in_set(OnUpdate(AssetLoadState::Success)),
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
    assets: Res<HumanoidAssets>,
    meshes: Res<Assets<Mesh>>,
    mut events: EventReader<CharacterSpawnEvent<Dummy>>,
    mut weapon_events: EventWriter<SMGSpawnEvent>,
) {
    for _ in events.iter() {
        let mut humanoid = HumanoidBuilder::new(&mut commands, &assets, &meshes);
        commands.get_or_spawn(humanoid.body).insert((
            Dummy::default(),
            Target::default(),
            Equipped::default(),
            MovementBundle {
                path_behavior: PathBehavior::Strafe {
                    radial_velocity: 0.0,
                    circular_velocity: CircularVelocity::Linear(1.0),
                },
                target: MoveTarget::default(),
            },
        ));
        humanoid
            .with_transform(Transform::from_xyz(10.0, 0.0, 0.0))
            .build(&mut commands);
        weapon_events.send(SMGSpawnEvent::new(humanoid.body));
    }
}

fn fire(
    time: Res<Time>,
    dummy_query: Query<&Equipped, With<Dummy>>,
    mut weapon_query: Query<(&mut Active, &mut Aiming)>,
) {
    let set = (time.elapsed_seconds() / 5.0) as u32 % 2 == 0;
    for Equipped(equipped) in dummy_query.iter() {
        for item in equipped {
            let Ok((mut active, mut aiming)) = weapon_query.get_mut(*item) else {
                println!("Item missing `(Active, Aiming)` component.");
                continue;
            };
            *active = Active(set);
            *aiming = Aiming(set);
        }
    }
}
