//! Creates the roughly-drawn effect of the world
//! (i.e. animated outlines/textures).

use bevy::{
    pbr::{
        ExtendedMaterial, MaterialExtension,
    },
    prelude::*,
    reflect::{TypePath},
    render::render_resource::{AsBindGroup, ShaderRef},
    scene::SceneInstance,
    utils::HashMap,
};
pub(crate) use bevy_mod_outline::*;

pub type SketchMaterial = ExtendedMaterial<StandardMaterial, SketchMaterialInfo>;

pub struct SketchEffectPlugin {
    pub outline: GlobalMeshOutline,
    pub autofill_sketch_effect: bool,
}

impl Plugin for SketchEffectPlugin {
    fn build(&self, app: &mut App) {
        let autofill_enabled = self.autofill_sketch_effect;
        app.add_plugins((
            OutlinePlugin,
            AutoGenerateOutlineNormalsPlugin,
            MaterialPlugin::<SketchMaterial>::default(),
        ))
        .insert_resource(self.outline.clone())
        .init_resource::<StandardToSketchMaterialInfoResource>()
        .init_asset::<SketchUiImage>()
        .add_systems(
            PreUpdate,
            (
                autofill_sketch_effect.run_if(move || autofill_enabled == true),
                purge_sketch_effects,
                
            ),
        )
        .add_systems(
            Update,
            (
                // TODO: fix
                //scale_outlines,
                init_sketched_ui_images,
                animate_sketched_materials,
                animate_sketched_outlines,
                animate_sketched_ui_images.after(init_sketched_ui_images),
                customize_scene_materials,
            ),
        );
    }
}

#[derive(Resource, Clone, Default)]
pub struct GlobalMeshOutline {
    pub standard: OutlineBundle,
    pub mini: OutlineBundle,
}

#[derive(Component)]
pub struct SketchCamera;

#[derive(Component, Clone)]
pub struct SketchAnimation {
    pub rate: f32,
}

impl From<f32> for SketchAnimation {
    fn from(value: f32) -> Self {
        Self { rate: value }
    }
}

impl Default for SketchAnimation {
    fn default() -> Self {
        Self::from(1.0)
    }
}

/// Prevents outlines from being generated for this mesh.
#[derive(Component, Clone, Default)]
pub struct NoOutline;

/// Whether to scale outline width based on Z distance.
#[derive(Component, Clone, Default)]
pub enum OutlineScaleMode {
    /// Scale where `0` is the apparent pixel width at a distance of `1.0` from the camera.
    Scale(f32),
    /// Outline has a consistent width.
    #[default]
    NoScale,
}

pub fn animate_sketched_materials(
    textures: Res<Assets<Image>>,
    mut materials: ResMut<Assets<SketchMaterial>>,
    time: Res<Time>,
    material_handle_query: Query<(&Handle<SketchMaterial>, &SketchAnimation)>,
) {
    for (material_handle, sketch) in material_handle_query.iter() {
        if let Some(material) = materials.get_mut(material_handle) {
            if let Some(max_layers) = material.extension.base_color_texture_layers(&textures) {
                material.extension.layer =
                    (time.elapsed_seconds_wrapped() / sketch.rate) as u32 % max_layers;
            }
        }
    }
}

// just gonna hardcode these xddd
// the fact that this measly thing is its own plugin is service enough
pub fn animate_sketched_outlines(
    time: Res<Time>,
    mut outline_query: Query<(&mut OutlineDeform, &SketchAnimation)>,
) {
    for (mut deform, sketch) in outline_query.iter_mut() {
        let even = (time.elapsed_seconds_wrapped() / sketch.rate) as u32 % 2 == 0;
        deform.seed = if even { 0.0 } else { 1000.0 };
    }
}

pub fn scale_outlines(
    camera_query: Query<&GlobalTransform, With<Camera>>,
    mut outline_query: Query<(&GlobalTransform, &mut OutlineVolume, &OutlineScaleMode)>,
) {
    for g_cam_transform in camera_query.iter() {
        let inv_proj = g_cam_transform.compute_matrix().inverse();

        for (g_outline_transform, mut outline, scale_mode) in outline_query.iter_mut() {
            if let OutlineScaleMode::Scale(scale) = scale_mode {
                let diff = Transform::from_matrix(g_outline_transform.compute_matrix() * inv_proj);
                if diff.translation.z < 0.0 {
                    outline.width = scale / -diff.translation.z;
                }
            }
        }
    }
}

pub fn init_sketched_ui_images(
    mut commands: Commands,
    query: Query<
        Entity,
        (
            With<Handle<SketchUiImage>>,
            Or<(Without<UiImage>, Without<BackgroundColor>)>,
        ),
    >,
) {
    for e_image in query.iter() {
        commands
            .entity(e_image)
            .insert((UiImage::default(), BackgroundColor::default()));
    }
}

pub fn animate_sketched_ui_images(
    time: Res<Time>,
    sketch_images: Res<Assets<SketchUiImage>>,
    mut query: Query<(
        &Handle<SketchUiImage>,
        &mut UiImage,
        &SketchAnimation,
        &mut BackgroundColor,
    )>,
) {
    for (sketch_image_handle, mut ui_image, sketch, mut background_color) in query.iter_mut() {
        let SketchUiImage { images } = sketch_images.get(sketch_image_handle).unwrap();
        let idx = (time.elapsed_seconds_wrapped() / sketch.rate) as usize % images.len();
        ui_image.texture = images[idx].clone();
        *background_color = BackgroundColor::default();
    }
}

pub fn autofill_sketch_effect(
    mut commands: Commands,
    no_outline_query: Query<
        Entity,
        (
            With<Handle<Mesh>>,
            Without<OutlineVolume>,
            Without<NoOutline>,
        ),
    >,
    //no_outline_depth_query: Query<Entity, (With<OutlineVolume>, Without<SetOutlineDepth>)>,
    no_animation_query: Query<
        Entity,
        (
            Or<(With<Handle<Mesh>>, With<Handle<SketchUiImage>>)>,
            Without<SketchAnimation>,
        ),
    >,
    outline: Res<GlobalMeshOutline>,
) {
    for entity in no_outline_query.iter() {
        commands
            .get_or_spawn(entity)
            .insert(outline.standard.clone());
    }
    /*for entity in no_outline_depth_query.iter() {
        commands.get_or_spawn(entity).insert(SetOutlineDepth::Real);
    }*/
    for entity in no_animation_query.iter() {
        commands
            .get_or_spawn(entity)
            .insert(SketchAnimation::default());
    }
}

pub fn purge_sketch_effects(
    mut commands: Commands,
    query: Query<Entity, (With<OutlineVolume>, With<NoOutline>)>,
) {
    for e_outline in query.iter() {
        commands.entity(e_outline).remove::<OutlineVolume>();
    }
}

/// Maps material names to sketch-materials. Then, when importing from GLTF, a corresponding
/// sketch-material can be swapped in (since GLTF doesn't support my custom texture).
///
/// Note: This gets populated in an external crate, `grin_asset`.
#[derive(Resource, Default)]
pub struct StandardToSketchMaterialInfoResource(
    pub HashMap<AssetId<StandardMaterial>, Handle<SketchMaterial>>,
);

#[derive(Component)]
pub struct CustomizeMaterial;

// https://github.com/bevyengine/bevy/discussions/8533
pub fn customize_scene_materials(
    mut commands: Commands,
    mut pbr_materials: ResMut<Assets<StandardMaterial>>,
    mut sketch_materials: ResMut<Assets<SketchMaterial>>,
    mut std_to_sketch_materials: ResMut<StandardToSketchMaterialInfoResource>,
    scene_manager: Res<SceneSpawner>,
    unloaded_instances: Query<(Entity, &SceneInstance), With<CustomizeMaterial>>,
    handles: Query<(Entity, &Handle<StandardMaterial>)>,
) {
    for (entity, instance) in unloaded_instances.iter() {
        if scene_manager.instance_is_ready(**instance) {
            commands.entity(entity).remove::<CustomizeMaterial>();
        }
        // Iterate over all entities in scene (once it's loaded)
        let handles = handles.iter_many(scene_manager.iter_instance_entities(**instance));
        for (entity, h_std) in handles {
            let h_sketched = match std_to_sketch_materials.0.get(&h_std.id()) {
                Some(h) => h.clone(),
                None => {
                    // there's no stored animated texture, so this is probably just a regular
                    // texture. still, it should be converted into a sketch material with the
                    // feature disabled.
                    let h_sketched = sketch_materials.add(SketchMaterial {
                        base: pbr_materials.remove(h_std).unwrap(),
                        extension: SketchMaterialInfo::default(),
                    });
                    std_to_sketch_materials
                        .0
                        .insert(h_std.id(), h_sketched.clone())
                        .unwrap();
                    h_sketched
                }
            };

            commands
                .entity(entity)
                .insert(h_sketched)
                .remove::<Handle<StandardMaterial>>();
        }
    }
}

/// `SceneBundle`, with processed `SketchMaterialInfo`.
pub struct SketchedSceneBundle {
    pub scene: Handle<Scene>,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub visibility: Visibility,
    pub inherited_visibility: InheritedVisibility,
    pub view_visibility: ViewVisibility,
}

#[derive(Asset, TypePath)]
pub struct SketchUiImage {
    pub images: Vec<Handle<Image>>,
}

// I am eternally, entirely grateful for whoever introduced StandardMaterial extensions
// in bevy 0.12. well done, my friend.
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone, Default)]
pub struct SketchMaterialInfo {
    /// Sketch enabled?
    //
    // TODO: this should really be a flag. unfortunately that is easier said than done,
    // and is not included in bevy... yet?
    #[uniform(100)]
    pub enabled: u32,
    /// The layer of `base_color_texture` to use.
    #[uniform(101)]
    pub layer: u32,
    #[texture(102, dimension = "2d_array")]
    #[sampler(103)]
    pub base_color_texture: Option<Handle<Image>>,
}

impl SketchMaterialInfo {
    pub fn base_color_texture_layers(&self, textures: &Assets<Image>) -> Option<u32> {
        let tex_handle = self.base_color_texture.clone()?;
        Some(
            textures
                .get(&tex_handle)?
                .texture_descriptor
                .array_layer_count(),
        )
    }
}

impl MaterialExtension for SketchMaterialInfo {
    fn fragment_shader() -> ShaderRef {
        "shaders/sketch_material.wgsl".into()
    }
}
