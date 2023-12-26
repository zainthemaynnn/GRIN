use std::f32::consts::PI;

use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_enum_filter::prelude::*;
use bevy_landmass::Agent;
use bevy_rapier3d::prelude::*;
use grin_asset::AssetLoadState;
use grin_character::PlayerCharacter;
use grin_damage::{
    projectiles::{BulletProjectile, ProjectileBundle, ProjectileColor},
    Damage, DamageVariant, Dead,
};
use grin_map::NavMesh;
use grin_physics::ForceTimer;
use grin_rig::humanoid::{
    Humanoid, HumanoidBundle, HumanoidDominantHand, HUMANOID_RADIUS,
};
use grin_time::Rewind;
use grin_util::{
    distr,
    event::Spawnable,
    query::gltf_path_search,
    vectors::{self, Vec3Ext},
};

use super::{
    bt::{BehaviorIteration, BehaviorSet, Brain, Verdict},
    configure_humanoid_physics,
    dummy::{dummy_ai_filters, DummyAi, ShotCooldown},
    movement::{match_desired_velocity, propagate_attack_target_to_agent_target},
    protective_cooldown, set_closest_attack_target, AiSet, EnemyAgentBundle,
};

pub struct BoomBoxPlugin;

impl Plugin for BoomBoxPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<BoomBoxSpawnEvent>()
            .add_collection_to_loading_state::<_, BoomBoxAssets>(AssetLoadState::Loading)
            .add_systems(Update, spawn.in_set(AiSet::Spawn))
            .add_systems(
                PreUpdate,
                (load, configure_humanoid_physics::<BoomBox>).in_set(AiSet::Load),
            )
            .add_systems(
                BehaviorIteration,
                (
                    set_closest_attack_target::<BoomBox, Enum!(DummyAi::Track), PlayerCharacter>,
                    propagate_attack_target_to_agent_target::<BoomBox, Enum!(DummyAi::Target)>,
                    protective_cooldown::<BoomBox, Enum!(DummyAi::FireCheck), ShotCooldown>,
                    match_desired_velocity::<BoomBox, Enum!(DummyAi::Chase)>,
                    fire::<BoomBox, Enum!(DummyAi::Fire)>,
                )
                    .in_set(BehaviorSet::Act),
            );
    }
}

#[derive(Component, Default)]
pub struct BoomBox;

#[derive(Resource, AssetCollection)]
pub struct BoomBoxAssets {
    #[asset(key = "rig.boombox")]
    pub rig: Handle<Scene>,
    #[asset(key = "anim.tote.left")]
    pub idle_lt: Handle<AnimationClip>,
    #[asset(key = "anim.tote.right")]
    pub idle_rt: Handle<AnimationClip>,
    #[asset(key = "anim.headbang")]
    pub headbang: Handle<AnimationClip>,
}

#[derive(Event, Clone, Default)]
pub struct BoomBoxSpawnEvent {
    pub transform: Transform,
}

impl Spawnable for BoomBox {
    type Event = BoomBoxSpawnEvent;
}

pub fn spawn(
    mut commands: Commands,
    nav_mesh: Res<NavMesh>,
    mut events: EventReader<BoomBoxSpawnEvent>,
    assets: Res<BoomBoxAssets>,
) {
    for BoomBoxSpawnEvent { transform } in events.read() {
        commands.spawn((
            BoomBox,
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
                ..EnemyAgentBundle::from_archipelago(nav_mesh.archipelago)
            },
        ));
    }
}

pub fn load(
    mut commands: Commands,
    assets: Res<BoomBoxAssets>,
    humanoid_query: Query<(&Humanoid, &HumanoidDominantHand), (With<BoomBox>, Added<Humanoid>)>,
    mut animator_query: Query<&mut AnimationPlayer>,
    mut transform_query: Query<&mut Transform>,
    children_query: Query<&Children>,
    name_query: Query<&Name>,
) {
    // TODO: accessory colliders? that's a hell of a lot of boilerplate.
    // doable, but I don't want to add them until I decide for sure if they're needed.
    for (humanoid, dominant) in humanoid_query.iter() {
        let mut animator = animator_query.get_mut(humanoid.armature).unwrap();
        animator
            .play(match dominant {
                HumanoidDominantHand::Left => assets.idle_lt.clone(),
                HumanoidDominantHand::Right => assets.idle_rt.clone(),
            })
            .repeat();
        animator.play(assets.headbang.clone()).repeat();

        let e_boombox = gltf_path_search(
            &EntityPath {
                parts: vec!["BoomBox".into()],
            },
            humanoid.rhand,
            &children_query,
            &name_query,
        )
        .unwrap();
        commands
            .entity(e_boombox)
            .set_parent(humanoid.dominant_hand());

        // unlike most weapons, boombox is asymmetrical along global z,
        // so it needs to be flipped depending on the hand
        // TODO: this should be a component for humanoid accessories
        // so I don't have to do it manual
        transform_query.get_mut(e_boombox).unwrap().rotation *= match dominant {
            HumanoidDominantHand::Left => Quat::from_rotation_y(PI),
            HumanoidDominantHand::Right => Quat::default(),
        };
    }
}

pub const BULLET_SIZE: f32 = 0.5;
pub const END_SPEED: f32 = 3.0;
pub const DRAG_DURATION: f32 = 0.5;
pub const DRAG_DISTANCE: f32 = 8.0;

pub const BEGIN_SPEED: f32 = 2.0 * DRAG_DISTANCE / DRAG_DURATION - END_SPEED;
pub const DRAG: f32 = (END_SPEED - BEGIN_SPEED) / DRAG_DURATION;

pub fn create_bullet(source: Entity, transform: Transform) -> impl Bundle {
    (
        BulletProjectile,
        ProjectileBundle {
            color: ProjectileColor::Red,
            damage: Damage {
                ty: DamageVariant::Ballistic,
                value: 5.0,
                source: Some(source),
            },
            transform: transform.with_scale(Vec3::splat(BULLET_SIZE)),
            velocity: Velocity::linear(transform.forward() * BEGIN_SPEED),
            mass_properties: ColliderMassProperties::Mass(1.0),
            ..ProjectileBundle::enemy_default()
        },
        ExternalForce {
            force: transform.forward() * DRAG,
            ..Default::default()
        },
        ForceTimer::from_seconds(DRAG_DURATION),
    )
}

pub fn fire<T: Component, A: Component>(
    mut commands: Commands,
    mut agent_query: Query<
        (Entity, &mut Brain, &Humanoid),
        (With<T>, With<A>, Without<Rewind>, Without<Dead>),
    >,
    transform_query: Query<&GlobalTransform>,
) {
    for (e_agent, mut brain, humanoid) in agent_query.iter_mut() {
        let origin = transform_query.get(humanoid.dominant_hand()).unwrap();
        let transform = Transform::from_translation(origin.translation());

        commands.spawn_batch(
            vectors::circle(
                origin.forward().xz_flat().normalize_or_zero(),
                Vec3::Y,
                16,
                &distr::linear,
            )
            .map(move |dir| {
                create_bullet(
                    e_agent,
                    transform.looking_to(dir, dir.any_orthogonal_vector()),
                )
            }),
        );

        brain.write_verdict(Verdict::Success);
    }
}
