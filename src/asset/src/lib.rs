pub mod texture;

use bevy::{
    asset::LoadState,
    gltf::Gltf,
    prelude::*,
    reflect::TypePath,
    render::render_resource::{Face, TextureViewDescriptor, TextureViewDimension},
    utils::{thiserror::Error, HashMap},
};
use bevy_asset_loader::prelude::*;
use bevy_common_assets::ron::RonAssetPlugin;
use grin_render::sketched::{SketchMaterial, SketchMaterialInfo, SketchUiImage};
use itertools::Itertools;
use iyes_progress::prelude::*;
use serde::Deserialize;

pub const GLTF_PRELOAD_FOLDER: &str = "gltf/";

pub struct DynamicAssetPlugin;

impl Plugin for DynamicAssetPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AssetLoadState>()
            .init_resource::<FallbackImage>()
            .init_resource::<GltfPreload>()
            .add_plugins((
                ProgressPlugin::new(AssetLoadState::Loading).continue_to(AssetLoadState::Success),
                RonAssetPlugin::<CustomDynamicAssetCollection>::new(&["assets.ron"]),
            ))
            .add_loading_state(
                LoadingState::new(AssetLoadState::Loading)
                    .continue_to_state(AssetLoadState::Success)
                    .on_failure_continue_to_state(AssetLoadState::Failure)
                    .register_dynamic_asset_collection::<CustomDynamicAssetCollection>()
                    .with_dynamic_assets_file::<CustomDynamicAssetCollection>("test.assets.ron"),
            )
            .add_systems(OnEnter(AssetLoadState::PreLoading), begin_gltf_preload)
            .add_systems(
                Update,
                end_gltf_preload.run_if(in_state(AssetLoadState::PreLoading)),
            )
            .add_systems(
                Update,
                log_load_progress.run_if(in_state(AssetLoadState::Loading)),
            );
    }
}

pub fn log_load_progress(progress: Option<Res<ProgressCounter>>, mut last_done: Local<u32>) {
    if let Some(progress) = progress.map(|counter| counter.progress()) {
        if progress.done > *last_done {
            *last_done = progress.done;
            debug!("{:?}", progress);
        }
    }
}

/// All GlTF files are loaded EARLY, so that the asset files can specify sub-assets directly
/// and put them into asset collections.
///
/// This step will no longer be needed if bevy ever supports direct loading of named GLTF sub-assets.
#[derive(Resource, Debug, Default)]
pub struct GltfPreload(pub HashMap<String, Handle<Gltf>>);

pub fn begin_gltf_preload(asset_server: Res<AssetServer>, mut gltf_preload: ResMut<GltfPreload>) {
    // load everything in the `GLTF_PRELOAD_FOLDER` folder
    for file in std::fs::read_dir("assets/".to_string() + GLTF_PRELOAD_FOLDER).unwrap() {
        if let Ok(file) = file {
            let fname = file.file_name().into_string().unwrap();
            let fpath = GLTF_PRELOAD_FOLDER.to_string() + fname.as_str();
            gltf_preload.0.insert(fname, asset_server.load(fpath));
        }
    }
}

pub fn end_gltf_preload(
    asset_server: Res<AssetServer>,
    gltf_preload: Res<GltfPreload>,
    mut next_state: ResMut<NextState<AssetLoadState>>,
) {
    // check for preload completion
    if gltf_preload
        .0
        .values()
        .all(|h| matches!(asset_server.get_load_state(h), Some(LoadState::Loaded)))
    {
        next_state.set(AssetLoadState::Loading);
    }
}

#[derive(Resource)]
pub struct FallbackImage {
    pub texture: Handle<Image>,
}

impl FromWorld for FallbackImage {
    fn from_world(world: &mut World) -> Self {
        let mut textures = world.resource_mut::<Assets<Image>>();
        let mut tex = Image::default();
        tex.reinterpret_stacked_2d_as_array(1);
        tex.texture_view_descriptor = Some(TextureViewDescriptor {
            label: Some("D2Array Texture View"),
            dimension: Some(TextureViewDimension::D2Array),
            format: Some(tex.texture_descriptor.format),
            array_layer_count: Some(1),
            ..Default::default()
        });
        Self {
            texture: textures.add(tex),
        }
    }
}

#[derive(States, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum AssetLoadState {
    Loading,
    #[default]
    PreLoading,
    Success,
    Failure,
}

/// Deserializable `Face`.
#[derive(Debug, Deserialize, Copy, Clone)]
pub enum AssetFace {
    Front,
    Back,
    NoCull,
}

impl From<AssetFace> for Option<Face> {
    fn from(value: AssetFace) -> Self {
        match value {
            AssetFace::Front => Some(Face::Front),
            AssetFace::Back => Some(Face::Back),
            AssetFace::NoCull => None,
        }
    }
}

/// Deserializable `AlphaMode`.
#[derive(Debug, Deserialize, Copy, Clone)]
pub enum AssetAlphaMode {
    Opaque,
    Mask(f32),
    Blend,
    Premultiplied,
    Add,
    Multiply,
}

impl From<AssetAlphaMode> for AlphaMode {
    fn from(value: AssetAlphaMode) -> Self {
        match value {
            AssetAlphaMode::Opaque => AlphaMode::Opaque,
            AssetAlphaMode::Mask(t) => AlphaMode::Mask(t),
            AssetAlphaMode::Blend => AlphaMode::Blend,
            AssetAlphaMode::Premultiplied => AlphaMode::Premultiplied,
            AssetAlphaMode::Add => AlphaMode::Add,
            AssetAlphaMode::Multiply => AlphaMode::Multiply,
        }
    }
}

#[derive(Debug, Deserialize, Copy, Clone)]
pub enum GltfSubAssetType {
    Scene,
    Animation,
    Mesh,
    Material,
    Node,
}

#[derive(Error, Debug)]
pub enum GltfSubAssetLoadError {
    SourceNotFound(String),
    ItemNotFound(String),
    GltfNotFound,
}

impl std::fmt::Display for GltfSubAssetLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SourceNotFound(s) => {
                f.write_fmt(format_args!("No GLTF source with name `{}`", s))
            }
            Self::ItemNotFound(s) => f.write_fmt(format_args!("No GLTF item with name `{}`", s)),
            Self::GltfNotFound => {
                f.write_str("[Internal error] `Assets<Gltf>` did not have the specified handle.")
            }
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub enum CustomDynamicAsset {
    File {
        path: String,
    },
    GltfSubAsset {
        source: String,
        item: String,
        ty: GltfSubAssetType,
    },
    UVSphereMesh {
        radius: f32,
    },
    // NOTE: will fill in the rest if I ever need to... feeling lazy
    SketchMaterial {
        base_color: Option<[f32; 4]>,
        base_color_texture: Option<String>,
        perceptual_roughness: Option<f32>,
        reflectance: Option<f32>,
        emissive: Option<[f32; 4]>,
        double_sided: Option<bool>,
        cull_mode: Option<AssetFace>,
        layers: Option<u32>,
        alpha_mode: Option<AssetAlphaMode>,
        unlit: Option<bool>,
    },
    SketchUiImage {
        images: Vec<String>,
    },
}

impl DynamicAsset for CustomDynamicAsset {
    fn load(&self, asset_server: &AssetServer) -> Vec<UntypedHandle> {
        trace!("{:?}", self);
        match self {
            Self::File { path } => vec![asset_server.load_untyped(path).untyped()],
            Self::UVSphereMesh { .. } | Self::GltfSubAsset { .. } => vec![],
            Self::SketchMaterial {
                base_color_texture, ..
            } => base_color_texture
                .as_ref()
                .map_or_else(Default::default, |path| {
                    vec![asset_server.load_untyped(path).untyped()]
                }),
            Self::SketchUiImage { images } => images
                .iter()
                .map(|path| asset_server.load_untyped(path).untyped())
                .collect_vec(),
        }
    }

    fn build(&self, world: &mut World) -> Result<DynamicAssetType, anyhow::Error> {
        let world_cell = world.cell();
        let asset_server = world_cell.resource::<AssetServer>();

        match self {
            Self::File { path } => Ok(DynamicAssetType::Single(
                asset_server.get_handle_untyped(path).unwrap(),
            )),
            Self::GltfSubAsset { source, item, ty } => {
                let gltf_assets = world_cell.resource::<Assets<Gltf>>();
                let gltf_preload = world_cell.resource::<GltfPreload>();
                // get the corresponding gltf defined in `source`
                let gltf_handle = gltf_preload
                    .0
                    .get(source)
                    .ok_or(GltfSubAssetLoadError::SourceNotFound(source.clone()))?;
                let gltf = gltf_assets.get(gltf_handle).unwrap();
                // AVERAGE RUST PROGRAM
                Ok(DynamicAssetType::Single(
                    match ty {
                        GltfSubAssetType::Scene => {
                            gltf.named_scenes.get(item).map(|h| h.clone().untyped())
                        }
                        GltfSubAssetType::Animation => {
                            gltf.named_animations.get(item).map(|h| h.clone().untyped())
                        }
                        GltfSubAssetType::Mesh => {
                            gltf.named_meshes.get(item).map(|h| h.clone().untyped())
                        }
                        GltfSubAssetType::Material => {
                            gltf.named_materials.get(item).map(|h| h.clone().untyped())
                        }
                        GltfSubAssetType::Node => {
                            gltf.named_nodes.get(item).map(|h| h.clone().untyped())
                        }
                    }
                    .ok_or(GltfSubAssetLoadError::ItemNotFound(item.clone()))?,
                ))
            }
            Self::UVSphereMesh { radius } => {
                let mut meshes = world_cell.resource_mut::<Assets<Mesh>>();
                Ok(DynamicAssetType::Single(
                    meshes
                        .add(Mesh::from(Sphere {
                            radius: *radius,
                            ..Default::default()
                        }))
                        .untyped(),
                ))
            }
            Self::SketchMaterial {
                base_color,
                base_color_texture,
                perceptual_roughness,
                reflectance,
                emissive,
                double_sided,
                cull_mode,
                layers,
                alpha_mode,
                unlit,
            } => {
                // the textureview dimension MUST be D2Array
                // this is a problem because singly layered images
                // are automatically interpreted as D2
                // which leads to mismatches
                let mut materials = world_cell.resource_mut::<Assets<SketchMaterial>>();

                let base_color_texture = match base_color_texture {
                    Some(tex_path) => {
                        let mut textures = world_cell.resource_mut::<Assets<Image>>();

                        let tex_handle = asset_server.load(tex_path);
                        let tex = textures
                            .get_mut(&tex_handle)
                            .expect("Failed to get StandardMaterial texture.");

                        if tex.texture_descriptor.size.depth_or_array_layers == 1 {
                            let layers = layers.unwrap_or(1);
                            tex.reinterpret_stacked_2d_as_array(layers);

                            // force dimension to D2Array
                            tex.texture_view_descriptor = Some(TextureViewDescriptor {
                                label: Some("D2Array Texture View"),
                                dimension: Some(TextureViewDimension::D2Array),
                                format: Some(tex.texture_descriptor.format),
                                array_layer_count: Some(layers),
                                ..Default::default()
                            });
                        }

                        tex_handle
                    }
                    // a custom FallbackImage is used
                    // because bevy's is always D2 even if the binding isn't
                    // (this will be fixed in 0.11)
                    // update may 2024: ha-ha. let's just leave it here for fun.
                    None => world_cell.resource::<FallbackImage>().texture.clone(),
                };

                let mat_default = StandardMaterial::default();

                Ok(DynamicAssetType::Single(
                    materials
                        .add(SketchMaterial {
                            base: StandardMaterial {
                                base_color: base_color
                                    .map_or(mat_default.base_color, Color::rgba_from_array),
                                base_color_texture: None,
                                perceptual_roughness: perceptual_roughness
                                    .unwrap_or(mat_default.perceptual_roughness),
                                reflectance: reflectance.unwrap_or(mat_default.reflectance),
                                emissive: emissive
                                    .map_or(mat_default.emissive, Color::rgba_from_array),
                                double_sided: double_sided.unwrap_or(mat_default.double_sided),
                                cull_mode: cull_mode
                                    .map_or(mat_default.cull_mode, Option::<Face>::from),
                                alpha_mode: alpha_mode
                                    .map_or(mat_default.alpha_mode, AlphaMode::from),
                                unlit: unlit.unwrap_or(mat_default.unlit),
                                ..Default::default()
                            },
                            extension: SketchMaterialInfo {
                                sketch_enabled: true,
                                fill_enabled: true,
                                layer: 0,
                                base_color_texture: Some(base_color_texture),
                            },
                        })
                        .untyped(),
                ))
            }
            Self::SketchUiImage { images } => {
                let mut assets = world_cell.resource_mut::<Assets<SketchUiImage>>();
                Ok(DynamicAssetType::Single(
                    assets
                        .add(SketchUiImage {
                            images: images
                                .iter()
                                .map(|path| asset_server.load(path))
                                .collect_vec(),
                        })
                        .untyped(),
                ))
            }
        }
    }
}

#[derive(Asset, Deserialize, TypePath)]
pub struct CustomDynamicAssetCollection(pub HashMap<String, CustomDynamicAsset>);

impl DynamicAssetCollection for CustomDynamicAssetCollection {
    fn register(&self, dynamic_assets: &mut DynamicAssets) {
        for (key, asset) in self.0.iter() {
            dynamic_assets.register_asset(key, Box::new(asset.clone()));
        }
    }
}
