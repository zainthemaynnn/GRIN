use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_landmass::{
    Agent, AgentBundle, AgentDesiredVelocity, AgentTarget, AgentVelocity, ArchipelagoRef,
    LandmassSystemSet,
};
use bevy_mod_outline::SetOutlineDepth;
use bevy_rapier3d::prelude::*;

use crate::{
    asset::AssetLoadState,
    character::PlayerCharacter,
    damage::{
        projectiles::{BulletProjectile, ProjectileBundle, ProjectileColor},
        Damage, DamageVariant, Dead,
    },
    humanoid::{Humanoid, HumanoidAssets, HumanoidBundle, HumanoidDominantHand, HUMANOID_RADIUS},
    item::Target,
    map::NavMesh,
    physics::{CollisionGroupExt, CollisionGroupsExt, ForceTimer},
    render::sketched::SketchMaterial,
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

pub struct BoomBoxPlugin;

impl Plugin for BoomBoxPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<BoomBoxSpawnEvent>()
            .add_collection_to_loading_state::<_, BoomBoxAssets>(AssetLoadState::Loading)
            .add_systems(
                Update,
                (
                    spawn.in_set(AISet::Spawn),
                    configure_humanoid_physics.in_set(AISet::Spawn),
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
    #[asset(key = "mesh.square_shades")]
    pub shades: Handle<Mesh>,
    #[asset(key = "mat.shades")]
    pub matte: Handle<SketchMaterial>,
    #[asset(key = "mesh.boombox")]
    pub boombox: Handle<Mesh>,
    #[asset(key = "mesh.headphones")]
    pub headphones: Handle<Mesh>,
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
    hum_assets: Res<HumanoidAssets>,
    nav_mesh: Res<NavMesh>,
    mut events: EventReader<BoomBoxSpawnEvent>,
) {
    for BoomBoxSpawnEvent { transform } in events.iter() {
        commands.spawn((
            BoomBox,
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

pub fn init_humanoid(
    mut commands: Commands,
    humanoid_query: Query<(Entity, &Humanoid, &HumanoidDominantHand), Added<Humanoid>>,
    mut animator_query: Query<&mut AnimationPlayer>,
    children_query: Query<&Children>,
    boombox_assets: Res<BoomBoxAssets>,
) {
    for (e_humanoid, humanoid, dominant) in humanoid_query.iter() {
        commands.entity(humanoid.head).with_children(|parent| {
            parent.spawn(MaterialMeshBundle {
                material: boombox_assets.matte.clone(),
                mesh: boombox_assets.shades.clone(),
                transform: Transform {
                    translation: Vec3::new(0.0, 0.0, -0.525),
                    rotation: match dominant {
                        HumanoidDominantHand::Left => Quat::from_rotation_y(180.0_f32.to_radians()),
                        HumanoidDominantHand::Right => Quat::default(),
                    },
                    ..Default::default()
                },
                ..Default::default()
            });
            parent.spawn((
                MaterialMeshBundle {
                    material: boombox_assets.matte.clone(),
                    mesh: boombox_assets.headphones.clone(),
                    ..Default::default()
                },
                SetOutlineDepth::Real,
            ));
        });

        commands
            .entity(humanoid.dominant_hand())
            .with_children(|parent| {
                parent.spawn(MaterialMeshBundle {
                    material: boombox_assets.matte.clone(),
                    mesh: boombox_assets.boombox.clone(),
                    transform: Transform {
                        translation: Vec3::new(0.0, 0.25, 0.0),
                        rotation: match dominant {
                            HumanoidDominantHand::Left => {
                                Quat::from_rotation_y(180.0_f32.to_radians())
                            }
                            HumanoidDominantHand::Right => Quat::default(),
                        },
                        ..Default::default()
                    },
                    ..Default::default()
                });
            });

        for e_child in children_query.iter_descendants(e_humanoid) {
            let Ok(mut animator) = animator_query.get_mut(e_child) else {
                continue;
            };

            animator
                .play(match dominant {
                    HumanoidDominantHand::Left => boombox_assets.idle_lt.clone(),
                    HumanoidDominantHand::Right => boombox_assets.idle_rt.clone(),
                })
                .repeat();

            break;
        }
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
