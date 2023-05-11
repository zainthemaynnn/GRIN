use bevy::{prelude::*, utils::HashMap};

/// Health and damage calculations.
pub struct DamagePlugin;

impl Plugin for DamagePlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets((DamageSet::Resist, DamageSet::Clear).chain())
            .add_systems((
                apply_resist.in_set(DamageSet::Resist),
                apply_damage_buffers.in_set(DamageSet::Clear),
            ));
    }
}

#[derive(SystemSet, Debug, Hash, Eq, PartialEq, Copy, Clone)]
pub enum DamageSet {
    /// `Resist` is applied in this stage.
    Resist,
    /// `DamageBuffer`s are cleared in this stage.
    Clear,
}

/// Health. Is there anything more I can say?
///
/// Can't fall below zero.
#[derive(Component)]
pub struct Health(pub f32);

impl Default for Health {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Assigns resistances for `DamageVariant`s.
///
/// Scales linearly from no resist at `0.0` to full resist at `1.0`. All values default to `0.0`.
#[derive(Component, Default)]
pub struct Resist(HashMap<DamageVariant, f32>);

#[derive(Bundle)]
pub struct HealthBundle {
    pub health: Health,
    pub resist: Resist,
}

/// Damage type.
#[derive(Component, Hash, Eq, PartialEq)]
pub enum DamageVariant {
    Ballistic,
}

/// Damage.
///
/// `source` refers to the `Entity` that dealt the damage.
#[derive(Component)]
pub struct Damage {
    pub ty: DamageVariant,
    pub value: f32,
    pub source: Entity,
}

/// Applies damage to adjacent health components. Clears every frame.
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

/// Applies damage values from `DamageBuffer`.
pub fn apply_damage_buffers(mut query: Query<(&mut Health, &mut DamageBuffer)>) {
    for (mut health, mut damage_buf) in query.iter_mut() {
        for damage in damage_buf.0.drain(0..) {
            health.0 = (health.0 - damage.value).max(0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_test() {
        let mut app = App::new();
        app.add_system(apply_damage_buffers);

        let damage_src = app.world.spawn_empty().id();
        let damage_dst = app
            .world
            .spawn((
                Health(100.0),
                DamageBuffer(vec![
                    Damage {
                        ty: DamageVariant::Ballistic,
                        value: 60.0,
                        source: damage_src,
                    },
                    Damage {
                        ty: DamageVariant::Ballistic,
                        value: 20.0,
                        source: damage_src,
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
                source: damage_src,
            });

        app.update();

        assert_eq!(
            app.world.get::<Health>(damage_dst).unwrap().0,
            0.0,
            "Health fell below zero."
        );
    }

    #[test]
    fn resist_test() {
        let mut app = App::new();
        app.add_systems((apply_resist, apply_damage_buffers).chain());

        let damage_src = app.world.spawn_empty().id();
        let damage_dst = app
            .world
            .spawn((
                Health(100.0),
                Resist(HashMap::from([(DamageVariant::Ballistic, 0.5)])),
                DamageBuffer(vec![Damage {
                    ty: DamageVariant::Ballistic,
                    value: 100.0,
                    source: damage_src,
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
}
