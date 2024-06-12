use std::ops::Range;

use bevy::{prelude::*, scene::SceneInstance};
use bevy_tweening::{component_animator_system, Lens};
use grin_util::spatial::{ComputeSceneAabb, SceneAabb};

use crate::{
    sketched::{MaterialMutationResource, SketchMaterial},
    EffectFlags, TweenAppExt, TweenCompletedEvent,
};

pub struct FillPlugin;

impl Plugin for FillPlugin {
    fn build(&self, app: &mut App) {
        app.add_tween_completion_event::<FillCompletedEvent>()
            .add_systems(Update, precalculate_aabbs)
            .add_systems(
                PostUpdate,
                (
                    (component_animator_system::<FillEffect>, set_fill_cutoffs).chain(),
                    complete_fills,
                ),
            );
    }
}

/// It's like a... glass-filling-with-liquid kinda thing.
///
/// This should be placed alongside a `SceneAabb` to work.
#[derive(Component, Clone, Debug, Default)]
pub struct FillEffect {
    /// Proportion of the fill effect that has been completed.
    pub t: f32,
    /// How the maximum height will be evaluated.
    pub bounds: HeightBounds,
    /// Effect flags.
    pub flags: EffectFlags,
}

#[derive(Component, Default)]
pub struct FillParamLens {
    pub start: f32,
    pub end: f32,
}

impl Lens<FillEffect> for FillParamLens {
    fn lerp(&mut self, target: &mut FillEffect, ratio: f32) {
        target.t = self.start.lerp(self.end, ratio);
    }
}

#[derive(Clone, Debug, Default)]
pub enum HeightBounds {
    Value(Range<f32>),
    #[default]
    UseAabb,
}

pub fn precalculate_aabbs(
    mut commands: Commands,
    effect_query: Query<(Entity, &FillEffect, Option<&ComputeSceneAabb>), Added<FillEffect>>,
) {
    for (e_effect, effect, aabb) in effect_query.iter() {
        if matches!(effect.bounds, HeightBounds::UseAabb) && aabb.is_none() {
            commands.entity(e_effect).insert(ComputeSceneAabb);
        }
    }
}

pub fn set_fill_cutoffs(
    scene_spawner: Res<SceneSpawner>,
    mut materials: ResMut<Assets<SketchMaterial>>,
    mut material_mutation: ResMut<MaterialMutationResource>,
    effect_query: Query<(&SceneInstance, &FillEffect, Option<&SceneAabb>)>,
    mut material_query: Query<&mut Handle<SketchMaterial>>,
) {
    for (scene_id, effect, aabb) in effect_query.iter() {
        if !scene_spawner.instance_is_ready(**scene_id) {
            continue;
        }

        let bounds = match &effect.bounds {
            HeightBounds::Value(bounds) => bounds.clone(),
            HeightBounds::UseAabb => match aabb {
                Some(aabb) => aabb.min.y..aabb.max.y,
                None => continue,
            },
        };

        let y_cutoff = bounds.start.lerp(bounds.end, effect.t);
        trace!(msg = "Setting y cutoff.", y_cutoff = y_cutoff,);

        for e_material in scene_spawner.iter_instance_entities(**scene_id) {
            let Ok(mut h_material) = material_query.get_mut(e_material) else {
                continue;
            };

            if let Some(h_mod_material) =
                material_mutation.modify(&mut materials, &h_material, |mat| {
                    mat.extension.y_cutoff = y_cutoff;
                })
            {
                *h_material = h_mod_material;
            }
        }
    }
}

#[derive(Event)]
pub struct FillCompletedEvent(pub Entity);

impl From<Entity> for FillCompletedEvent {
    fn from(value: Entity) -> Self {
        Self(value)
    }
}

impl TweenCompletedEvent for FillCompletedEvent {
    const EVENT_ID: u64 = 12379087;
}

pub fn complete_fills(
    mut commands: Commands,
    scene_spawner: Res<SceneSpawner>,
    mut material_mutation: ResMut<MaterialMutationResource>,
    effect_query: Query<(Entity, &SceneInstance, &FillEffect)>,
    mut material_query: Query<&mut Handle<SketchMaterial>>,
    mut finished: EventReader<FillCompletedEvent>,
) {
    for (e_effect, scene_id, FillEffect { flags, .. }) in
        finished.read().filter_map(|ev| effect_query.get(ev.0).ok())
    {
        if flags.intersects(EffectFlags::REZERO) {
            for e_material in scene_spawner.iter_instance_entities(**scene_id) {
                let Ok(mut h_material) = material_query.get_mut(e_material) else {
                    continue;
                };

                if let Ok(h_base_material) = material_mutation.pop_base(&h_material.id()) {
                    *h_material = h_base_material;
                }
            }
        }

        if flags.intersects(EffectFlags::DESPAWN) {
            commands.entity(e_effect).remove::<FillEffect>();
        }
    }
}
