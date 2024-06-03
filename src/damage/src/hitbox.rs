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
    health::DamageBuffer,
    hit::{ContactDamage, MacroCollisionFilter},
    plugin::HitboxSet,
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
    mut hitboxes_query: Query<(&mut HitboxManager, Option<&GltfHitboxAutoGenTemplate>)>,
) {
    for (e_scene, scene_instance, autogen) in loaded_items {
        let e_master = match autogen {
            GltfHitboxAutoGenTarget::Here => e_scene,
            GltfHitboxAutoGenTarget::Remote(target) => target,
        };

        let Ok((mut hitbox_manager, template)) = hitboxes_query.get_mut(e_master) else {
            error!(
                error="Missing `HitboxManager` during collider autogen.",
                entity=?e_master,
            );
            continue;
        };

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

            let mut e = commands.entity(e_hitbox);

            e.insert((Hitbox { target: e_master }, generated_collider));

            if let Some(&template) = template {
                match template {
                    GltfHitboxAutoGenTemplate::Hitbox => {
                        e.insert((
                            RigidBody::KinematicPositionBased,
                            ContactDamage::FollowThrough,
                            ColliderDisabled,
                            MacroCollisionFilter::default(),
                            CollisionGroups::default(),
                            ActiveEvents::COLLISION_EVENTS,
                            ActiveHooks::FILTER_CONTACT_PAIRS,
                            ActiveCollisionTypes::default()
                                | ActiveCollisionTypes::KINEMATIC_KINEMATIC,
                        ));
                    }
                    GltfHitboxAutoGenTemplate::Hurtbox => {
                        e.insert((
                            RigidBody::KinematicPositionBased,
                            DamageBuffer::default(),
                            CollisionGroups::default(),
                            ActiveEvents::COLLISION_EVENTS,
                            ActiveHooks::FILTER_CONTACT_PAIRS,
                        ));
                    }
                }
            }

            if let Err(OccupiedError { entry, .. }) =
                hitbox_manager.colliders.try_insert(node_id, e_hitbox)
            {
                error!(
                    error = ?HitboxGenerationError::DupedNodeId {
                        node_id: entry.key().to_string(),
                    }
                );
                continue;
            }
        }

        debug!(
            msg="Generated hitbox manager.",
            source=?scene_instance,
            entity=?e_master,
            template=?template,
            item=?*hitbox_manager,
        );
    }
}

/// Propagates `HitboxManager` `CollisionGroups` to child hitboxes.
pub fn sync_hitbox_collision_groups(
    hitbox_query: Query<(Entity, &CollisionGroups, &HitboxManager), Changed<CollisionGroups>>,
    mut collision_groups_query: Query<&mut CollisionGroups, Without<HitboxManager>>,
) {
    for (e_hitboxes, collision_groups, hitboxes) in hitbox_query.iter() {
        trace!(
            msg="Syncing hitbox collision groups.",
            entity=?e_hitboxes,
            collision_groups=?collision_groups,
        );
        for &e_hitbox in hitboxes.colliders.values() {
            match collision_groups_query.get_mut(e_hitbox) {
                Ok(mut collision_groups_ref) => {
                    *collision_groups_ref = *collision_groups;
                }
                Err(..) => {
                    warn!(
                        msg="Missing `CollisionGroups` for `Hitbox`.",
                        entity=?e_hitbox,
                    );
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

/// TODO: I don't know why the colliders veer off in a random direction under animations.
/// my guess is it has something to do with system ordering between bevy animation stuff
/// and rapier collider stuff. this is possible to fix through app configuration. however,
/// I'm too lazy to resolve this right now and I think I'll do this 30-second fix instead.
pub fn rezero_hitbox_positions(mut hitbox_query: Query<&mut Transform, With<Hitbox>>) {
    hitbox_query.iter_mut().for_each(|mut t| {
        *t = Transform::IDENTITY;
    });
}

// TODO?: deactivating for now, due to the addition of AutoGenTemplate.
// admittedly, the current solution is pretty spaghetti, but not used in many places.
// I'm not sure if I like the idea of a template very much. I may switch back
// to using this with certain modifications in the future, if it becomes an issue
// (i.e. I design an enemy that has some weird interaction with hitboxes).
/*pub fn convert_to_hurtboxes(
    mut commands: Commands,
    hurtbox_query: Query<(Entity, &HitboxManager), Added<Health>>,
) {
    for (e_hitboxes, hitboxes) in hurtbox_query.iter() {
        debug!(
            msg="Adding damage buffers.",
            entity=?e_hitboxes,
        );
        for &e_hitbox in hitboxes.colliders.values() {
            commands.entity(e_hitbox).insert(DamageBuffer::default());
        }
    }
}*/

#[derive(Component, Clone, Copy, Debug, Default)]
pub enum GltfHitboxAutoGenTemplate {
    #[default]
    Hitbox,
    Hurtbox,
}

/// Note: Generated hitboxes use `RigidBody::KinematicPositionBased`.
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
        app.configure_sets(PreUpdate, (HitboxSet::Generate, HitboxSet::Sync).chain())
            .add_systems(First, rezero_hitbox_positions)
            .add_systems(
                PreUpdate,
                (
                    cloned_scene_initializer::<GltfHitboxAutoGenTarget>
                        .pipe(init_hitboxes)
                        .in_set(HitboxSet::Generate),
                    (
                        sync_hitbox_collision_groups,
                        sync_hitbox_activation,
                        sync_hitbox_deactivation,
                    )
                        .in_set(HitboxSet::Sync),
                ),
            );
    }
}
