use bevy::{
    ecs::query::{ReadOnlyWorldQuery, WorldQuery},
    prelude::*,
    scene::{InstanceId, SceneInstance},
};
use itertools::Itertools;

/// Matches two entities against a query. The entity that matches occurs first in the tuple.
///
/// If neither match, returns a `QueryEntityError`.
pub fn distinguish_by_query<Q: WorldQuery, F: ReadOnlyWorldQuery>(
    query: &Query<Q, F>,
    entity_0: Entity,
    entity_1: Entity,
) -> Option<(Entity, Entity)> {
    match query.contains(entity_0) {
        true => Some((entity_0, entity_1)),
        false => match query.contains(entity_1) {
            true => Some((entity_1, entity_0)),
            false => None,
        },
    }
}

/// Finds an `Entity` corresponding to the `EntityPath` from `root`.
pub fn gltf_path_search(
    path: &EntityPath,
    root: Entity,
    children_query: &Query<&Children>,
    name_query: &Query<&Name>,
) -> Result<Entity, ()> {
    let mut current_entity = root;

    for part in path.parts.iter() {
        let mut found = false;
        if let Ok(children) = children_query.get(current_entity) {
            for child in children.iter() {
                if let Ok(name) = name_query.get(*child) {
                    if name == part {
                        // Found a children with the right name, continue to the next part
                        current_entity = *child;
                        found = true;
                        break;
                    }
                }
            }
        }
        if !found {
            warn!("Entity not found for path {:?} on part {:?}", path, part);
            return Err(());
        }
    }

    Ok(current_entity)
}

/// Searches the GLTF tree for all nodes with the corresponding prefix.
pub fn gltf_prefix_search(
    prefix: &str,
    root: &InstanceId,
    scene_manager: &SceneSpawner,
    name_query: &Query<&Name>,
) -> Vec<Entity> {
    return scene_manager
        .iter_instance_entities(*root)
        .filter(|e_node| match name_query.get(*e_node) {
            Ok(name) => name.starts_with(prefix),
            Err(..) => false,
        })
        .collect_vec();
}

pub struct PotentialAncestorIter<'w, 's, Q: WorldQuery, F: ReadOnlyWorldQuery>
where
    Q::ReadOnly: WorldQuery<Item<'w> = &'w Parent>,
{
    parent_query: &'w Query<'w, 's, Q, ()>,
    filter_query: &'w Query<'w, 's, (), F>,
    next: Option<Entity>,
}

impl<'w, 's, Q: WorldQuery, F: ReadOnlyWorldQuery> PotentialAncestorIter<'w, 's, Q, F>
where
    Q::ReadOnly: WorldQuery<Item<'w> = &'w Parent>,
{
    pub fn new(
        parent_query: &'w Query<'w, 's, Q, ()>,
        filter_query: &'w Query<'w, 's, (), F>,
        entity: Entity,
    ) -> Self {
        PotentialAncestorIter {
            parent_query,
            filter_query,
            next: Some(entity),
        }
    }
}

impl<'w, 's, Q: WorldQuery, F: ReadOnlyWorldQuery> Iterator for PotentialAncestorIter<'w, 's, Q, F>
where
    Q::ReadOnly: WorldQuery<Item<'w> = &'w Parent>,
{
    type Item = Entity;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            self.next = self.parent_query.get(self.next?).ok().map(|p| p.get());
            match self.next {
                Some(e_next) => {
                    self.next = Some(e_next);
                    if self.filter_query.get(e_next).is_ok() {
                        return self.next;
                    }
                }
                None => {
                    return None;
                }
            }
        }
    }
}

/// Designates the marker component `T` as an initializer for `SceneInstance` entities.
///
/// This removes the component when the scene is ready, and returns the corresponding entities.
/// Ideally you should pipe this into other systems. It does not use mutable access.
//
// this is courtesy of here. thanks: https://github.com/bevyengine/bevy/discussions/8533
pub fn labelled_scene_initializer<T: Component>(
    mut commands: Commands,
    scene_manager: Res<SceneSpawner>,
    unloaded_instances: Query<(Entity, &SceneInstance), With<T>>,
) -> Vec<(Entity, InstanceId)> {
    let mut scenes = Vec::new();
    for (entity, instance) in unloaded_instances.iter() {
        if scene_manager.instance_is_ready(**instance) {
            commands.entity(entity).remove::<T>();
            // I dunno if duping the uuid is really necessary, but it's nice to have so far
            scenes.push((entity, **instance));
        }
    }
    return scenes;
}

/// Variant of `labelled_scene_intializer` which additionally provides a clone of `T`.
pub fn cloned_scene_initializer<T: Component + Clone>(
    mut commands: Commands,
    scene_manager: Res<SceneSpawner>,
    unloaded_instances: Query<(Entity, &SceneInstance, &T)>,
) -> Vec<(Entity, InstanceId, T)> {
    let mut scenes = Vec::new();
    for (entity, instance, init) in unloaded_instances.iter() {
        if scene_manager.instance_is_ready(**instance) {
            commands.entity(entity).remove::<T>();
            scenes.push((entity, **instance, init.clone()));
        }
    }
    return scenes;
}
