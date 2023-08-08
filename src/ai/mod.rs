pub mod dummy;
pub mod movement;

use bevy::{app::PluginGroupBuilder, prelude::*};

use crate::{
    damage::Dead,
    humanoid::HUMANOID_HEIGHT,
    item::{Equipped, Target, Weapon},
    time::Rewind,
};

use self::{dummy::DummyPlugin, movement::MoveTarget};

pub struct AIPlugins;

impl PluginGroup for AIPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(DummyPlugin)
            .add(dummy_2d::DummyPlugin)
    }
}

pub fn set_closest_target<T: Component, E: Component>(
    mut self_query: Query<
        (&mut Target, &GlobalTransform),
        (With<T>, Without<Rewind>, Without<Dead>),
    >,
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
                    transform: dst_transform.compute_transform().with_translation(
                        dst_transform.transform_point(Vec3::new(0.0, HUMANOID_HEIGHT / 2.0, 0.0)),
                    ),
                    distance,
                };
            }
        }
        *target = new_target;
        trace!("Player target: {:?}", target);
    }
}

pub fn propagate_move_target<T: Component>(
    mut query: Query<(&Target, &mut MoveTarget), (With<T>, Without<Rewind>, Without<Dead>)>,
) {
    for (target, mut move_target) in query.iter_mut() {
        move_target.0 = target.transform;
    }
}
