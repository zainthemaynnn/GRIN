//! Creates the roughly-drawn effect of the world
//! (i.e. animated outlines/textures).

use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
    prelude::*,
    render::{
        mesh::{MeshVertexAttribute, MeshVertexBufferLayout},
        render_resource::{
            AsBindGroup, RenderPipelineDescriptor, ShaderRef, SpecializedMeshPipelineError,
            VertexFormat,
        },
    },
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
                jank_i_hope_nobody_reads_this_std_material_purge,
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

// for some reason a few standard-materials are "slipping through the cracks," and I'm too lazy to fix
pub fn jank_i_hope_nobody_reads_this_std_material_purge(
    mut commands: Commands,
    material_query: Query<Entity, (With<Handle<SketchMaterial>>, With<Handle<StandardMaterial>>)>,
) {
    for e_material in material_query.iter() {
        trace!(
            msg=":thinking emoji:",
            e_material=?e_material,
        );
        commands
            .entity(e_material)
            .remove::<Handle<StandardMaterial>>();
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

/// Maps standard-materials to sketch-materials. Then, when importing from GLTF, a corresponding
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
                    let base = pbr_materials.remove(h_std).unwrap();
                    let extension = SketchMaterialInfo {
                        sketch_enabled: false,
                        ..Default::default()
                    };
                    let h_sketched = sketch_materials.add(SketchMaterial { base, extension });
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
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
#[bind_group_data(SketchMaterialKey)]
pub struct SketchMaterialInfo {
    /// Sketch effect enabled?
    pub sketch_enabled: bool,
    /// Fill effect supported?
    pub fill_enabled: bool,
    /// The layer of `base_color_texture` to use.
    #[uniform(101)]
    pub layer: u32,
    #[texture(102, dimension = "2d_array")]
    #[sampler(103)]
    pub base_color_texture: Option<Handle<Image>>,
}

impl Default for SketchMaterialInfo {
    fn default() -> Self {
        Self {
            sketch_enabled: true,
            fill_enabled: true,
            layer: 0,
            base_color_texture: None,
        }
    }
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

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
pub struct SketchMaterialKey {
    pub sketch_enabled: bool,
    pub y_cutoff_enabled: bool,
}

impl From<&SketchMaterialInfo> for SketchMaterialKey {
    fn from(value: &SketchMaterialInfo) -> Self {
        Self {
            sketch_enabled: value.sketch_enabled,
            y_cutoff_enabled: value.fill_enabled,
        }
    }
}

// this vertex attribute is duplicated across the entire mesh.
//
// according to my research, changing the uniform for every draw call is slow,
// and obviously stops batching, so I think I'll avoid using a uniform for this case.
//
// I cannot use an instance_index buffer (like bevy's mesh buffer) because the same
// instances are not guaranteed to have the same y-cutoff.
//
// also, the meshes in grin are low-poly and this attribute is only an f32.
//
// SO, IN SUMMARY, as counterintuitive as it is, I don't think it's a serious problem.
// also read: https://stackoverflow.com/a/30339855
pub const ATTRIBUTE_Y_CUTOFF: MeshVertexAttribute =
    MeshVertexAttribute::new("y_cutoff", 988540917, VertexFormat::Float32);

impl MaterialExtension for SketchMaterialInfo {
    fn vertex_shader() -> ShaderRef {
        "shaders/vertex.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/sketch_material.wgsl".into()
    }

    fn deferred_fragment_shader() -> ShaderRef {
        "shaders/sketch_material.wgsl".into()
    }

    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayout,
        key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(ref mut fragment) = &mut descriptor.fragment {
            if key.bind_group_data.sketch_enabled {
                fragment.shader_defs.push("SKETCHED".into());
            }

            // TODO: this needs a per-mesh flag, see issue #7.
            if key.bind_group_data.y_cutoff_enabled {
                let attrs = layout.get_layout(&[ATTRIBUTE_Y_CUTOFF.at_shader_location(9)])?;
                // uh... is this allowed?
                descriptor.vertex.buffers[0]
                    .attributes
                    .push(attrs.attributes[0]);
                descriptor.vertex.shader_defs.push("FILL".into());
                fragment.shader_defs.push("FILL".into());
            }
        }

        debug!(
            msg="New `SketchMaterial` pipeline.",
            vs_defs=?descriptor.vertex.shader_defs,
            fs_defs=?descriptor.fragment.as_ref().map(|f| &f.shader_defs),
        );

        Ok(())
    }
}

/// Maps materials under an effect to their standard non-effected versions.
/// Materials under an effect are drawn independently, but non-effected versions are not.
/// This resource helps eliminate some of the performance loss by still allowing batching of non-effected meshes.
///
/// This resource does NOT help reuse materials with the same effects applied.
/// You will need to manually clone the handles provided by this resource if you want to reuse them.
///
/// [Also see this message.](https://discord.com/channels/691052431525675048/749332104487108618/1219354680732291227)
#[derive(Resource, Default)]
pub struct MaterialMutationResource {
    map: HashMap<AssetId<SketchMaterial>, Handle<SketchMaterial>>,
}

#[derive(Copy, Clone, Debug)]
pub enum MaterialMutationError {
    EntryNotFound,
}

impl MaterialMutationResource {
    /// Modify this material, caching a new base material if necessary and returning the modified handle.
    ///
    /// If the provided material is already modded, modifies the existing material like normal.
    pub fn modify(
        &mut self,
        materials: &mut Assets<SketchMaterial>,
        material_handle: &Handle<SketchMaterial>,
        mut edit_fn: impl FnMut(&mut SketchMaterial) -> (),
    ) -> Option<Handle<SketchMaterial>> {
        match self.map.contains_key(&material_handle.id()) {
            true => {
                // nothing extra needs to be done since the base is already cached
                // and this is just another modification.
                // we can just edit the referenced material like normal.
                let mut base_material = materials.get_mut(material_handle).unwrap();
                edit_fn(&mut base_material);
                None
            }
            false => {
                // get base material
                let base_material = materials.get(material_handle).unwrap();
                // create the new modded material
                let mut mod_material = base_material.clone();
                edit_fn(&mut mod_material);
                let mod_material_handle = materials.add(mod_material);
                // link the modded material to the base
                self.map
                    .insert(mod_material_handle.id(), material_handle.clone());
                // return new handle
                Some(mod_material_handle)
            }
        }
    }

    /// Returns the corresponding base material, and removes this entry from the map.
    pub fn get_base(
        &mut self,
        material_id: &AssetId<SketchMaterial>,
    ) -> Result<&Handle<SketchMaterial>, MaterialMutationError> {
        self.map
            .get(material_id)
            .ok_or(MaterialMutationError::EntryNotFound)
    }

    /// Returns the corresponding base material, and removes this entry from the map.
    pub fn pop_base(
        &mut self,
        material_id: &AssetId<SketchMaterial>,
    ) -> Result<Handle<SketchMaterial>, MaterialMutationError> {
        self.map
            .remove(material_id)
            .ok_or(MaterialMutationError::EntryNotFound)
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Removes this modded material from the resource.
    fn drop(&mut self, material_id: &AssetId<SketchMaterial>) {
        self.map.remove(material_id);
    }
}

pub fn clear_unloaded_modded_materials(
    mut material_mutation: ResMut<MaterialMutationResource>,
    mut asset_events: EventReader<AssetEvent<SketchMaterial>>,
) {
    for ev in asset_events.read() {
        if let AssetEvent::Removed { id } | AssetEvent::Unused { id } = ev {
            material_mutation.drop(id);
        };
    }
}

#[cfg(test)]
mod material_tests {
    use super::*;

    #[test]
    fn add_modded_materials() {
        let mut app = App::new();
        app.add_plugins(AssetPlugin::default())
            .init_asset::<SketchMaterial>()
            .init_resource::<MaterialMutationResource>();

        let mut world_cell = app.world.cell();
        let mut materials = world_cell.resource_mut::<Assets<SketchMaterial>>();
        let h_base_material = materials.add(SketchMaterial {
            base: StandardMaterial::from(Color::WHITE),
            extension: SketchMaterialInfo::default(),
        });

        let mut material_mutation = world_cell.resource_mut::<MaterialMutationResource>();
        let h_mod_material = material_mutation
            .modify(&mut materials, &h_base_material, |mat| {
                mat.base.base_color = Color::BLACK
            })
            .expect("`MaterialMutationResource::modify` did not provide a handle.");

        let entry = material_mutation
            .get_base(&h_mod_material.id())
            .expect("`MaterialMutationResource::get_base` did not provide a handle.");
        assert_eq!(entry, &h_base_material);

        let entry = material_mutation
            .pop_base(&h_mod_material.id())
            .expect("`MaterialMutationResource::pop_base` did not provide a handle.");
        assert_eq!(entry, h_base_material);
        assert!(material_mutation.get_base(&h_mod_material.id()).is_err());

        assert_eq!(
            materials.get(&h_base_material).unwrap().base.base_color,
            Color::WHITE,
        );
        assert_eq!(
            materials.get(&h_mod_material).unwrap().base.base_color,
            Color::BLACK,
        );
    }

    #[test]
    fn drop_modded_materials() {
        let mut app = App::new();
        app.add_plugins(AssetPlugin::default())
            .init_asset::<SketchMaterial>()
            .init_resource::<MaterialMutationResource>()
            .add_systems(Update, clear_unloaded_modded_materials);

        let h_mod_material = {
            let mut world_cell = app.world.cell();
            let mut materials = world_cell.resource_mut::<Assets<SketchMaterial>>();
            let h_base_material = materials.add(SketchMaterial {
                base: StandardMaterial::from(Color::WHITE),
                extension: SketchMaterialInfo::default(),
            });

            let mut material_mutation = world_cell.resource_mut::<MaterialMutationResource>();
            material_mutation
                .modify(&mut materials, &h_base_material, |mat| {
                    mat.base.base_color = Color::BLACK
                })
                .unwrap()
        };

        drop(h_mod_material);

        // for some reason a single update doesn't drop, but three does? ¯\_(ツ)_/¯
        app.update();
        app.update();
        app.update();

        let mut material_mutation = app.world.resource_mut::<MaterialMutationResource>();
        assert!(material_mutation.is_empty());
    }
}
