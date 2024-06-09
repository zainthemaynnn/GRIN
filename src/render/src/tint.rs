use bevy::{prelude::*, render::mesh::VertexAttributeValues, scene::SceneInstance};
use bevy_tweening::{component_animator_system, Lens};
use grin_util::color::ColorExt;

use crate::{EffectFlags, TweenAppExt, TweenCompletedEvent};

pub struct TintPlugin;

impl Plugin for TintPlugin {
    fn build(&self, app: &mut App) {
        app.add_tween_completion_event::<TintCompletedEvent>()
            .add_systems(
                PostUpdate,
                (
                    (component_animator_system::<TintEffect>, set_vertex_colors).chain(),
                    complete_tints,
                ),
            );
    }
}

/// Just an easy way to change the `Mesh::ATTRIBUTE_COLOR` uniformly.
///
/// Note: Alpha also applies.
#[derive(Component, Clone, Debug, Default)]
pub struct TintEffect {
    /// Tint color.
    pub color: Color,
}

#[derive(Component, Default)]
pub struct TintParamLens {
    pub start: Color,
    pub end: Color,
}

impl Lens<TintEffect> for TintParamLens {
    fn lerp(&mut self, target: &mut TintEffect, ratio: f32) {
        target.color = self.start.lerp(&self.end, ratio);
    }
}

pub fn set_vertex_colors(
    scene_spawner: Res<SceneSpawner>,
    mut meshes: ResMut<Assets<Mesh>>,
    effect_query: Query<(&SceneInstance, &TintEffect)>,
    mesh_query: Query<&Handle<Mesh>>,
) {
    for (scene_id, effect) in effect_query.iter() {
        if !scene_spawner.instance_is_ready(**scene_id) {
            continue;
        }

        let tint = effect.color.as_rgba_f32();
        trace!(msg = "Setting tint color.", tint = ?tint,);

        for e_mesh in scene_spawner.iter_instance_entities(**scene_id) {
            let Ok(h_mesh) = mesh_query.get(e_mesh) else {
                continue;
            };

            let Some(mesh) = meshes.get_mut(h_mesh) else {
                continue;
            };

            let Some(VertexAttributeValues::Float32x4(ref mut buf)) =
                mesh.attribute_mut(Mesh::ATTRIBUTE_COLOR)
            else {
                warn!(
                    msg="Mesh doesn't support vertex colors.",
                    mesh_id=?h_mesh.id(),
                    attr=Mesh::ATTRIBUTE_COLOR.name,
                    found=?mesh.attribute(Mesh::ATTRIBUTE_COLOR),
                );
                continue;
            };

            buf.fill(tint);
        }
    }
}

#[derive(Event)]
pub struct TintCompletedEvent(pub Entity);

impl From<Entity> for TintCompletedEvent {
    fn from(value: Entity) -> Self {
        Self(value)
    }
}

impl TweenCompletedEvent for TintCompletedEvent {
    const EVENT_ID: u64 = 54327890;
}

pub fn complete_tints(
    mut commands: Commands,
    scene_spawner: Res<SceneSpawner>,
    mut meshes: ResMut<Assets<Mesh>>,
    effect_query: Query<(Entity, &SceneInstance, Option<&EffectFlags>)>,
    mesh_query: Query<&Handle<Mesh>>,
    mut finished: EventReader<TintCompletedEvent>,
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

                let Some(VertexAttributeValues::Float32x4(ref mut buf)) =
                    mesh.attribute_mut(Mesh::ATTRIBUTE_COLOR)
                else {
                    continue;
                };

                buf.fill(Color::WHITE.as_rgba_f32());
            }
        }

        if flags.intersects(EffectFlags::DESPAWN) {
            commands.entity(e_effect).remove::<TintEffect>();
        }
    }
}
