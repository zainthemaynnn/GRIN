pub mod projectiles;

use std::time::Duration;

use bevy::{ecs::query::QueryEntityError, prelude::*, utils::HashMap};
use bevy_rapier3d::prelude::*;
use grin_util::query::distinguish_by_query;

use projectiles::ProjectilePlugin;

/// Health and damage calculations.
pub struct DamagePlugin;

impl Plugin for DamagePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ProjectilePlugin)
            .add_event::<DamageEvent>()
            .configure_sets(
                Update,
                (
                    DamageSet::Insert,
                    DamageSet::Resist,
                    DamageSet::Propagate,
                    DamageSet::Clear,
                    DamageSet::Kill,
                )
                    .chain(),
            )
            .add_systems(PreUpdate, send_contact_damage_events)
            .add_systems(
                Update,
                (
                    push_contact_damage.in_set(DamageSet::Insert),
                    propagate_damage_buffers.in_set(DamageSet::Propagate),
                    apply_resist.in_set(DamageSet::Resist),
                    apply_damage_buffers.in_set(DamageSet::Clear),
                    die.in_set(DamageSet::Kill),
                ),
            );
    }
}

#[derive(SystemSet, Debug, Hash, Eq, PartialEq, Copy, Clone)]
pub enum DamageSet {
    /// `DamageBuffer`s are empty in this stage.
    Insert,
    /// `DamageBuffer`s are propagated in this stage.
    Propagate,
    /// `Resist` is applied in this stage.
    Resist,
    /// `DamageBuffer`s are cleared in this stage.
    Clear,
    /// Things die in this stage.
    Kill,
}

/// Health. Is there anything more I can say?
///
/// Can't fall below zero.
#[derive(Component, Debug)]
pub struct Health(pub f32);

impl Default for Health {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Assigns resistances against `DamageVariant`s.
///
/// Scales linearly from no resist at `0.0` to full resist at `1.0`. All values default to `0.0`.
#[derive(Component, Debug, Default)]
pub struct Resist(HashMap<DamageVariant, f32>);

#[derive(Bundle, Default)]
pub struct HealthBundle {
    pub health: Health,
    pub resist: Resist,
    pub damage_buffer: DamageBuffer,
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

/// Applies damage to adjacent `Health` components. Clears every frame.
///
/// If there is no adjacent `Health` component, applies to the first ancestor containing one.
#[derive(Component, Default)]
pub struct DamageBuffer(pub Vec<Damage>);

/// Applies resistance by scaling `Damage` values.
pub fn apply_resist(mut query: Query<(&mut DamageBuffer, &Resist)>) {
    for (mut damage_buf, Resist(resist)) in query.iter_mut() {
        for damage in damage_buf.0.iter_mut() {
            let r = resist.get(&damage.ty).unwrap_or(&0.0);
            damage.value *= 1.0 - r;
        }
    }
}

/// Recursively empties `DamageBuffer`s for entities without a `Health` component
/// and appends them to the `DamageBuffer` of the first ancestor with a `Health` component.
pub fn propagate_damage_buffers(
    mut health_query: Query<(&mut DamageBuffer, &Children), With<Health>>,
    mut buffer_query: Query<Option<&mut DamageBuffer>, Without<Health>>,
    children_query: Query<&Children, Without<Health>>,
) {
    for (mut buffer, children) in health_query.iter_mut() {
        for child in children.iter() {
            propagate_damage_buffers_child(&mut buffer, *child, &mut buffer_query, &children_query);
        }
    }
}

fn propagate_damage_buffers_child(
    buffer: &mut DamageBuffer,
    child: Entity,
    buffer_query: &mut Query<Option<&mut DamageBuffer>, Without<Health>>,
    children_query: &Query<&Children, Without<Health>>,
) {
    if let Some(mut child_buffer) = buffer_query.get_mut(child).unwrap() {
        buffer.0.append(&mut child_buffer.0);
    }

    if let Ok(children) = children_query.get(child) {
        for child in children.iter() {
            propagate_damage_buffers_child(buffer, *child, buffer_query, children_query);
        }
    }
}

/// Applies damage values from `DamageBuffer`.
pub fn apply_damage_buffers(mut query: Query<(&mut Health, &mut DamageBuffer), Without<Dead>>) {
    for (mut health, mut damage_buf) in query.iter_mut() {
        for damage in damage_buf.0.drain(0..) {
            health.0 = (health.0 - damage.value).max(0.0);
            info!("health: {}", health.0);
        }
    }
}

/// Inserts `Dead` component.
pub fn die(mut commands: Commands, health_query: Query<(Entity, &Health)>) {
    for (entity, health) in health_query.iter() {
        if health.0 == 0.0 {
            commands.entity(entity).insert(Dead);
        }
    }
}

/// Sends `DamageEvent::Contact` on collision. If this component has an adjacent `Damage`
/// component, it will be applied automatically.
#[derive(Component, Default, Copy, Clone)]
pub enum ContactDamage {
    /// This entity is despawned after contact damage event is fired.
    #[default]
    Despawn,
    /// This component is removed after contact damage event is fired.
    Once,
    /// Contact damage events are disabled for `0`.
    Debounce(Duration),
}

/// PCs and NPCs with this are dead.
#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct Dead;

/// Event for when... something takes damage.
///
/// Note that this event is not fired when `DamageBuffer`s are updated. `DamageBuffer`s are lower
/// level structures that should be updated when `DamageEvent` is fired by something else.
#[derive(Event)]
pub enum DamageEvent {
    /// Collision damage.
    Contact {
        kind: ContactDamage,
        e_damage: Entity,
        e_hit: Entity,
    },
    /// Direct damage.
    // TODO: system to apply this damage, when it actually ends up getting used.
    Direct {
        damage: Damage,
        e_hit: Entity,
    }
}

pub fn send_contact_damage_events(
    damage_query: Query<&ContactDamage>,
    mut collision_events: EventReader<CollisionEvent>,
    mut damage_events: EventWriter<DamageEvent>,
) {
    for collision_event in collision_events.iter() {
        let CollisionEvent::Started(entity_0, entity_1, ..) = collision_event else {
            continue;
        };
        debug!("Collision for {:?} on {:?}.", entity_0, entity_1);

        let Ok((e_damage, e_hit)) = distinguish_by_query(&damage_query, *entity_0, *entity_1) else {
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
    for damage_event in damage_events.iter() {
        let DamageEvent::Contact { kind, e_damage, e_hit } = damage_event else {
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
        };

        if let Ok(damage) = try_push_damage(*e_damage, *e_hit, &damage_query, &mut hit_query) {
            debug!("Contact damage for {:?} on {:?}.", *damage, *e_hit);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage() {
        let mut app = App::new();
        app.add_systems(Update, apply_damage_buffers);

        let damage_dst = app
            .world
            .spawn((
                Health(100.0),
                DamageBuffer(vec![
                    Damage {
                        ty: DamageVariant::Ballistic,
                        value: 60.0,
                        source: None,
                    },
                    Damage {
                        ty: DamageVariant::Ballistic,
                        value: 20.0,
                        source: None,
                    },
                ]),
            ))
            .id();

        app.update();

        assert_eq!(
            app.world.get::<Health>(damage_dst).unwrap().0,
            20.0,
            "Inaccurate damage calculation."
        );

        app.update();

        assert_eq!(
            app.world.get::<Health>(damage_dst).unwrap().0,
            20.0,
            "`DamageBuffer` persisted between frames."
        );

        app.world
            .get_mut::<DamageBuffer>(damage_dst)
            .unwrap()
            .0
            .push(Damage {
                ty: DamageVariant::Ballistic,
                value: 999.0,
                source: None,
            });

        app.update();

        assert_eq!(
            app.world.get::<Health>(damage_dst).unwrap().0,
            0.0,
            "Health fell below zero."
        );
    }

    #[test]
    fn resist() {
        let mut app = App::new();
        app.add_systems(Update, (apply_resist, apply_damage_buffers).chain());

        let damage_dst = app
            .world
            .spawn((
                Health(100.0),
                Resist(HashMap::from([(DamageVariant::Ballistic, 0.5)])),
                DamageBuffer(vec![Damage {
                    ty: DamageVariant::Ballistic,
                    value: 100.0,
                    source: None,
                }]),
            ))
            .id();

        app.update();

        assert_eq!(
            app.world.get::<Health>(damage_dst).unwrap().0,
            50.0,
            "Inaccurate resist calculation.",
        );
    }

    #[test]
    fn propagation() {
        let mut app = App::new();
        app.add_systems(Update, propagate_damage_buffers);

        let child = app.world.spawn(DamageBuffer(vec![Damage::default()])).id();

        let damage_dst = app
            .world
            .spawn((Health(100.0), DamageBuffer(vec![])))
            .add_child(child)
            .id();

        app.update();

        assert_eq!(
            app.world.get::<DamageBuffer>(damage_dst).unwrap().0.len(),
            1,
            "`Damage` was not propagated",
        );

        assert_eq!(
            app.world.get::<DamageBuffer>(child).unwrap().0.len(),
            0,
            "Child `DamageBuffer` was not cleared.",
        );
    }

    #[test]
    fn death() {
        let mut app = App::new();
        app.add_systems(Update, die);

        let e = app.world.spawn(Health(0.0)).id();

        app.update();

        assert!(app.world.entity(e).contains::<Dead>());
    }
}
