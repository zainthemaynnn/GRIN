use bevy::prelude::*;
use bevy_enum_filter::prelude::*;
use bevy_landmass::Agent;
use bevy_rapier3d::prelude::*;
use grin_derive::Cooldown;

use crate::{
    ai::bt::tree::CompositeNode,
    bt,
    character::PlayerCharacter,
    damage::{
        projectiles::{BulletProjectile, ProjectileBundle, ProjectileColor},
        Damage, DamageVariant, Dead,
    },
    humanoid::{Humanoid, HumanoidAssets, HumanoidBundle, HUMANOID_RADIUS},
    map::NavMesh,
    time::Rewind,
    util::{
        distr,
        event::Spawnable,
        vectors::{self, Vec3Ext},
    },
};

use super::{
    bt::{AiModel, BehaviorIteration, BehaviorSet, Brain, EnumBehaviorPlugin, Verdict},
    configure_humanoid_physics,
    movement::{match_desired_velocity, propagate_attack_target_to_agent_target, AttackTarget},
    protective_cooldown, set_closest_attack_target, AiSet, EnemyAgentBundle,
};

#[derive(Component, Cooldown)]
#[cooldown(duration = 1.0)]
pub struct ShotCooldown(pub Timer);

pub struct DummyPlugin;

impl Plugin for DummyPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<DummySpawnEvent>()
            .add_plugins(EnumBehaviorPlugin::<DummyAi>::default())
            .insert_resource(AiModel {
                bt: bt! {
                    Composite(CompositeNode::Sequence) {
                        Leaf(DummyAi::Track),
                        Leaf(DummyAi::Target),
                        Composite(CompositeNode::Selector) {
                            Composite(CompositeNode::Sequence) {
                                Leaf(DummyAi::FireCheck),
                                Leaf(DummyAi::Fire), 
                            },
                            Leaf(DummyAi::Chase),
                        },
                    },
                },
            })
            .add_systems(
                Update,
                (spawn, configure_humanoid_physics::<Dummy>).in_set(AiSet::Spawn),
            )
            .add_systems(
                BehaviorIteration,
                (
                    set_closest_attack_target::<Dummy, Enum!(DummyAi::Track), PlayerCharacter>,
                    propagate_attack_target_to_agent_target::<Dummy, Enum!(DummyAi::Target)>,
                    protective_cooldown::<Dummy, Enum!(DummyAi::FireCheck), ShotCooldown>,
                    match_desired_velocity::<Dummy, Enum!(DummyAi::Chase)>,
                    fire::<Dummy, Enum!(DummyAi::Fire)>,
                )
                    .in_set(BehaviorSet::Act),
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

#[derive(Component, EnumFilter, Clone, Copy, Debug, Default)]
pub enum DummyAi {
    #[default]
    Empty,
    Track,
    Target,
    FireCheck,
    Fire,
    Chase,
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
            ShotCooldown::default(),
            HumanoidBundle {
                skeleton_gltf: hum_assets.skeleton.clone(),
                spatial: SpatialBundle::from_transform(transform.clone()),
                ..Default::default()
            },
            EnemyAgentBundle::<DummyAi> {
                agent: Agent {
                    radius: HUMANOID_RADIUS,
                    max_velocity: 2.0,
                },
                ..EnemyAgentBundle::from_archipelago(nav_mesh.archipelago)
            },
        ));
    }
}

pub fn fire<T: Component, A: Component>(
    mut commands: Commands,
    mut agent_query: Query<
        (Entity, &mut Brain, &Humanoid, &AttackTarget),
        (With<T>, With<A>, Without<Rewind>, Without<Dead>),
    >,
    g_transform_query: Query<&GlobalTransform>,
) {
    for (e_agent, mut brain, humanoid, AttackTarget(e_target)) in agent_query.iter_mut() {
        let (origin, target) = (
            g_transform_query.get(humanoid.dominant_hand()).unwrap(),
            g_transform_query.get(*e_target).unwrap(),
        );
        let bullet_transform = Transform::from_translation(origin.translation())
            .looking_at(target.translation().with_y(origin.translation().y), Vec3::Y);

        commands.spawn_batch(
            vectors::centered_arc(
                bullet_transform.forward(),
                Vec3::Y,
                4,
                30.0_f32.to_radians(),
                &distr::linear,
            )
            .map(move |dir| {
                let bullet_transform =
                    bullet_transform.looking_to(dir, dir.any_orthogonal_vector());
                (
                    BulletProjectile,
                    ProjectileBundle {
                        color: ProjectileColor::Red,
                        damage: Damage {
                            ty: DamageVariant::Ballistic,
                            value: 5.0,
                            source: Some(e_agent),
                        },
                        transform: bullet_transform.with_scale(Vec3::splat(0.5)),
                        velocity: Velocity::linear(bullet_transform.forward() * 10.0),
                        ccd: Ccd::enabled(),
                        ..ProjectileBundle::enemy_default()
                    },
                )
            }),
        );

        brain.write_verdict(Verdict::Success);
    }
}
