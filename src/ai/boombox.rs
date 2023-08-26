use std::f32::consts::PI;

use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_landmass::Agent;
use bevy_rapier3d::prelude::*;

use crate::{
    asset::AssetLoadState,
    character::PlayerCharacter,
    damage::{
        projectiles::{BulletProjectile, ProjectileBundle, ProjectileColor},
        Damage, DamageVariant, Dead,
    },
    humanoid::{Humanoid, HumanoidBundle, HumanoidDominantHand, HUMANOID_RADIUS},
    item::Target,
    map::NavMesh,
    physics::{CollisionGroupExt, CollisionGroupsExt, ForceTimer},
    time::Rewind,
    util::{
        distr,
        event::Spawnable,
        query::gltf_path_search,
        vectors::{self, Vec3Ext},
    },
};

use super::{
    configure_humanoid_physics,
    movement::{match_desired_velocity, propagate_attack_target_to_agent_target},
    propagate_attack_target_to_weapon_target, set_closest_attack_target, AISet, EnemyAgentBundle,
};

pub struct BoomBoxPlugin;

impl Plugin for BoomBoxPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<BoomBoxSpawnEvent>()
            .add_collection_to_loading_state::<_, BoomBoxAssets>(AssetLoadState::Loading)
            .add_systems(
                Update,
                (
                    spawn.in_set(AISet::Spawn),
                    configure_humanoid_physics::<BoomBox>.in_set(AISet::Spawn),
                    init_humanoid.in_set(AISet::Spawn),
                    set_closest_attack_target::<BoomBox, PlayerCharacter>.in_set(AISet::Target),
                    propagate_attack_target_to_weapon_target::<BoomBox>.in_set(AISet::ActionStart),
                    propagate_attack_target_to_agent_target::<BoomBox>.in_set(AISet::ActionStart),
                    match_desired_velocity::<BoomBox>.in_set(AISet::Act),
                    fire.in_set(AISet::Act),
                ),
            );
    }
}

#[derive(Component, Default)]
pub struct BoomBox;

#[derive(Resource, AssetCollection)]
pub struct BoomBoxAssets {
    #[asset(key = "rig.boombox")]
    pub skeleton: Handle<Scene>,
    #[asset(key = "anim.idle.boombox.left")]
    pub idle_lt: Handle<AnimationClip>,
    #[asset(key = "anim.idle.boombox.right")]
    pub idle_rt: Handle<AnimationClip>,
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
    for BoomBoxSpawnEvent { transform } in events.iter() {
        commands.spawn((
            BoomBox,
            Target::default(),
            ShotCooldown::default(),
            HumanoidBundle {
                skeleton_gltf: assets.skeleton.clone(),
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

pub fn init_humanoid(
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
                HumanoidDominantHand::Left => assets.idle_rt.clone(),
                HumanoidDominantHand::Right => assets.idle_rt.clone(),
            })
            .repeat();

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

#[derive(Component, Default)]
pub struct ShotCooldown(pub f32);

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
                source: source.into(),
            },
            velocity: Velocity::linear(transform.forward() * BEGIN_SPEED),
            collision_groups: CollisionGroups::from_group_default(Group::ENEMY_PROJECTILE),
            spatial: SpatialBundle::from_transform(transform.with_scale(Vec3::splat(BULLET_SIZE))),
            mass_properties: ColliderMassProperties::Mass(1.0),
            ..Default::default()
        },
        ExternalForce {
            force: transform.forward() * DRAG,
            ..Default::default()
        },
        ForceTimer::from_seconds(DRAG_DURATION),
    )
}

pub fn fire(
    mut commands: Commands,
    time: Res<Time>,
    mut dummy_query: Query<
        (Entity, &Humanoid, &mut ShotCooldown),
        (With<BoomBox>, Without<Rewind>, Without<Dead>),
    >,
    transform_query: Query<&GlobalTransform>,
) {
    for (entity, humanoid, mut cooldown) in dummy_query.iter_mut() {
        cooldown.0 += time.delta_seconds();
        if cooldown.0 < 4.0 {
            continue;
        }

        cooldown.0 -= 4.0;
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
                    entity,
                    transform.looking_to(dir, dir.any_orthogonal_vector()),
                )
            }),
        );
    }
}
