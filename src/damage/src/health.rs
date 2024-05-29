use bevy::{prelude::*, utils::HashMap};

use crate::{
    hit::{Damage, DamageVariant},
    hitbox::Hitbox,
    plugin::DamageSet,
};

pub struct HealthPlugin;

impl Plugin for HealthPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                propagate_damage_buffers.in_set(DamageSet::Propagate),
                apply_resist.in_set(DamageSet::Resist),
                apply_damage_buffers.in_set(DamageSet::Clear),
                die.in_set(DamageSet::Kill),
            ),
        );
    }
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
    mut health_query: Query<&mut DamageBuffer, With<Health>>,
    mut hitbox_query: Query<(&Hitbox, &mut DamageBuffer), Without<Health>>,
) {
    for (Hitbox { target: e_health }, mut src_buf) in hitbox_query.iter_mut() {
        let mut dst_buf = health_query.get_mut(*e_health).unwrap();
        dst_buf.0.append(&mut src_buf.0);
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

/// PCs and NPCs with this are dead.
#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct Dead;

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
