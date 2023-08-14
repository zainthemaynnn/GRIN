use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

pub struct CollisionsPlugin;

impl Plugin for CollisionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                assign_collider_ref_transforms,
                assign_collider_ref_collision_groups,
            ),
        );
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

        crate::generic_collider!($meshes, $mesh_handle, &ComputedColliderShape::TriMesh)
    }};
}

/// Generates a convex collider from a mesh handle and mesh collection using VHACD parameters.
#[macro_export]
macro_rules! convex_collider {
    ( $meshes:expr, $mesh_handle:expr, $vhacd:expr ) => {{
        use bevy_rapier3d::prelude::ComputedColliderShape;

        crate::generic_collider!(
            $meshes,
            $mesh_handle,
            &ComputedColliderShape::ConvexDecomposition($vhacd)
        )
    }};
}

#[cfg(test)]
/// Creates an app with a one second timestep and the plugins needed for rapier physics.
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
