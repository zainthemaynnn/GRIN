use bevy::{prelude::*, scene::InstanceId};
use bevy_rapier3d::prelude::*;
use grin_damage::ContactDamage;
use grin_physics::{collider, CollisionGroupExt, CollisionGroupsExt};
use grin_render::sketched::NoOutline;
use grin_util::query::{cloned_scene_initializer, gltf_prefix_search};

use crate::equip::Models;

/// Collision groups to use when dealing damage.
#[derive(Component, Copy, Clone)]
pub struct DamageCollisionGroups(pub CollisionGroups);

impl Default for DamageCollisionGroups {
    fn default() -> Self {
        Self(CollisionGroups::from_group_default(
            Group::PLAYER_PROJECTILE,
        ))
    }
}

impl From<&DamageCollisionGroups> for CollisionGroups {
    fn from(value: &DamageCollisionGroups) -> Self {
        value.0
    }
}

#[derive(Component, Default)]
pub struct Hitbox;

pub enum HitboxGenerationError {
    NoMeshPrimitive { node_id: String },
    DupedNodeId { node_id: String },
}

impl std::fmt::Display for HitboxGenerationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HitboxGenerationError::NoMeshPrimitive { node_id } => {
                return f.write_fmt(format_args!(
                    "Missing mesh primitive on hitbox node {}.",
                    node_id
                ));
            }
            HitboxGenerationError::DupedNodeId { node_id } => {
                return f.write_fmt(format_args!("Duplicated hitbox node key: {}", node_id));
            }
        }
    }
}

/// Contains a list of hitbox colliders related to this object.
#[derive(Component, Clone, Debug, Default)]
pub struct HitboxManager {
    pub colliders: Vec<Entity>, // TODO?: better way to make this work with ECS?
}

/// Constructs GLTF hitboxes and populates `HitboxManager`.
pub fn init_hitboxes(
    In(loaded_items): In<Vec<(Entity, InstanceId, GltfHitboxAutoGenTarget)>>,
    mut commands: Commands,
    meshes: Res<Assets<Mesh>>,
    scene_manager: Res<SceneSpawner>,
    prefix: Res<GltfHitboxAutoGenConfig>,
    name_query: Query<&Name>,
    mesh_query: Query<&Handle<Mesh>>,
) {
    for (_, scene_instance, GltfHitboxAutoGenTarget { master: e_master }) in loaded_items {
        let mut colliders = Vec::new();
        let hitboxes = gltf_prefix_search(&prefix.0, &scene_instance, &scene_manager, &name_query);

        for e_hitbox in hitboxes {
            // `unwrap` is safe because it was verified in `gltf_prefix_search`
            let node_id = name_query.get(e_hitbox).unwrap().to_string();

            match mesh_query.get(e_hitbox) {
                Ok(hitbox_geo) => {
                    commands.entity(e_hitbox).insert((
                        Hitbox::default(),
                        collider!(meshes, hitbox_geo),
                        // I doubt the below properties need to be customized
                        // TODO: I don't know if fixed works; tests required
                        RigidBody::Fixed,
                        ActiveEvents::COLLISION_EVENTS,
                        ActiveHooks::FILTER_CONTACT_PAIRS,
                        ColliderMassProperties::default(),
                        CollisionGroups::from_group_default(Group::DEBRIS),
                        GravityScale(1.0),
                        Visibility::Hidden,
                        NoOutline,
                    ));
                }
                Err(..) => {
                    error!("{}", HitboxGenerationError::NoMeshPrimitive { node_id });
                    continue;
                }
            }

            colliders.push(e_hitbox);
        }

        commands
            .entity(e_master)
            .insert(HitboxManager { colliders });
    }
}

/// Propagates `HitboxManager` `CollisionGroups` to child hitboxes.
pub fn sync_hitbox_collision_groups(
    hitbox_query: Query<(&DamageCollisionGroups, &HitboxManager), Changed<DamageCollisionGroups>>,
    mut collision_groups_query: Query<&mut CollisionGroups>,
) {
    for (collision_groups, hitboxes) in hitbox_query.iter() {
        for &e_hitbox in &hitboxes.colliders {
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

/// Propagates `HitboxManager` `ContactDamage` to child hitboxes.
///
/// Note: If the `ContactDamage` is consumed (i.e. hit something), it will not be reapplied
/// until the component is added again.
// TODO?: for multiple hitboxes I think the contact event should be applied to all?
// would be tricky to do.
pub fn sync_hitbox_activation(
    mut commands: Commands,
    hitbox_query: Query<(&ContactDamage, &HitboxManager), Added<ContactDamage>>,
) {
    for (contact_damage, hitboxes) in hitbox_query.iter() {
        for &e_hitbox in &hitboxes.colliders {
            commands.entity(e_hitbox).insert(*contact_damage);
        }
    }
}

/// Inverse of `sync_hitbox_activation`.
pub fn sync_hitbox_deactivation(
    mut commands: Commands,
    hitbox_query: Query<&HitboxManager>,
    mut removed: RemovedComponents<ContactDamage>,
) {
    for e_hitboxman in removed.read() {
        let Ok(hitboxes) = hitbox_query.get(e_hitboxman) else {
            continue;
        };

        for &e_hitbox in &hitboxes.colliders {
            commands.entity(e_hitbox).remove::<ContactDamage>();
        }
    }
}

/// Enables GLTF hitbox auto-generation for the associated entity.
#[derive(Component, Clone, Debug, Default)]
pub enum GltfHitboxAutoGen {
    #[default]
    Enabled,
    Disabled,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct GltfHitboxAutoGenTarget {
    pub master: Entity,
}

pub fn insert_autogen_markers(
    mut commands: Commands,
    autogen_query: Query<(Entity, &Models, &GltfHitboxAutoGen)>,
) {
    for (e_master, Models { targets }, autogen) in autogen_query.iter() {
        if let GltfHitboxAutoGen::Enabled = autogen {
            for &e_target in targets.values() {
                commands
                    .entity(e_target)
                    .insert(GltfHitboxAutoGenTarget { master: e_master });
            }
        }
        commands.entity(e_master).remove::<GltfHitboxAutoGen>();
    }
}

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
                cloned_scene_initializer::<GltfHitboxAutoGenTarget>.pipe(init_hitboxes),
            )
            .add_systems(
                PostUpdate,
                (
                    sync_hitbox_collision_groups,
                    sync_hitbox_activation,
                    sync_hitbox_deactivation,
                    insert_autogen_markers,
                ),
            );
    }
}
