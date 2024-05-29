// TODO: this should be reworked to use builtin collider shapes instead of mesh-produced ones
// this should be done using GLTF custom attributes
use bevy::{
    prelude::*,
    scene::InstanceId,
    utils::{hashbrown::hash_map::OccupiedError, HashMap},
};
use bevy_rapier3d::prelude::*;
use grin_physics::{collider, CollisionGroupExt, CollisionGroupsExt};
use grin_render::sketched::NoOutline;
use grin_util::query::{cloned_scene_initializer, gltf_prefix_search};

use crate::{
    health::{DamageBuffer, Health},
    hit::{ContactDamage, DamageCollisionGroups},
};

#[derive(Component)]
pub struct Hitbox {
    pub target: Entity,
}

#[derive(Debug)]
pub enum HitboxGenerationError {
    NoMeshPrimitive { node_id: Name },
    DupedNodeId { node_id: Name },
}

/// Contains a list of hitbox colliders related to this object.
#[derive(Component, Clone, Debug, Default)]
pub struct HitboxManager {
    pub colliders: HashMap<Name, Entity>,
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
    for (e_item, scene_instance, autogen) in loaded_items {
        let e_master = match autogen {
            GltfHitboxAutoGenTarget::Here => e_item,
            GltfHitboxAutoGenTarget::Remote(target) => target,
        };

        let mut colliders = HashMap::new();
        let hitboxes = gltf_prefix_search(&prefix.0, &scene_instance, &scene_manager, &name_query);

        for e_hitbox in hitboxes {
            // `unwrap` is safe because it was verified in `gltf_prefix_search`
            let node_id = name_query.get(e_hitbox).unwrap().to_owned();

            match mesh_query.get(e_hitbox) {
                Ok(hitbox_geo) => {
                    commands.entity(e_hitbox).insert((
                        Hitbox { target: e_master },
                        // ME WHEN I REALIZE YOU CAN HAVE COLLIDERS WITHOUT RIGID BODIES 🤯
                        collider!(meshes, hitbox_geo),
                        Sensor,
                        ActiveEvents::COLLISION_EVENTS,
                        ActiveHooks::FILTER_CONTACT_PAIRS,
                        CollisionGroups::from_group_default(Group::DEBRIS),
                    ))
                    .remove::<Handle<Mesh>>();
                }
                Err(..) => {
                    error!(error = ?HitboxGenerationError::NoMeshPrimitive { node_id });
                    continue;
                }
            }

            if let Err(OccupiedError { entry, .. }) = colliders.try_insert(node_id, e_hitbox) {
                error!(
                    error = ?HitboxGenerationError::DupedNodeId {
                        node_id: entry.key().clone()
                    }
                );
                continue;
            }
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
        for &e_hitbox in hitboxes.colliders.values() {
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
        for &e_hitbox in hitboxes.colliders.values() {
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

        for &e_hitbox in hitboxes.colliders.values() {
            commands.entity(e_hitbox).remove::<ContactDamage>();
        }
    }
}

pub fn convert_to_hurtboxes(
    mut commands: Commands,
    hurtbox_query: Query<&HitboxManager, Or<(Added<HitboxManager>, Added<Health>)>>,
) {
    for hitboxes in hurtbox_query.iter() {
        for &e_hitbox in hitboxes.colliders.values() {
            commands.entity(e_hitbox).insert(DamageBuffer::default());
        }
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub enum GltfHitboxAutoGenTarget {
    Here,
    Remote(Entity),
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
                    convert_to_hurtboxes,
                ),
            );
    }
}
