use bevy::{prelude::*, utils::HashMap};
use bevy_rapier3d::prelude::*;

use crate::{render::sketched::SketchMaterial, util::query::distinguish_by_query};

/// Health and damage calculations.
pub struct DamagePlugin;

impl Plugin for DamagePlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            (
                DamageSet::Insert,
                DamageSet::Resist,
                DamageSet::Clear,
                DamageSet::Kill,
            )
                .chain(),
        )
        .add_systems((
            push_contact_damage.in_set(DamageSet::Insert),
            propagate_damage_buffers,
            apply_resist.in_set(DamageSet::Resist),
            apply_damage_buffers.in_set(DamageSet::Clear),
            die.in_set(DamageSet::Kill),
        ));
    }
}

#[derive(SystemSet, Debug, Hash, Eq, PartialEq, Copy, Clone)]
pub enum DamageSet {
    /// `DamageBuffer`s are empty in this stage.
    Insert,
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
///
/// `source` refers to the `Entity` that dealt the damage.
#[derive(Component, Copy, Clone, Debug, Default)]
pub struct Damage {
    pub ty: DamageVariant,
    pub value: f32,
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
        for mut damage in damage_buf.0.iter_mut() {
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

pub fn die(mut commands: Commands, health_query: Query<(Entity, &Health)>) {
    for (entity, health) in health_query.iter() {
        if health.0 == 0.0 {
            commands.entity(entity).insert(Dead);
        }
    }
}

/// Items colliding with this entity will have damage propagated to it.
#[derive(Component, Default)]
pub struct ContactDamage;

/// PCs and NPCs with this are dead.
#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct Dead;

pub fn push_contact_damage(
    mut commands: Commands,
    mut collision_events: EventReader<CollisionEvent>,
    damage_query: Query<&Damage, With<ContactDamage>>,
    mut hit_query: Query<&mut DamageBuffer>,
) {
    for collision_event in collision_events.iter() {
        let CollisionEvent::Started(entity_0, entity_1, ..) = collision_event else {
            continue;
        };
        let Ok((e_damage, e_hit)) = distinguish_by_query(&damage_query, *entity_0, *entity_1) else {
            continue;
        };

        if let Some(mut e) = commands.get_entity(e_damage) {
            e.despawn();
        }
        let damage = damage_query.get(e_damage).unwrap();
        let Ok(mut damage_buf) = hit_query.get_mut(e_hit) else {
            continue;
        };
        damage_buf.0.push(*damage);
    }
}

#[derive(Bundle)]
pub struct ProjectileBundle {
    pub body: RigidBody,
    pub material_mesh: MaterialMeshBundle<SketchMaterial>,
    pub collider: Collider,
    pub collision_groups: CollisionGroups,
    pub velocity: Velocity,
    pub sensor: Sensor,
    pub active_events: ActiveEvents,
    pub gravity: GravityScale,
    pub damage: Damage,
    pub contact_damage: ContactDamage,
}

impl Default for ProjectileBundle {
    fn default() -> Self {
        Self {
            #[rustfmt::skip]
            active_events: ActiveEvents::COLLISION_EVENTS,
            gravity: GravityScale(0.0),
            collision_groups: CollisionGroups::default(),
            body: RigidBody::default(),
            material_mesh: MaterialMeshBundle::default(),
            collider: Collider::default(),
            velocity: Velocity::default(),
            sensor: Sensor::default(),
            damage: Damage::default(),
            contact_damage: ContactDamage::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage() {
        let mut app = App::new();
        app.add_system(apply_damage_buffers);

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
        app.add_systems((apply_resist, apply_damage_buffers).chain());

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
        app.add_system(propagate_damage_buffers);

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
        app.add_system(die);

        let e = app.world.spawn(Health(0.0)).id();

        app.update();

        assert!(app.world.entity(e).contains::<Dead>());
    }
}
