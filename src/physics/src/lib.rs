use std::time::Duration;

use bevy::{ecs::system::SystemParam, prelude::*, time::TimeSystem};
use bevy_rapier3d::prelude::*;
use grin_time::scaling::TimeScale;

#[derive(Default)]
pub struct GrinPhysicsPlugin {
    /// Is the debug-rendering enabled?
    pub debug_enabled: bool,
    /// Control some aspects of the render coloring.
    pub debug_style: DebugRenderStyle,
    /// Flags to select what part of physics scene is rendered (by default
    /// everything is rendered).
    pub debug_mode: DebugRenderMode,
}

impl Plugin for GrinPhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PhysicsTime>()
            .add_plugins((
                RapierPhysicsPlugin::<GrinPhysicsHooks>::default(),
                RapierDebugRenderPlugin {
                    enabled: self.debug_enabled,
                    style: self.debug_style,
                    mode: self.debug_mode,
                },
            ))
            .init_resource::<GrinPhysicsHooks>()
            .insert_resource(RapierConfiguration {
                // gravity is scaled by human height / humanoid height.
                // it's a magic number. I don't want to import `grin_character`.
                // deal with it.
                gravity: Vec3::NEG_Y * 9.81 * (2.625 / 1.8),
                ..Default::default()
            })
            .add_systems(
                Update,
                (
                    assign_collider_ref_transforms,
                    assign_collider_ref_collision_groups,
                ),
            )
            .add_systems(First, write_physics_time.after(TimeSystem))
            .add_systems(Last, (update_force_timers, kill_timed_forces).chain());
    }
}

/// Custom physics hooks.
///
/// This is important for computing contact points for melee weapons, without the constraints
/// solver interfering.
#[derive(Resource, SystemParam, Default)]
pub struct GrinPhysicsHooks;

impl BevyPhysicsHooks for GrinPhysicsHooks {
    fn filter_contact_pair(&self, _context: PairFilterContextView) -> Option<SolverFlags> {
        Some(SolverFlags::empty())
    }
}

pub trait CollisionGroupExt {
    const PLAYER: Group;
    const PLAYER_PROJECTILE: Group;
    const ENEMY: Group;
    const ENEMY_PROJECTILE: Group;
    const DEBRIS: Group;
    const MAP: Group;
    const PROJECTILE: Group;
}

impl CollisionGroupExt for Group {
    const PLAYER: Group = Self::GROUP_1;
    const PLAYER_PROJECTILE: Group = Self::GROUP_2;
    const ENEMY: Group = Self::GROUP_3;
    const ENEMY_PROJECTILE: Group = Self::GROUP_4;
    const DEBRIS: Group = Self::GROUP_5;
    const MAP: Group = Self::GROUP_32;
    // well this is weird... thanks rust
    // https://github.com/bitflags/bitflags/issues/180
    const PROJECTILE: Group = Self::PLAYER_PROJECTILE.union(Self::ENEMY_PROJECTILE);
}

pub trait CollisionGroupsExt {
    fn from_group_default(group: Group) -> Self;
}

impl CollisionGroupsExt for CollisionGroups {
    /// Creates `CollisionGroups` with "default" collision group filters from a `Group`.
    fn from_group_default(group: Group) -> Self {
        match group {
            Group::NONE => CollisionGroups::new(Group::NONE, Group::NONE),
            Group::PLAYER => CollisionGroups::new(Group::PLAYER, Group::all() - Group::PLAYER),
            Group::ENEMY => CollisionGroups::new(Group::ENEMY, Group::all() - Group::ENEMY),
            Group::PLAYER_PROJECTILE => CollisionGroups::new(
                Group::PLAYER_PROJECTILE,
                Group::all() - Group::PLAYER - Group::PROJECTILE,
            ),
            Group::ENEMY_PROJECTILE => CollisionGroups::new(
                Group::ENEMY_PROJECTILE,
                Group::all() - Group::ENEMY - Group::PROJECTILE,
            ),
            Group::DEBRIS => CollisionGroups::new(Group::DEBRIS, Group::MAP),
            Group::MAP => CollisionGroups::new(Group::MAP, Group::all()),
            _ => CollisionGroups::default(),
        }
    }
}

/// Generates a collider from a mesh handle and mesh collection with its `ComputedColliderShape`.
///
/// Created because `Collider::from_bevy_mesh` requires `Mesh` but `MaterialMeshBundle` just requires `Handle<Mesh>`,
/// and because getting a `T` from `Assets<T>` is already a mouthful.
#[macro_export]
macro_rules! generic_collider {
    ( $meshes:expr, $mesh_handle:expr, $shape:expr ) => {{
        use bevy_rapier3d::prelude::Collider;

        Collider::from_bevy_mesh(
            $meshes
                .get($mesh_handle)
                .expect("Source mesh not found when generating mesh collider."),
            $shape,
        )
        .expect("Failed to generate mesh collider.")
    }};
}

/// Generates a trimesh collider from a mesh handle and mesh collection.
#[macro_export]
macro_rules! collider {
    ( $meshes:expr, $mesh_handle:expr ) => {{
        use bevy_rapier3d::prelude::ComputedColliderShape;

        grin_physics::generic_collider!($meshes, $mesh_handle, &ComputedColliderShape::TriMesh)
    }};
}

/// Generates a convex collider from a mesh handle and mesh collection using VHACD parameters.
#[macro_export]
macro_rules! convex_collider {
    ( $meshes:expr, $mesh_handle:expr, $vhacd:expr ) => {{
        use grin_physics::ComputedColliderShape;

        grin_physics::generic_collider!(
            $meshes,
            $mesh_handle,
            &ComputedColliderShape::ConvexDecomposition($vhacd)
        )
    }};
}

/// Creates an app with a one second timestep and the plugins needed for rapier physics.
#[cfg(test)]
pub fn new_physics_app() -> App {
    use bevy::{render::mesh::MeshPlugin, scene::ScenePlugin, time::TimePlugin};

    let mut app = App::new();
    app.insert_resource(RapierConfiguration {
        timestep_mode: TimestepMode::Fixed {
            dt: 1.0,
            substeps: 1,
        },
        ..Default::default()
    })
    .add_plugins((TimePlugin, AssetPlugin::default(), MeshPlugin, ScenePlugin))
    .add_plugins(RapierPhysicsPlugin::<NoUserData>::default());
    app
}

/// While `Time` represents actual real world time, `PhysicsTime` records
/// physics simulation time, which might change during frame lag.
#[derive(Resource, Default)]
pub struct PhysicsTime(pub Time);

/// Updates `PhysicsTime`.
pub fn write_physics_time(
    time: Res<Time>,
    mut physics_time: ResMut<PhysicsTime>,
    rapier_config: Res<RapierConfiguration>,
) {
    // just found out about `SimulationToRenderTime`. hm. whoops.
    let dt = match rapier_config.timestep_mode {
        TimestepMode::Fixed { dt, .. } => dt,
        // as far as I'm concerned here, they're the same.
        TimestepMode::Variable {
            max_dt, time_scale, ..
        }
        | TimestepMode::Interpolated {
            dt: max_dt,
            time_scale,
            ..
        } => f32::min(max_dt, time.delta_seconds() * time_scale),
    };
    let last_update = physics_time
        .0
        .last_update()
        .unwrap_or_else(|| time.last_update().unwrap());
    physics_time
        .0
        .update_with_instant(last_update + Duration::from_secs_f32(dt));
}

/// Add to an `ExternalForce`. At the end of this component's duration,
/// the `ExternalForce` is stopped and this component is removed.
#[derive(Component)]
pub struct ForceTimer {
    pub timer: Timer,
}

impl ForceTimer {
    pub fn new(duration: Duration) -> Self {
        Self {
            timer: Timer::new(duration, TimerMode::Once),
        }
    }

    pub fn from_seconds(duration: f32) -> Self {
        Self {
            timer: Timer::from_seconds(duration, TimerMode::Once),
        }
    }
}

/// Updates `ForceTimer` durations.
pub fn update_force_timers(
    time: Res<PhysicsTime>,
    mut timer_query: Query<(&mut ForceTimer, &TimeScale)>,
) {
    for (mut timer, time_scale) in timer_query.iter_mut() {
        timer
            .timer
            .tick(time.0.delta().mul_f32(f32::from(time_scale)));
    }
}

/// Removes `ForceTimer`s and stops `ExternalForce`s.
pub fn kill_timed_forces(
    mut commands: Commands,
    mut timer_query: Query<(Entity, &ForceTimer, &mut ExternalForce)>,
) {
    for (e_timer, timer, mut force) in timer_query.iter_mut() {
        if timer.timer.just_finished() {
            force.force = Vec3::ZERO;
            force.torque = Vec3::ZERO;
            commands.entity(e_timer).remove::<ForceTimer>();
        }
    }
}

// NOTE: it turns out that this was unnecessary
// but hell, I wrote it, and might need it in my belt later, so it'll sit for now

/// Matches the transform of an adjacent `Collider` to this entity.
///
/// A jank solution to combined colliders of a rigid-body
/// needing to have direct children as colliders.
#[derive(Component)]
pub struct ColliderRef(pub Entity);

#[derive(Bundle)]
pub struct ColliderRefBundle {
    pub collider: Collider,
    pub collider_ref: ColliderRef,
    pub collision_groups: CollisionGroups,
    pub transform: Transform,
    pub global_transform: Transform,
}

/// Updates the `Transform`s of `ColliderRef`s to match the source entity.
pub fn assign_collider_ref_transforms(
    mut collider_ref_query: Query<(&ColliderRef, Option<&Parent>, &mut Transform)>,
    g_transform_query: Query<&GlobalTransform>,
) {
    for (ColliderRef(e_collider), parent, mut transform) in collider_ref_query.iter_mut() {
        let g_parent_transform = match parent {
            Some(parent) => *g_transform_query.get(parent.get()).unwrap(),
            None => GlobalTransform::default(),
        };
        let g_collider_transform = g_transform_query.get(*e_collider).unwrap();
        *transform = g_collider_transform.reparented_to(&g_parent_transform);
    }
}

/// Updates the `CollisionGroups` of `ColliderRef`s to match the source entity.
pub fn assign_collider_ref_collision_groups(
    mut collider_ref_query: Query<(&ColliderRef, &mut CollisionGroups)>,
    collision_groups_query: Query<&CollisionGroups, Without<ColliderRef>>,
) {
    for (ColliderRef(e_collider), mut collision_groups) in collider_ref_query.iter_mut() {
        if let Ok(new_collision_groups) = collision_groups_query.get(*e_collider) {
            *collision_groups = *new_collision_groups;
        };
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::TAU;

    use super::*;

    #[test]
    fn physics_time() {
        let mut app = new_physics_app();
        app.add_plugins(GrinPhysicsPlugin)
            .insert_resource(RapierConfiguration {
                timestep_mode: TimestepMode::Variable {
                    max_dt: 1.0 / 60.0,
                    time_scale: 1.0,
                    substeps: 1,
                },
                ..Default::default()
            })
            .add_systems(
                First,
                (|mut time: ResMut<Time>| {
                    let latest = time.last_update().unwrap();
                    time.update_with_instant(latest + Duration::from_secs(1));
                })
                .after(TimeSystem)
                .before(write_physics_time),
            );

        // first update just syncs the `last_update` so delta is zero
        // gotta do it twice
        app.update();
        app.update();

        let time = app.world.resource::<Time>();
        let physics_time = app.world.resource::<PhysicsTime>();
        assert_eq!(time.delta_seconds(), 1.0);
        assert_eq!(
            physics_time.0.delta_seconds(),
            1.0 / 60.0,
            "`PhysicsTime` wasn't capped.",
        );
    }

    #[test]
    fn force_timer() {
        let mut app = new_physics_app();
        app.add_plugins(GrinPhysicsPlugin);

        let e_force = app
            .world
            .spawn((
                TimeScale::default(),
                ExternalForce {
                    force: Vec3::new(1.0, 0.0, 0.0),
                    torque: Vec3::new(TAU, 0.0, 0.0),
                },
                ForceTimer::from_seconds(1.0),
            ))
            .id();

        app.update();

        let mut time = app.world.resource_mut::<Time>();
        let latest = time.last_update().unwrap();
        time.update_with_instant(latest + Duration::from_secs(2));

        app.update();

        let force_timer = app.world.entity(e_force).get::<ForceTimer>();
        let force = app.world.entity(e_force).get::<ExternalForce>().unwrap();

        assert!(force_timer.is_none(), "`ForceTimer` wasn't removed.");
        assert_eq!(
            force.force,
            Vec3::ZERO,
            "`ExternalForce.force` wasn't reset.",
        );
        assert_eq!(
            force.torque,
            Vec3::ZERO,
            "`ExternalForce.torque` wasn't reset.",
        );
    }
}
