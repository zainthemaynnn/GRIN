use std::time::Duration;

use bevy::{ecs::query::QueryEntityError, prelude::*};
use bevy_rapier3d::prelude::*;
use grin_util::query::distinguish_by_query;

use crate::{health::DamageBuffer, plugin::DamageSet};

pub struct ContactDamagePlugin;

impl Plugin for ContactDamagePlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<DamageEvent>()
            .add_systems(PreUpdate, send_contact_damage_events)
            .add_systems(Update, push_contact_damage.in_set(DamageSet::Add));
    }
}
/// Damage variant.
#[derive(Component, Hash, Eq, PartialEq, Copy, Clone, Debug, Default)]
pub enum DamageVariant {
    #[default]
    Ballistic,
}

/// Damage.
#[derive(Component, Copy, Clone, Debug, Default)]
pub struct Damage {
    /// Damage type.
    pub ty: DamageVariant,
    /// Damage amount.
    pub value: f32,
    /// The `Entity` that dealt the damage.
    pub source: Option<Entity>,
}

/// Collision groups to use when dealing damage.
#[derive(Component, Copy, Clone, Default)]
pub struct DamageCollisionGroups(pub CollisionGroups);

impl From<&DamageCollisionGroups> for CollisionGroups {
    fn from(value: &DamageCollisionGroups) -> Self {
        value.0
    }
}

/// Sends `DamageEvent::Contact` on collision. If this component has an adjacent `Damage`
/// component, it will be applied automatically.
#[derive(Component, Default, Copy, Clone, Debug)]
pub enum ContactDamage {
    /// Disabled.
    #[default]
    None,
    /// This entity is despawned after contact damage event is fired.
    Despawn,
    /// This component is removed after contact damage event is fired.
    Once,
    /// Contact damage events are disabled for `0`.
    Debounce(Duration),
}

/// Event for when... something takes damage.
///
/// Note that this event is not fired when `DamageBuffer`s are updated. `DamageBuffer`s are lower
/// level structures that should be updated when `DamageEvent` is fired by something else.
#[derive(Event, Debug, Clone)]
pub enum DamageEvent {
    /// Collision damage.
    Contact {
        kind: ContactDamage,
        e_damage: Entity,
        e_hit: Entity,
    },
    /// Direct damage.
    // TODO: system to apply this damage, when it actually ends up getting used.
    Direct { damage: Damage, e_hit: Entity },
}

pub fn send_contact_damage_events(
    damage_query: Query<&ContactDamage>,
    mut collision_events: EventReader<CollisionEvent>,
    mut damage_events: EventWriter<DamageEvent>,
) {
    for collision_event in collision_events.read() {
        let CollisionEvent::Started(entity_0, entity_1, ..) = collision_event else {
            continue;
        };
        trace!("Collision for {:?} on {:?}.", entity_0, entity_1);

        let Some((e_damage, e_hit)) = distinguish_by_query(&damage_query, *entity_0, *entity_1)
        else {
            continue;
        };

        let contact_damage_kind = damage_query.get(e_damage).unwrap();

        damage_events.send(DamageEvent::Contact {
            kind: *contact_damage_kind,
            e_damage,
            e_hit,
        })
    }
}

fn try_push_damage<'a>(
    e_damage: Entity,
    e_hit: Entity,
    damage_query: &'a Query<&Damage>,
    hit_query: &mut Query<&mut DamageBuffer>,
) -> Result<&'a Damage, QueryEntityError> {
    let damage = damage_query.get(e_damage)?;
    let mut damage_buf = hit_query.get_mut(e_hit)?;
    damage_buf.0.push(*damage);
    Ok(damage)
}

pub fn push_contact_damage(
    mut commands: Commands,
    mut hit_query: Query<&mut DamageBuffer>,
    mut damage_events: EventReader<DamageEvent>,
    damage_query: Query<&Damage>,
) {
    for damage_event in damage_events.read() {
        let DamageEvent::Contact {
            kind,
            e_damage,
            e_hit,
        } = damage_event
        else {
            continue;
        };

        match kind {
            ContactDamage::Despawn => {
                commands.get_or_spawn(*e_damage).despawn_recursive();
            }
            ContactDamage::Once => {
                commands.get_or_spawn(*e_damage).remove::<ContactDamage>();
            }
            ContactDamage::Debounce(_debounce) => {
                todo!();
            }
            ContactDamage::None => continue,
        };

        if let Ok(damage) = try_push_damage(*e_damage, *e_hit, &damage_query, &mut hit_query) {
            debug!("Contact damage for {:?} on {:?}.", *damage, *e_hit);
        }
    }
}
