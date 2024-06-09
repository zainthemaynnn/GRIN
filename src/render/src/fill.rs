use std::ops::Range;

use bevy::{prelude::*, render::mesh::VertexAttributeValues, scene::SceneInstance};
use bevy_tweening::{component_animator_system, Lens};
use grin_util::spatial::{ComputeSceneAabb, SceneAabb};

use crate::{sketched::ATTRIBUTE_Y_CUTOFF, EffectFlags, TweenAppExt, TweenCompletedEvent};

pub struct FillPlugin;

impl Plugin for FillPlugin {
    fn build(&self, app: &mut App) {
        app.add_tween_completion_event::<FillCompletedEvent>()
            .add_systems(First, fill_y_cutoff_buffers)
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
#[derive(Component, Clone, Debug)]
pub struct FillEffect {
    /// Proportion of the fill effect that has been completed.
    pub t: f32,
    /// Rate of change for `t` per second.
    pub rate: f32,
    /// How the maximum height will be evaluated.
    pub bounds: HeightBounds,
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

impl Default for FillEffect {
    fn default() -> Self {
        Self {
            t: 0.0,
            rate: 1.0,
            bounds: HeightBounds::default(),
        }
    }
}

pub fn fill_y_cutoff_buffers(
    mut meshes: ResMut<Assets<Mesh>>,
    mut asset_events: EventReader<AssetEvent<Mesh>>,
) {
    for asset_event in asset_events.read() {
        let AssetEvent::LoadedWithDependencies { id } = asset_event else {
            continue;
        };

        let mesh = meshes.get_mut(*id).unwrap();
        // idk how to get the actual vert count... this works lol
        let num_verts = mesh.attributes().next().unwrap().1.len();
        mesh.insert_attribute(ATTRIBUTE_Y_CUTOFF, vec![f32::MAX; num_verts]);

        trace!(
            msg="Set y cutoff to `f32::MAX`.",
            asset_id=?id,
        );
    }
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
    mut meshes: ResMut<Assets<Mesh>>,
    effect_query: Query<(&SceneInstance, &FillEffect, Option<&SceneAabb>)>,
    mesh_query: Query<&Handle<Mesh>>,
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

        for e_mesh in scene_spawner.iter_instance_entities(**scene_id) {
            let Ok(h_mesh) = mesh_query.get(e_mesh) else {
                continue;
            };

            let Some(mesh) = meshes.get_mut(h_mesh) else {
                continue;
            };

            let Some(VertexAttributeValues::Float32(ref mut buf)) =
                mesh.attribute_mut(ATTRIBUTE_Y_CUTOFF)
            else {
                warn!(
                    msg="Mesh doesn't support `y_cutoff`.",
                    mesh_id=?h_mesh.id(),
                    attr=ATTRIBUTE_Y_CUTOFF.name,
                    found=?mesh.attribute(ATTRIBUTE_Y_CUTOFF),
                );
                continue;
            };

            buf.fill(y_cutoff);
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
    mut meshes: ResMut<Assets<Mesh>>,
    effect_query: Query<(Entity, &SceneInstance, Option<&EffectFlags>)>,
    mesh_query: Query<&Handle<Mesh>>,
    mut finished: EventReader<FillCompletedEvent>,
) {
    for (e_effect, scene_id, flags) in finished.read().filter_map(|ev| effect_query.get(ev.0).ok())
    {
        let flags = flags.copied().unwrap_or_default();
        if flags.intersects(EffectFlags::REZERO) {
            for e_mesh in scene_spawner.iter_instance_entities(**scene_id) {
                let Ok(h_mesh) = mesh_query.get(e_mesh) else {
                    continue;
                };

                let Some(mesh) = meshes.get_mut(h_mesh) else {
                    continue;
                };

                let Some(VertexAttributeValues::Float32(ref mut buf)) =
                    mesh.attribute_mut(ATTRIBUTE_Y_CUTOFF)
                else {
                    continue;
                };

                buf.fill(f32::MAX);
            }
        }

        if flags.intersects(EffectFlags::DESPAWN) {
            commands.entity(e_effect).remove::<FillEffect>();
        }
    }
}
