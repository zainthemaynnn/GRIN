use bevy::prelude::*;
use bevy_rapier3d::prelude::{Ccd, CollisionGroups, Group, Velocity};

use crate::{
    asset::AssetLoadState,
    character::PlayerCharacter,
    damage::{
        projectiles::{BulletProjectile, ProjectileBundle, ProjectileColor},
        Damage, DamageBuffer, DamageVariant, Dead, Health, HealthBundle,
    },
    humanoid::{Humanoid, HumanoidAssets, HumanoidBundle, HumanoidPartType},
    item::Target,
    physics::{CollisionGroupExt, CollisionGroupsExt},
    time::Rewind,
    util::{
        distr,
        vectors::{self, Vec3Ext},
    },
};

use super::{
    movement::{move_to_target, CircularVelocity, MoveTarget, MovementBundle, PathBehavior},
    propagate_move_target, set_closest_target,
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
pub struct DummySpawnEvent {
    pub transform: Transform,
}

impl Dummy {
    // how do I return impl IntoSystem??? too confusing???
    pub fn spawn_at(transform: Transform) -> impl Fn(EventWriter<DummySpawnEvent>) {
        move |mut events: EventWriter<DummySpawnEvent>| {
            events.send(DummySpawnEvent { transform });
        }
    }
}

pub fn spawn(
    mut commands: Commands,
    hum_assets: Res<HumanoidAssets>,
    mut events: EventReader<DummySpawnEvent>,
) {
    for DummySpawnEvent { transform } in events.iter() {
        commands.spawn((
            DummyUninit,
            Target::default(),
            ShotCooldown::default(),
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
                spatial: SpatialBundle::from_transform(transform.clone()),
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
    for (e_humanoid, humanoid) in humanoid_query.iter() {
        commands.entity(e_humanoid).remove::<DummyUninit>().insert((
            Dummy::default(),
            HealthBundle {
                health: Health(100.0),
                ..Default::default()
            },
        ));

        for e_part in humanoid.parts(HumanoidPartType::HITBOX) {
            commands.entity(e_part).insert((
                DamageBuffer::default(),
                CollisionGroups::from_group_default(Group::ENEMY),
            ));
        }

        for e_part in humanoid.parts(HumanoidPartType::HANDS) {
            commands
                .entity(e_part)
                .insert(CollisionGroups::new(Group::ENEMY, Group::empty()));
        }
    }
}

#[derive(Component, Default)]
pub struct ShotCooldown(pub f32);

pub fn fire(
    mut commands: Commands,
    time: Res<Time>,
    mut dummy_query: Query<
        (Entity, &Humanoid, &Target, &mut ShotCooldown),
        (With<Dummy>, Without<Rewind>, Without<Dead>),
    >,
    transform_query: Query<&GlobalTransform>,
) {
    for (
        entity,
        humanoid,
        Target {
            transform: target, ..
        },
        mut cooldown,
    ) in dummy_query.iter_mut()
    {
        cooldown.0 += time.delta_seconds();
        if cooldown.0 < 4.0 {
            continue;
        }

        cooldown.0 -= 4.0;
        let origin = transform_query.get(humanoid.dominant_hand()).unwrap();
        let fwd = (target.translation - origin.translation())
            .xz_flat()
            .normalize();
        let bullet_transform = Transform::from_translation(origin.translation());

        commands.spawn_batch(
            vectors::centered_arc(fwd, Vec3::Y, 4, 30.0_f32.to_radians(), &distr::linear).map(
                move |dir| {
                    let bullet_transform =
                        bullet_transform.looking_to(dir, dir.any_orthogonal_vector());
                    (
                        BulletProjectile,
                        ProjectileBundle {
                            color: ProjectileColor::Red,
                            damage: Damage {
                                ty: DamageVariant::Ballistic,
                                value: 5.0,
                                source: Some(entity),
                            },
                            velocity: Velocity::linear(bullet_transform.forward() * 10.0),
                            collision_groups: CollisionGroups::from_group_default(
                                Group::ENEMY_PROJECTILE,
                            ),
                            spatial: SpatialBundle::from_transform(
                                bullet_transform.with_scale(Vec3::splat(0.5)),
                            ),
                            ccd: Ccd::enabled(),
                            ..Default::default()
                        },
                    )
                },
            ),
        );
    }
}
