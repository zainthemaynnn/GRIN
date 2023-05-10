pub mod dummy;
pub mod movement;

use bevy::{app::PluginGroupBuilder, prelude::*};

use crate::item::{Equipped, Target, Weapon};

use self::{dummy::DummyPlugin, movement::MoveTarget};

pub struct AIPlugins;

impl PluginGroup for AIPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>().add(DummyPlugin)
    }
}

pub fn set_closest_target<T: Component, E: Component>(
    mut self_query: Query<(&mut Target, &GlobalTransform), With<T>>,
    target_query: Query<&GlobalTransform, With<E>>,
) {
    for (mut target, src_transform) in self_query.iter_mut() {
        let mut new_target = Target::default();
        for dst_transform in target_query.iter() {
            let distance = src_transform
                .translation()
                .distance(dst_transform.translation());
            if distance < new_target.distance {
                new_target = Target {
                    // TODO?: could this height just be a constant 1.50?
                    transform: Transform::from_translation(
                        dst_transform.transform_point(Vec3::new(0.0, 1.50, 0.0)),
                    ),
                    distance,
                };
            }
        }
        *target = new_target;
    }
}

pub fn propagate_move_target<E: Component>(mut query: Query<(&Target, &mut MoveTarget), With<E>>) {
    for (target, mut move_target) in query.iter_mut() {
        move_target.0 = target.transform;
    }
}

pub fn propagate_item_target<E: Component>(
    dummy_query: Query<(&Target, &Equipped), With<E>>,
    mut weapon_query: Query<&mut Target, (With<Weapon>, Without<E>)>,
) {
    for (target, Equipped(equipped)) in dummy_query.iter() {
        let mut weapons_it = weapon_query.iter_many_mut(equipped);
        while let Some(mut weapon_target) = weapons_it.fetch_next() {
            *weapon_target = *target;
        }
    }
}
