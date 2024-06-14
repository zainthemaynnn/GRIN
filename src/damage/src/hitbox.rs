use std::{hash::BuildHasherDefault, marker::PhantomData};

use bevy::{
    gltf::GltfExtras,
    prelude::*,
    scene::InstanceId,
    utils::{hashbrown::hash_map::OccupiedError, AHasher, HashMap},
};
use bevy_rapier3d::prelude::*;
use grin_util::query::cloned_scene_initializer;
use serde::Deserialize;

use crate::{
    health::DamageBuffer,
    hit::{ContactDamage, MacroCollisionFilter},
    plugin::HitboxSet,
};

/// It's sometimes impractical or inefficient to use the item's actual mesh for collisions.
///
/// This plugin allows you to export simplified hitbox meshes to GLTF by adding an
/// associated prefix (`config.0`) to the name of the node. Generated colliders are added
/// to a `HitboxManager`, and created on scene load.
pub struct GltfHitboxGenerationPlugin;

impl Plugin for GltfHitboxGenerationPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(PreUpdate, (HitboxSet::Generate, HitboxSet::Sync).chain())
            .add_systems(
                PreUpdate,
                (
                    cloned_scene_initializer::<GltfHitboxAutoGenTarget>
                        .pipe(init_hitboxes)
                        .in_set(HitboxSet::Generate),
                    (
                        sync_hitbox_collision_groups::<Hitboxes>,
                        sync_hitbox_collision_groups::<Hurtboxes>,
                        sync_hitbox_activation::<Hitboxes>,
                        sync_hitbox_deactivation::<Hitboxes>,
                    )
                        .in_set(HitboxSet::Sync),
                ),
            );
    }
}

// yeah, I'm adding debug, screw you
pub trait HitboxCategory: Send + Sync + 'static + std::fmt::Debug + Sized {
    fn template() -> impl Bundle;
}

#[derive(Copy, Clone, Debug, Default)]
pub struct Hitboxes;

impl HitboxCategory for Hitboxes {
    fn template() -> impl Bundle {
        (
            RigidBody::KinematicPositionBased,
            ContactDamage::FollowThrough,
            ColliderDisabled,
            MacroCollisionFilter::default(),
            CollisionGroups::default(),
            ActiveEvents::COLLISION_EVENTS,
            ActiveHooks::FILTER_CONTACT_PAIRS,
            ActiveCollisionTypes::default() | ActiveCollisionTypes::KINEMATIC_KINEMATIC,
        )
    }
}

#[derive(Copy, Clone, Debug, Default)]
pub struct Hurtboxes;

impl HitboxCategory for Hurtboxes {
    fn template() -> impl Bundle {
        (
            RigidBody::KinematicPositionBased,
            DamageBuffer::default(),
            CollisionGroups::default(),
            ActiveEvents::COLLISION_EVENTS,
            ActiveHooks::FILTER_CONTACT_PAIRS,
        )
    }
}

#[derive(Component)]
pub struct Hitbox {
    pub target: Entity,
}

#[derive(Debug)]
pub enum HitboxGenerationError<'a> {
    DupedNodeId {
        e_master: Entity,
        err: OccupiedError<'a, Name, Entity, BuildHasherDefault<AHasher>>,
    },
    ColliderParseError {
        e_master: Entity,
        node_id: String,
        json: String,
        err: serde_json::Error,
    },
    HitboxManagerNotFound {
        e_master: Entity,
        node_id: String,
        ty: HitboxCategoryIdentifier,
    },
}

/// Contains a list of hitbox colliders related to this object.
#[derive(Component, Clone, Debug, Default)]
pub struct HitboxManager<T: HitboxCategory> {
    pub colliders: HashMap<Name, Entity>,
    pub phantom_data: PhantomData<T>,
}

#[derive(Bundle, Default)]
pub struct GltfHitboxBundle<T: HitboxCategory> {
    pub hitbox_manager: HitboxManager<T>,
    pub autogen_target: GltfHitboxAutoGenTarget,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "Collider", rename_all_fields = "snake_case")]
pub enum ColliderAttributes {
    /// Uses `Collider::ball`.
    Ball {
        radius: f32,
        ty: HitboxCategoryIdentifier,
    },
    /// Uses `Collider::capsule_y`.
    CapsuleY {
        half_height: f32,
        radius: f32,
        ty: HitboxCategoryIdentifier,
    },
}

impl ColliderAttributes {
    pub fn hitbox_category(&self) -> &HitboxCategoryIdentifier {
        match self {
            ColliderAttributes::Ball { ty, .. } | ColliderAttributes::CapsuleY { ty, .. } => ty,
        }
    }
}

impl From<ColliderAttributes> for Collider {
    fn from(value: ColliderAttributes) -> Self {
        match value {
            ColliderAttributes::Ball { radius, .. } => Collider::ball(radius),
            ColliderAttributes::CapsuleY {
                half_height,
                radius,
                ..
            } => Collider::capsule_y(half_height, radius),
        }
    }
}

#[derive(Deserialize, Copy, Clone, Debug, Eq, PartialEq)]
pub enum HitboxCategoryIdentifier {
    Hitbox,
    Hurtbox,
}

/// Constructs GLTF hitboxes and populates `HitboxManager`.
pub fn init_hitboxes(
    In(loaded_items): In<Vec<(Entity, InstanceId, GltfHitboxAutoGenTarget)>>,
    mut commands: Commands,
    scene_manager: Res<SceneSpawner>,
    name_query: Query<&Name>,
    extras_query: Query<&GltfExtras>,
    mut hitboxes_query: Query<&mut HitboxManager<Hitboxes>>,
    mut hurtboxes_query: Query<&mut HitboxManager<Hurtboxes>>,
) {
    for (e_scene, scene_instance, autogen) in loaded_items {
        let e_master = match autogen {
            GltfHitboxAutoGenTarget::Here => e_scene,
            GltfHitboxAutoGenTarget::Remote(target) => target,
        };

        let mut hitbox_manager = hitboxes_query.get_mut(e_master);
        let mut hurtbox_manager = hurtboxes_query.get_mut(e_master);

        for (e_hitbox, extras) in scene_manager
            .iter_instance_entities(scene_instance)
            .filter_map(|e_node| extras_query.get(e_node).ok().map(|extras| (e_node, extras)))
        {
            let node_id = name_query.get(e_hitbox).unwrap().to_owned();
            // yeah, I'm deserializing. what you gonna do about it?
            // TODO?: for real though, I can probably change the asset loader to deserialize this
            // into the asset itself only once, instead of on every load.
            // not a big priority though; I'll see if this is actually a problem first.
            let collider_attrs = match serde_json::from_str::<ColliderAttributes>(&extras.value) {
                Ok(collider_attrs) => collider_attrs,
                Err(err) => {
                    error!(error = ?HitboxGenerationError::ColliderParseError {
                        e_master,
                        node_id: node_id.to_string(),
                        err,
                        json: extras.value.to_owned(),
                    });
                    continue;
                }
            };

            trace!(name=?node_id, attrs=?collider_attrs);

            // bruhhhhhhhhh... why does it have to be so zesty
            // if there was a third one I would just macro it
            match collider_attrs.hitbox_category() {
                HitboxCategoryIdentifier::Hitbox => {
                    let Ok(ref mut hitbox_manager) = hitbox_manager else {
                        error!(error = ?HitboxGenerationError::HitboxManagerNotFound {
                            e_master,
                            node_id: node_id.to_string(),
                            ty: *collider_attrs.hitbox_category(),
                        });
                        continue;
                    };

                    if let Err(err) = hitbox_manager.colliders.try_insert(node_id, e_hitbox) {
                        error!(error = ?HitboxGenerationError::DupedNodeId {
                            e_master,
                            err,
                        });
                        continue;
                    }

                    commands
                        .entity(e_hitbox)
                        .insert((Hitbox { target: e_master }, Collider::from(collider_attrs)))
                        .insert(Hitboxes::template());
                }
                HitboxCategoryIdentifier::Hurtbox => {
                    let Ok(ref mut hitbox_manager) = hurtbox_manager else {
                        error!(error = ?HitboxGenerationError::HitboxManagerNotFound {
                            e_master,
                            node_id: node_id.to_string(),
                            ty: *collider_attrs.hitbox_category(),
                        });
                        continue;
                    };

                    if let Err(err) = hitbox_manager.colliders.try_insert(node_id, e_hitbox) {
                        error!(error = ?HitboxGenerationError::DupedNodeId {
                            e_master,
                            err,
                        });
                        continue;
                    }

                    commands
                        .entity(e_hitbox)
                        .insert((Hitbox { target: e_master }, Collider::from(collider_attrs)))
                        .insert(Hurtboxes::template());
                }
            }
        }

        debug!(
            msg="Generated hitbox manager.",
            source=?scene_instance,
            entity=?e_master,
            hitboxes=?hitbox_manager.ok(),
            hurtboxes=?hurtbox_manager.ok(),
        );
    }
}

/// Propagates `HitboxManager` `CollisionGroups` to child hitboxes.
pub fn sync_hitbox_collision_groups<T: HitboxCategory>(
    hitbox_query: Query<(Entity, &CollisionGroups, &HitboxManager<T>), Changed<CollisionGroups>>,
    mut collision_groups_query: Query<&mut CollisionGroups, Without<HitboxManager<T>>>,
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
pub fn sync_hitbox_activation<T: HitboxCategory>(
    mut commands: Commands,
    hitbox_query: Query<(&ContactDamage, &HitboxManager<T>), Added<ContactDamage>>,
) {
    for (contact_damage, hitboxes) in hitbox_query.iter() {
        for &e_hitbox in hitboxes.colliders.values() {
            commands.entity(e_hitbox).insert(*contact_damage);
        }
    }
}

/// Inverse of `sync_hitbox_activation`.
pub fn sync_hitbox_deactivation<T: HitboxCategory>(
    mut commands: Commands,
    hitbox_query: Query<&HitboxManager<T>>,
    mut removed: RemovedComponents<ContactDamage>,
) {
    for e_hitboxes in removed.read() {
        let Ok(hitboxes) = hitbox_query.get(e_hitboxes) else {
            continue;
        };

        for &e_hitbox in hitboxes.colliders.values() {
            commands.entity(e_hitbox).remove::<ContactDamage>();
        }
    }
}

/// Note: Generated hitboxes use `RigidBody::KinematicPositionBased`.
#[derive(Component, Clone, Copy, Debug, Default)]
pub enum GltfHitboxAutoGenTarget {
    #[default]
    Here,
    Remote(Entity),
}
