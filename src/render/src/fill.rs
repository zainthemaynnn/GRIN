use bevy::{prelude::*, render::mesh::VertexAttributeValues, scene::SceneInstance};
use grin_time::scaling::TimeScale;
use grin_util::spatial::SceneAabb;

use crate::sketched::ATTRIBUTE_Y_CUTOFF;

pub struct FillPlugin;

impl Plugin for FillPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(First, set_y_cutoff_attributes).add_systems(
            PostUpdate,
            (update_fill_transitions, set_fill_cutoffs).chain(),
        );
    }
}

/// It's like a... glass-filling-with-liquid kinda thing.
///
/// This should be placed alongside a `SceneAabb` to work.
#[derive(Component, Copy, Clone, Debug)]
pub struct FillEffect {
    /// Proportion of the fill effect that has been completed.
    pub t: f32,
    /// Rate of change for `t` per second.
    pub rate: f32,
}

impl Default for FillEffect {
    fn default() -> Self {
        Self { t: 0.0, rate: 1.0 }
    }
}

pub fn update_fill_transitions(
    time: Res<Time>,
    mut effect_query: Query<(&mut FillEffect, &TimeScale)>,
) {
    for (mut effect, time_scale) in effect_query.iter_mut() {
        effect.t = (effect.t + time.delta_seconds() * effect.rate * f32::from(time_scale)).min(1.0);
    }
}

pub fn set_y_cutoff_attributes(
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

        //warn!(attrss=?mesh.attributes().map(|(id, _)| id).collect::<Vec<_>>());

        trace!(
            msg="Set y cutoff to `f32::MAX`.",
            asset_id=?id,
        );
    }
}

pub fn set_fill_cutoffs(
    scene_spawner: Res<SceneSpawner>,
    mut meshes: ResMut<Assets<Mesh>>,
    effect_query: Query<(&SceneInstance, &SceneAabb, &FillEffect)>,
    mesh_query: Query<&Handle<Mesh>>,
) {
    for (scene_id, aabb, effect) in effect_query.iter() {
        if !scene_spawner.instance_is_ready(**scene_id) {
            continue;
        }

        let y_cutoff = aabb.min.y.lerp(aabb.max.y, effect.t);
        trace!(
            msg="Setting y cutoff.",
            y_cutoff=y_cutoff,
        );

        for e_mesh in scene_spawner.iter_instance_entities(**scene_id) {
            let Ok(h_mesh) = mesh_query.get(e_mesh) else {
                continue;
            };

            let Some(mesh) = meshes.get_mut(h_mesh) else {
                warn!(
                    msg="Mesh not found.",
                    asset_id=?h_mesh.id(),
                );
                continue;
            };

            //warn!(attrs=?mesh.attributes().map(|(id, _)| id).collect::<Vec<_>>());

            let write_result = match mesh.attribute_mut(ATTRIBUTE_Y_CUTOFF) {
                Some(VertexAttributeValues::Float32(ref mut buf)) => {
                    buf.fill(y_cutoff);
                    Ok(())
                }
                Some(values) => Err(Some(values)),
                None => Err(None),
            };

            if let Err(v) = write_result {
                error!(
                    msg="Vertex attribute type mismatch.",
                    attr=ATTRIBUTE_Y_CUTOFF.name,
                    expected=?VertexAttributeValues::Float32(Vec::default()),
                    found=?v,
                    asset_id=?h_mesh.id(),
                );
            }
        }
    }
}
