use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_enum_filter::prelude::*;
use bevy_landmass::Agent;
use bevy_rapier3d::prelude::*;
use grin_asset::AssetLoadState;
use grin_character::PlayerCharacter;
use grin_damage::{
    health::Dead,
    hit::{Damage, DamageVariant},
    projectiles::{BulletProjectile, ProjectileBundle, ProjectileColor},
};
use grin_derive::Cooldown;
use grin_map::MapData;
use grin_rig::humanoid::{Humanoid, HumanoidBundle, HUMANOID_RADIUS};
use grin_time::Rewind;
use grin_util::{event::Spawnable, vectors::Vec3Ext};

use super::{
    bt::{
        tree::CompositeNode, AiModel, BehaviorIteration, BehaviorSet, Brain, EnumBehaviorPlugin,
        Verdict,
    },
    configure_humanoid_physics,
    movement::{match_desired_velocity, propagate_attack_target_to_agent_target, AttackTarget},
    protective_cooldown, set_closest_attack_target, AiSet, EnemyAgentBundle,
};
use crate::bt;

#[derive(Component, Cooldown)]
#[cooldown(duration = 2.0)]
pub struct ShotCooldown(pub Timer);

pub struct DummyPlugin;

impl Plugin for DummyPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<DummySpawnEvent>()
            .configure_loading_state(
                LoadingStateConfig::new(AssetLoadState::Loading).load_collection::<DummyAssets>(),
            )
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
            .add_systems(Update, spawn.in_set(AiSet::Spawn))
            .add_systems(
                PreUpdate,
                configure_humanoid_physics::<Dummy>.in_set(AiSet::Load),
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

#[derive(Resource, AssetCollection)]
pub struct DummyAssets {
    #[asset(key = "rig.dummy")]
    pub rig: Handle<Scene>,
}

#[derive(Event, Clone, Default)]
pub struct DummySpawnEvent {
    pub transform: Transform,
}

impl Spawnable for Dummy {
    type Event = DummySpawnEvent;
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
    assets: Res<DummyAssets>,
    map_data: Res<MapData>,
    mut events: EventReader<DummySpawnEvent>,
) {
    for DummySpawnEvent { transform } in events.read() {
        commands.spawn((
            Dummy,
            ShotCooldown::default(),
            HumanoidBundle {
                rig: assets.rig.clone(),
                spatial: SpatialBundle::from_transform(transform.clone()),
                ..Default::default()
            },
            EnemyAgentBundle::<DummyAi> {
                agent: Agent {
                    radius: HUMANOID_RADIUS,
                    max_velocity: 2.0,
                },
                ..EnemyAgentBundle::from_archipelago(map_data.archipelago)
            },
            grin_character::kit::grin::FreezeTargettable,
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
        commands.spawn((
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
        ));

        brain.write_verdict(Verdict::Success);
    }
}
