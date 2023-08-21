pub mod boombox;
pub mod dummy;
pub mod movement;

use bevy::{app::PluginGroupBuilder, prelude::*};

use crate::{damage::Dead, item::Target, time::Rewind};

use self::{boombox::BoomBoxPlugin, dummy::DummyPlugin, movement::AttackTarget};

pub struct AIPlugins;

impl PluginGroup for AIPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(DummyPlugin)
            .add(BoomBoxPlugin)
    }
}

pub fn set_closest_attack_target<T: Component, E: Component>(
    mut commands: Commands,
    mut self_query: Query<(Entity, &GlobalTransform), (With<T>, Without<Rewind>, Without<Dead>)>,
    target_query: Query<(Entity, &GlobalTransform), With<E>>,
) {
    for (e_agent, src_transform) in self_query.iter_mut() {
        let mut new_target = None;
        let mut target_distance = f32::MAX;
        for (e_target, dst_transform) in target_query.iter() {
            let distance = src_transform
                .translation()
                .distance(dst_transform.translation());
            if distance < target_distance {
                new_target = Some(AttackTarget(e_target));
                target_distance = distance;
            }
        }

        if let Some(t) = new_target {
            commands.entity(e_agent).insert(t);
            trace!("Target: {:?}", t);
        } else {
            commands.entity(e_agent).remove::<AttackTarget>();
            trace!("Target removed.");
        }
    }
}

pub fn propagate_attack_target_to_weapon<T: Component>(
    mut agent_query: Query<(&AttackTarget, &mut Target), (With<T>, Without<Rewind>, Without<Dead>)>,
    transform_query: Query<&Transform, Without<T>>,
) {
    for (AttackTarget(e_target), mut target) in agent_query.iter_mut() {
        *target = Target {
            transform: *transform_query.get(*e_target).unwrap(),
            distance: 1.0,
        };
    }
}
