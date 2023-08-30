use bevy::prelude::*;
use bevy_landmass::Agent;
use bevy_rapier3d::prelude::*;

use crate::{
    character::PlayerCharacter,
    damage::{
        projectiles::{BulletProjectile, ProjectileBundle, ProjectileColor},
        Damage, DamageVariant, Dead,
    },
    humanoid::{Humanoid, HumanoidAssets, HumanoidBundle, HUMANOID_RADIUS},
    item::Target,
    map::NavMesh,
    physics::PhysicsTime,
    time::Rewind,
    util::{
        distr,
        event::Spawnable,
        vectors::{self, Vec3Ext},
    },
};

use super::{
    configure_humanoid_physics,
    movement::{match_desired_velocity, propagate_attack_target_to_agent_target},
    propagate_attack_target_to_weapon_target, set_closest_attack_target, AISet, EnemyAgentBundle,
};

pub struct DummyPlugin;

impl Plugin for DummyPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<DummySpawnEvent>().add_systems(
            Update,
            (
                spawn.in_set(AISet::Spawn),
                configure_humanoid_physics::<Dummy>.in_set(AISet::Spawn),
                set_closest_attack_target::<Dummy, PlayerCharacter>.in_set(AISet::Target),
                propagate_attack_target_to_weapon_target::<Dummy>.in_set(AISet::ActionStart),
                propagate_attack_target_to_agent_target::<Dummy>.in_set(AISet::ActionStart),
                match_desired_velocity::<Dummy>.in_set(AISet::Act),
                fire.in_set(AISet::Act),
            ),
        );
    }
}

#[derive(Component, Default)]
pub struct Dummy;

impl Spawnable for Dummy {
    type Event = DummySpawnEvent;
}

#[derive(Event, Clone, Default)]
pub struct DummySpawnEvent {
    pub transform: Transform,
}

pub fn spawn(
    mut commands: Commands,
    hum_assets: Res<HumanoidAssets>,
    nav_mesh: Res<NavMesh>,
    mut events: EventReader<DummySpawnEvent>,
) {
    for DummySpawnEvent { transform } in events.iter() {
        commands.spawn((
            Dummy,
            Target::default(),
            ShotCooldown::default(),
            HumanoidBundle {
                skeleton_gltf: hum_assets.skeleton.clone(),
                spatial: SpatialBundle::from_transform(transform.clone()),
                ..Default::default()
            },
            EnemyAgentBundle {
                agent: Agent {
                    radius: HUMANOID_RADIUS,
                    max_velocity: 2.0,
                },
                ..EnemyAgentBundle::from_archipelago(nav_mesh.archipelago)
            },
        ));
    }
}

#[derive(Component, Default)]
pub struct ShotCooldown(pub f32);

pub fn fire(
    mut commands: Commands,
    time: Res<PhysicsTime>,
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
        cooldown.0 += time.0.delta_seconds();
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
                            transform: bullet_transform.with_scale(Vec3::splat(0.5)),
                            velocity: Velocity::linear(bullet_transform.forward() * 10.0),
                            ccd: Ccd::enabled(),
                            ..ProjectileBundle::enemy_default()
                        },
                    )
                },
            ),
        );
    }
}
