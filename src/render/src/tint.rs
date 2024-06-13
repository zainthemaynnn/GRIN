use bevy::{prelude::*, scene::SceneInstance};
use bevy_tweening::{component_animator_system, AnimationSystem, Lens};
use grin_util::color::ColorExt;

use crate::{
    sketched::{MaterialMutationResource, SketchMaterial},
    EffectFlags, TweenAppExt, TweenCompletedEvent,
};

pub struct TintPlugin;

impl Plugin for TintPlugin {
    fn build(&self, app: &mut App) {
        app.add_tween_completion_event::<TintCompletedEvent>()
            .add_systems(
                Update,
                (
                    (component_animator_system::<TintEffect>, set_tint_color)
                        .chain()
                        .in_set(AnimationSystem::AnimationUpdate),
                    complete_tints.after(AnimationSystem::AnimationUpdate),
                ),
            );
    }
}

/// Just an easy way to change the `Mesh::ATTRIBUTE_COLOR` uniformly.
///
/// Note: Alpha also applies.
#[derive(Component, Clone, Debug)]
pub struct TintEffect {
    /// Tint base color.
    pub base_color: Option<Color>,
    /// Tint emissive color.
    pub emissive: Option<Color>,
    /// Whether to hide the current texture.
    pub hide_texture: bool,
    /// Whether to enable material unlit.
    pub unlit: bool,
    /// Effect flags.
    pub flags: EffectFlags,
}

impl Default for TintEffect {
    fn default() -> Self {
        Self {
            base_color: None,
            emissive: None,
            hide_texture: false,
            unlit: false,
            flags: EffectFlags::default(),
        }
    }
}

#[derive(Component, Default)]
pub struct TintColorLens {
    pub start: Color,
    pub end: Color,
}

impl Lens<TintEffect> for TintColorLens {
    fn lerp(&mut self, target: &mut TintEffect, ratio: f32) {
        target.base_color = Some(self.start.lerp(&self.end, ratio));
    }
}

#[derive(Component, Default)]
pub struct TintEmissiveLens {
    pub start: Color,
    pub end: Color,
}

impl Lens<TintEffect> for TintEmissiveLens {
    fn lerp(&mut self, target: &mut TintEffect, ratio: f32) {
        target.emissive = Some(self.start.lerp(&self.end, ratio));
    }
}

pub fn set_tint_color(
    scene_spawner: Res<SceneSpawner>,
    mut materials: ResMut<Assets<SketchMaterial>>,
    mut material_mutation: ResMut<MaterialMutationResource>,
    effect_query: Query<(&SceneInstance, &TintEffect)>,
    mut material_query: Query<&mut Handle<SketchMaterial>>,
) {
    for (scene_id, effect) in effect_query.iter() {
        if !scene_spawner.instance_is_ready(**scene_id) {
            continue;
        }

        trace!(msg="Setting tint color", tint=?effect);

        for e_material in scene_spawner.iter_instance_entities(**scene_id) {
            let Ok(mut h_material) = material_query.get_mut(e_material) else {
                continue;
            };

            if let Some(h_mod_material) =
                material_mutation.modify(&mut materials, &h_material, |mat| {
                    if let Some(base_color) = effect.base_color {
                        mat.base.base_color = base_color;
                    }
                    if let Some(emissive) = effect.emissive {
                        mat.base.emissive = emissive;
                    }
                    if effect.hide_texture {
                        mat.extension.base_color_texture = None;
                    }
                    mat.base.unlit = effect.unlit;
                })
            {
                *h_material = h_mod_material;
            }
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
    mut material_mutation: ResMut<MaterialMutationResource>,
    effect_query: Query<(Entity, &SceneInstance, &TintEffect)>,
    mut material_query: Query<&mut Handle<SketchMaterial>>,
    mut finished: EventReader<TintCompletedEvent>,
) {
    for (e_effect, scene_id, TintEffect { flags, .. }) in
        finished.read().filter_map(|ev| effect_query.get(ev.0).ok())
    {
        trace!(msg="Tint completed", e_effect=?e_effect, flags=?flags);
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
            commands.entity(e_effect).remove::<TintEffect>();
        }
    }
}
