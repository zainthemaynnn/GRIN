use bevy::{
    gltf::GltfExtras,
    prelude::*,
    scene::InstanceId,
    utils::{hashbrown::hash_map::OccupiedError, HashMap},
};
use bevy_rapier3d::prelude::*;
use grin_util::query::cloned_scene_initializer;
use serde::Deserialize;

use crate::{
    health::{DamageBuffer, Health},
    hit::{ContactDamage, DamageCollisionGroups, MacroCollisionFilter},
};

#[derive(Component)]
pub struct Hitbox {
    pub target: Entity,
}

#[derive(Debug)]
pub enum HitboxGenerationError {
    DupedNodeId { node_id: String },
}

/// Contains a list of hitbox colliders related to this object.
#[derive(Component, Clone, Debug, Default)]
pub struct HitboxManager {
    pub colliders: HashMap<Name, Entity>,
}

#[derive(Deserialize)]
#[serde(tag = "Collider", rename_all_fields = "snake_case")]
pub enum ColliderAttributes {
    /// Uses `Collider::ball`.
    Ball { radius: f32 },
    /// Uses `Collider::capsule_y`.
    CapsuleY { half_height: f32, radius: f32 },
}

impl From<ColliderAttributes> for Collider {
    fn from(value: ColliderAttributes) -> Self {
        match value {
            ColliderAttributes::Ball { radius } => Collider::ball(radius),
            ColliderAttributes::CapsuleY {
                half_height,
                radius,
            } => Collider::capsule_y(half_height, radius),
        }
    }
}

/// Constructs GLTF hitboxes and populates `HitboxManager`.
pub fn init_hitboxes(
    In(loaded_items): In<Vec<(Entity, InstanceId, GltfHitboxAutoGenTarget)>>,
    mut commands: Commands,
    scene_manager: Res<SceneSpawner>,
    name_query: Query<&Name>,
    extras_query: Query<&GltfExtras>,
) {
    for (e_scene, scene_instance, autogen) in loaded_items {
        let e_master = match autogen {
            GltfHitboxAutoGenTarget::Here => e_scene,
            GltfHitboxAutoGenTarget::Remote(target) => target,
        };

        let mut colliders = HashMap::new();

        if let Ok(name) = name_query.get(e_scene) {
            debug!(owner=true, node=?name);
        }

        for e in scene_manager.iter_instance_entities(scene_instance) {
            if let Ok(name) = name_query.get(e) {
                debug!(node=?name, has_extras=extras_query.get(e).is_ok());
            }
        }

        for (e_hitbox, extras) in scene_manager
            .iter_instance_entities(scene_instance)
            .filter_map(|e_node| extras_query.get(e_node).ok().map(|extras| (e_node, extras)))
        {
            let node_id = name_query.get(e_hitbox).unwrap().to_owned();
            // yeah, I'm deserializing. what you gonna do about it?
            // TODO?: for real though, I can probably change the asset loader to deserialize this
            // into the asset itself only once, instead of on every load.
            // not a big priority though; I'll see if this is actually a problem first.
            let generated_collider = match serde_json::from_str::<ColliderAttributes>(&extras.value)
            {
                Ok(collider_attrs) => Collider::from(collider_attrs),
                Err(e) => {
                    error!(
                        msg="Failed to generate collider params from extras.",
                        error=?e,
                        node=node_id.to_string(),
                        json=extras.value,
                    );
                    continue;
                }
            };

            commands.entity(e_hitbox).insert((
                Hitbox { target: e_master },
                // ME WHEN I REALIZE YOU CAN HAVE COLLIDERS WITHOUT RIGID BODIES 🤯
                generated_collider,
                Sensor,
                ActiveEvents::COLLISION_EVENTS,
                ActiveHooks::FILTER_CONTACT_PAIRS,
                ColliderDisabled,
                CollisionGroups::default(),
                MacroCollisionFilter::default(),
            ));

            if let Err(OccupiedError { entry, .. }) = colliders.try_insert(node_id, e_hitbox) {
                error!(
                    error = ?HitboxGenerationError::DupedNodeId {
                        node_id: entry.key().to_string(),
                    }
                );
                continue;
            }
        }

        let hitbox_manager = HitboxManager { colliders };
        debug!(
            msg="Generated hitbox manager.",
            scene=?scene_instance,
            item=?hitbox_manager,
        );
        commands.entity(e_master).insert(hitbox_manager);
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

#[derive(Component, Clone, Copy, Debug, Default)]
pub enum GltfHitboxAutoGenTarget {
    #[default]
    Here,
    Remote(Entity),
}

/// It's sometimes impractical or inefficient to use the item's actual mesh for collisions.
///
/// This plugin allows you to export simplified hitbox meshes to GLTF by adding an
/// associated prefix (`config.0`) to the name of the node. Generated colliders are added
/// to a `HitboxManager`, and created on scene load.
pub struct GltfHitboxGenerationPlugin;

impl Plugin for GltfHitboxGenerationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
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
