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
