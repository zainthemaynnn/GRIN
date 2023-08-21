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
        Damage, DamageBuffer, DamageVariant, Dead, Health, HealthBundle,
    },
    humanoid::{
        Humanoid, HumanoidAssets, HumanoidBundle, HumanoidDominantHand, HumanoidPartType,
        HUMANOID_RADIUS,
    },
    item::Target,
    map::{MapLoadState, NavMesh},
    physics::{CollisionGroupExt, CollisionGroupsExt, ForceTimer},
    render::sketched::SketchMaterial,
    time::Rewind,
    util::{
        distr,
        vectors::{self, Vec3Ext},
    },
};

use super::{
    dummy::DummySet,
    movement::{follow_velocity, move_to_target, CircularVelocity, MovementBundle, PathBehavior},
    propagate_attack_target_to_weapon, set_closest_attack_target,
};

#[derive(SystemSet, Hash, Debug, Eq, PartialEq, Copy, Clone)]
pub enum BoomBoxSet {
    Spawn,
    Setup,
    Propagate,
    Act,
}

pub struct BoomBoxPlugin;

impl Plugin for BoomBoxPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<BoomBoxSpawnEvent>()
            .configure_sets(
                Update,
                (
                    BoomBoxSet::Setup.run_if(in_state(AssetLoadState::Success)),
                    BoomBoxSet::Propagate,
                    BoomBoxSet::Act,
                )
                    .chain(),
            )
            .add_collection_to_loading_state::<_, BoomBoxAssets>(AssetLoadState::Loading)
            .add_systems(
                Update,
                (
                    apply_deferred
                        .before(LandmassSystemSet::SyncExistence)
                        .after(DummySet::Spawn),
                    spawn
                        .in_set(BoomBoxSet::Spawn)
                        .run_if(in_state(MapLoadState::Success)),
                    init_humanoid.in_set(BoomBoxSet::Spawn),
                    set_closest_attack_target::<BoomBox, PlayerCharacter>.in_set(BoomBoxSet::Setup),
                    propagate_attack_target_to_weapon::<BoomBox>.in_set(BoomBoxSet::Propagate),
                    move_to_target::<BoomBox>.in_set(BoomBoxSet::Act),
                    fire.in_set(BoomBoxSet::Act),
                    fire.in_set(BoomBoxSet::Act),
                    follow_velocity::<BoomBox>
                        .in_set(BoomBoxSet::Act)
                        .after(LandmassSystemSet::Output),
                )
                    .run_if(in_state(AssetLoadState::Success)),
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

#[derive(Event, Default)]
pub struct BoomBoxSpawnEvent {
    pub transform: Transform,
}

impl BoomBox {
    // how do I return impl IntoSystem??? too confusing???
    pub fn spawn_at(transform: Transform) -> impl Fn(EventWriter<BoomBoxSpawnEvent>) {
        move |mut events: EventWriter<BoomBoxSpawnEvent>| {
            events.send(BoomBoxSpawnEvent { transform });
        }
    }
}

pub fn spawn(
    mut commands: Commands,
    hum_assets: Res<HumanoidAssets>,
    nav_mesh: Res<NavMesh>,
    mut events: EventReader<BoomBoxSpawnEvent>,
) {
    for BoomBoxSpawnEvent { transform } in events.iter() {
        commands.spawn((
            BoomBoxUninit,
            Target::default(),
            ShotCooldown::default(),
            HealthBundle {
                health: Health(1.0),
                ..Default::default()
            },
            MovementBundle {
                path_behavior: PathBehavior::Beeline { velocity: 2.0 },
            },
            CollisionGroups::from_group_default(Group::ENEMY),
            HumanoidBundle {
                skeleton_gltf: hum_assets.skeleton.clone(),
                spatial: SpatialBundle::from_transform(transform.clone()),
                ..Default::default()
            },
            AgentBundle {
                archipelago_ref: ArchipelagoRef(nav_mesh.archipelago),
                agent: Agent {
                    radius: HUMANOID_RADIUS,
                    max_velocity: 2.0,
                },
                velocity: AgentVelocity::default(),
                desired_velocity: AgentDesiredVelocity::default(),
                target: AgentTarget::None,
            },
            RigidBody::KinematicPositionBased,
        ));
    }
}

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct BoomBoxUninit;

pub fn init_humanoid(
    mut commands: Commands,
    humanoid_query: Query<(Entity, &Humanoid, &HumanoidDominantHand), With<BoomBoxUninit>>,
    mut animator_query: Query<&mut AnimationPlayer>,
    children_query: Query<&Children>,
    boombox_assets: Res<BoomBoxAssets>,
) {
    for (e_humanoid, humanoid, dominant) in humanoid_query.iter() {
        commands
            .entity(e_humanoid)
            .remove::<BoomBoxUninit>()
            .insert(BoomBox::default());

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

pub fn create_bullet(source: Entity, base_transform: Transform, dir: Vec3) -> impl Bundle {
    let transform = base_transform.looking_to(dir, dir.any_orthogonal_vector());
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
            .map(move |dir| create_bullet(entity, transform, dir)),
        );
    }
}
