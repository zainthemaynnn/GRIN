use bevy_rapier3d::prelude::*;

pub trait CollisionGroupExt {
    const PLAYER: Group;
    const PLAYER_PROJECTILE: Group;
    const ENEMY: Group;
    const ENEMY_PROJECTILE: Group;
}

impl CollisionGroupExt for Group {
    const PLAYER: Group = Self::GROUP_1;
    const PLAYER_PROJECTILE: Group = Self::GROUP_2;
    const ENEMY: Group = Self::GROUP_3;
    const ENEMY_PROJECTILE: Group = Self::GROUP_4;
}

/// Generates a collider from a mesh handle and mesh collection with its `ComputedColliderShape`.
///
/// Created because `Collider::from_bevy_mesh` requires `Mesh` but `MaterialMeshBundle` just requires `Handle<Mesh>`,
/// and because getting a `T` from `Assets<T>` is already a mouthful.
#[macro_export]
macro_rules! generic_collider {
    ( $meshes:expr, $mesh_handle:expr, $shape:expr ) => {
        Collider::from_bevy_mesh(
            $meshes
                .get($mesh_handle)
                .expect("Source mesh not found when generating mesh collider."),
            $shape,
        )
        .expect("Failed to generate mesh collider.")
    };
}

/// Generates a trimesh collider from a mesh handle and mesh collection.
#[macro_export]
macro_rules! collider {
    ( $meshes:expr, $mesh_handle:expr ) => {
        crate::generic_collider!($meshes, $mesh_handle, &ComputedColliderShape::TriMesh)
    };
}

/// Generates a convex collider from a mesh handle and mesh collection using VHACD parameters.
#[macro_export]
macro_rules! convex_collider {
    ( $meshes:expr, $mesh_handle:expr, $vhacd:expr ) => {
        crate::generic_collider!(
            $meshes,
            $mesh_handle,
            &ComputedColliderShape::ConvexDecomposition($vhacd)
        )
    };
}
