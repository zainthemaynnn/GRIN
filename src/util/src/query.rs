use bevy::{
    ecs::query::{QueryEntityError, ReadOnlyWorldQuery, WorldQuery},
    prelude::*,
};

/// Matches two entities against a query. The entity that matches occurs first in the tuple.
///
/// If neither match, returns a `QueryEntityError`.
pub fn distinguish_by_query<Q: WorldQuery, F: ReadOnlyWorldQuery>(
    query: &Query<Q, F>,
    entity_0: Entity,
    entity_1: Entity,
) -> Result<(Entity, Entity), QueryEntityError> {
    match query.get(entity_0) {
        Ok(_) => Ok((entity_0, entity_1)),
        Err(_) => match query.get(entity_1) {
            Ok(_) => Ok((entity_1, entity_0)),
            Err(e) => Err(e),
        },
    }
}

/// Finds an `Entity` corresponding to the `EntityPath` from `root`.
pub fn gltf_path_search(
    path: &EntityPath,
    root: Entity,
    children: &Query<&Children>,
    names: &Query<&Name>,
) -> Result<Entity, ()> {
    let mut current_entity = root;

    for part in path.parts.iter() {
        let mut found = false;
        if let Ok(children) = children.get(current_entity) {
            for child in children.iter() {
                if let Ok(name) = names.get(*child) {
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
    pub fn new(parent_query: &'w Query<'w, 's, Q, ()>, filter_query: &'w Query<'w, 's, (), F>, entity: Entity) -> Self {
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
