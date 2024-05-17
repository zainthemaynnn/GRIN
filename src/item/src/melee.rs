use std::marker::PhantomData;

use bevy::{
    prelude::*,
    scene::{InstanceId, SceneInstance},
};
use bevy_rapier3d::prelude::*;
use grin_physics::{collider, CollisionGroupExt, CollisionGroupsExt};
use grin_util::query::{gltf_prefix_search, labelled_scene_initializer};

use crate::DamageCollisionGroups;

#[derive(Component)]
pub struct SingleGrip {
    pub grip: Vec3,
}

#[derive(Component)]
pub struct DoubleGrip {
    pub single: Vec3,
    pub double: Vec3,
}

impl DoubleGrip {
    pub fn hammer() -> Self {
        Self {
            single: Vec3::ZERO,
            double: Vec3::new(0.0, -0.5, 0.0),
        }
    }
}

/// Wind-up progress for a melee weapon.
#[derive(Component)]
pub struct Wind {
    /// Current charge amount in seconds.
    pub charge: f32,
    /// Time to reach max charge in seconds.
    pub max: f32,
}

impl Wind {
    pub fn new(max: f32) -> Self {
        Self { charge: 0.0, max }
    }

    pub fn progress(&self) -> f32 {
        self.charge / self.max
    }
}

impl Default for Wind {
    fn default() -> Self {
        Self::new(1.0)
    }
}

#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct Winding {
    pub duration: f32,
}

#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct Charging;

#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct Swinging {
    pub duration: f32,
}

pub fn update_hammer_winds(time: Res<Time>, mut wind_query: Query<(&mut Wind, Option<&Winding>)>) {
    for (mut wind, active) in wind_query.iter_mut() {
        if active.is_some() {
            wind.charge += time.delta_seconds();
        } else {
            wind.charge = 0.0;
        }
    }
}

#[derive(Component, Default)]
pub struct Hitbox;

pub enum HitboxGenerationError {
    NoMeshPrimitive,
}

impl std::fmt::Display for HitboxGenerationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HitboxGenerationError::NoMeshPrimitive => {
                return f.write_str("Missing mesh primitive on hitbox node.");
            }
        }
    }
}

/// Contains a list of hitbox colliders related to this object.
#[derive(Component)]
pub struct HitboxManager {
    pub colliders: Vec<Entity>, // TODO?: better way to make this work with ECS?
}

/// Constructs GLTF hitboxes and populates `HitboxManager`.
pub fn init_hitboxes(
    In(loaded_items): In<Vec<(Entity, InstanceId)>>,
    mut commands: Commands,
    meshes: Res<Assets<Mesh>>,
    scene_manager: Res<SceneSpawner>,
    prefix: Res<GltfHitboxAutoGenConfig>,
    name_query: Query<&Name>,
    mesh_query: Query<&Handle<Mesh>>,
) {
    for (e_item, scene_instance) in loaded_items {
        let mut colliders = Vec::new();
        let hitboxes = gltf_prefix_search(&prefix.0, &scene_instance, &scene_manager, &name_query);

        for e_hitbox in hitboxes {
            match mesh_query.get(e_hitbox) {
                Ok(hitbox_geo) => {
                    commands.entity(e_hitbox).insert((
                        Hitbox::default(),
                        collider!(meshes, hitbox_geo),
                        // I doubt the below properties need to be customized
                        RigidBody::Dynamic,
                        ActiveEvents::COLLISION_EVENTS,
                        ActiveHooks::FILTER_CONTACT_PAIRS,
                        ColliderMassProperties::default(),
                        CollisionGroups::from_group_default(Group::DEBRIS),
                        GravityScale(1.0),
                    ));
                }
                Err(..) => {
                    warn!("{}", HitboxGenerationError::NoMeshPrimitive);
                }
            }
        }

        commands.entity(e_item).insert(HitboxManager { colliders });
    }
}

pub fn update_hitbox_collision_groups(
    hitbox_query: Query<(&DamageCollisionGroups, &HitboxManager), Changed<DamageCollisionGroups>>,
    mut collision_groups_query: Query<&mut CollisionGroups>,
) {
    for (collision_groups, hitboxes) in hitbox_query.iter() {
        for e_hitbox in hitboxes.colliders {
            match collision_groups_query.get_mut(e_hitbox) {
                Ok(mut collision_groups_ref) => {
                    *collision_groups_ref = collision_groups.0;
                }
                Err(..) => {
                    warn!("Missing `CollisionGroups` for `Hitbox`.");
                }
            }
        }
    }
}

/// Enables GLTF hitbox auto-generation for the associated entity.
#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct GltfHitboxAutoGen;

/// It's sometimes impractical or inefficient to use the item's actual mesh for collisions.
///
/// This plugin allows you to export simplified hitbox meshes to GLTF by adding an
/// associated prefix (`config.0`) to the name of the node. Generated colliders are added
/// to a `HitboxManager`, and created on scene load.
pub struct GltfHitboxGenerationPlugin {
    pub config: GltfHitboxAutoGenConfig,
}

#[derive(Resource, Clone)]
pub struct GltfHitboxAutoGenConfig(pub String);

impl Plugin for GltfHitboxGenerationPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.config.clone())
            .add_systems(
                PreUpdate,
                labelled_scene_initializer::<GltfHitboxAutoGen>.pipe(init_hitboxes),
            )
            .add_systems(PostUpdate, update_hitbox_collision_groups);
    }
}
