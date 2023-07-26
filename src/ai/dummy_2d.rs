use bevy::prelude::*;
use bevy_rapier3d::prelude::{Ccd, CollisionGroups, Group, Velocity};

use crate::{
    asset::AssetLoadState,
    character::PlayerCharacter,
    collisions::{CollisionGroupExt, CollisionGroupsExt},
    damage::{
        projectiles::{BulletProjectile, ProjectileBundle, ProjectileColor},
        Damage, DamageVariant, Dead, Health, HealthBundle,
    },
    humanoid::{Humanoid, HumanoidAssets, HumanoidBundle, HumanoidPartType},
    item::Target,
    time::Rewind,
};

use super::{
    movement::{move_to_target, CircularVelocity, MoveTarget, MovementBundle, PathBehavior},
    propagate_item_target, propagate_move_target, set_closest_target,
};

#[derive(SystemSet, Hash, Debug, Eq, PartialEq, Copy, Clone)]
pub enum DummySet {
    Spawn,
    Setup,
    Propagate,
    Act,
}

pub struct DummyPlugin;

impl Plugin for DummyPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<DummySpawnEvent>()
            .configure_sets(
                Update,
                (DummySet::Setup, DummySet::Propagate, DummySet::Act).chain(),
            )
            .add_systems(
                Update,
                (
                    spawn.in_set(DummySet::Spawn),
                    init_humanoid.in_set(DummySet::Spawn),
                    set_closest_target::<Dummy, PlayerCharacter>.in_set(DummySet::Setup),
                    propagate_move_target::<Dummy>.in_set(DummySet::Propagate),
                    propagate_item_target::<Dummy>.in_set(DummySet::Propagate),
                    move_to_target::<Dummy>.in_set(DummySet::Act),
                    fire.in_set(DummySet::Act),
                )
                    .run_if(in_state(AssetLoadState::Success)),
            );
    }
}

#[derive(Component, Default)]
pub struct Dummy;

#[derive(Event, Default)]
pub struct DummySpawnEvent;

impl Dummy {
    pub fn spawn(mut events: EventWriter<DummySpawnEvent>) {
        events.send_default();
    }
}

pub fn spawn(
    mut commands: Commands,
    hum_assets: Res<HumanoidAssets>,
    mut events: EventReader<DummySpawnEvent>,
) {
    for _ in events.iter() {
        commands.spawn((
            DummyUninit,
            Target::default(),
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

    for e_part in humanoid.parts(HumanoidPartType::HANDS) {
        commands
            .entity(e_part)
            .insert(CollisionGroups::new(Group::ENEMY, Group::empty()));
    }
}

pub fn fire(
    mut commands: Commands,
    time: Res<Time>,
    dummy_query: Query<(Entity, &Humanoid, &Target), (With<Dummy>, Without<Rewind>, Without<Dead>)>,
    transform_query: Query<&GlobalTransform>,
    mut cd: Local<f32>,
) {
    *cd += time.delta_seconds();
    for (
        entity,
        humanoid,
        Target {
            transform: target, ..
        },
    ) in dummy_query.iter()
    {
        if *cd < 1.0 {
            return;
        }

        *cd -= 1.0;
        let origin = transform_query.get(humanoid.dominant_hand()).unwrap();
        let fwd =
            ((target.translation - origin.translation()) * Vec3::new(1.0, 0.0, 1.0)).normalize();
        let bullet_transform = Transform::from_translation(origin.translation())
            .looking_to(fwd, fwd.any_orthogonal_vector());
        commands.spawn((
            BulletProjectile,
            ProjectileBundle {
                color: ProjectileColor::Red,
                damage: Damage {
                    ty: DamageVariant::Ballistic,
                    value: 5.0,
                    source: Some(entity),
                },
                velocity: Velocity::linear(bullet_transform.forward() * 10.0),
                collision_groups: CollisionGroups::from_group_default(Group::ENEMY_PROJECTILE),
                spatial: SpatialBundle::from_transform(
                    bullet_transform.with_scale(Vec3::splat(0.4)),
                ),
                ccd: Ccd::enabled(),
                ..Default::default()
            },
        ));
    }
}
