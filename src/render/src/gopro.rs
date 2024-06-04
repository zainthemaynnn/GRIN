use bevy::{
    asset::LoadState,
    prelude::*,
    render::{
        camera::RenderTarget,
        render_resource::{
            Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
        },
        view::RenderLayers,
    },
};

pub struct GoProPlugin;

impl Plugin for GoProPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, unload_gopros);
    }
}

#[derive(Component)]
pub struct GoPro;

pub struct GoProSettings {
    /// Parent entity of the camera.
    pub entity: Entity,
    /// Transform of the camera.
    pub transform: Transform,
    /// Dimensions of the render target.
    pub size: UVec2,
    /// `RenderLayers` of the camera.
    pub render_layers: RenderLayers,
}

pub fn create_image_target(images: &mut Assets<Image>, size: UVec2) -> Handle<Image> {
    let size = Extent3d {
        width: size.x,
        height: size.y,
        depth_or_array_layers: 1,
    };
    let mut target = Image {
        texture_descriptor: TextureDescriptor {
            label: None,
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Bgra8UnormSrgb,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        },
        ..Default::default()
    };
    target.resize(size);
    images.add(target)
}

pub fn create_gopro(
    target: Handle<Image>,
    transform: Transform,
    render_layers: RenderLayers,
) -> impl Bundle {
    (
        GoPro,
        Camera3dBundle {
            transform,
            camera: Camera {
                order: -1,
                // by looking at the internal camera system for like 2 seconds
                // it looks like this will just silently fail if the asset is dropped.
                // which is what I want.
                target: RenderTarget::Image(target),
                ..Default::default()
            },
            ..Default::default()
        },
        render_layers.clone(),
    )
}

/// Returns a strong handle to an image rendered to by a camera defined by `GoProSettings`.
///
/// The camera is removed when this handle is dropped.
pub fn add_gopro(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    settings: GoProSettings,
) -> Handle<Image> {
    let GoProSettings {
        entity,
        transform,
        size,
        render_layers,
    } = settings;

    let h_target = create_image_target(images, size);

    commands
        .spawn(create_gopro(
            h_target.clone_weak(),
            transform,
            render_layers,
        ))
        .set_parent(entity);

    h_target
}

/// Returns a strong handle to an image rendered to by a camera defined by `GoProSettings`.
///
/// The camera is removed when this handle is dropped.
pub fn add_gopro_world(world: &mut World, settings: GoProSettings) -> Handle<Image> {
    let GoProSettings {
        entity,
        transform,
        size,
        render_layers,
    } = settings;
    let mut images = world.resource_mut::<Assets<Image>>();

    let h_target = create_image_target(&mut images, size);

    world
        .spawn(create_gopro(
            h_target.clone_weak(),
            transform,
            render_layers,
        ))
        .set_parent(entity);

    h_target
}

/// Remove `GoPro`s with dropped target handles.
pub fn unload_gopros(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    query: Query<(Entity, &Camera), With<GoPro>>,
) {
    for (entity, camera) in query.iter() {
        if let RenderTarget::Image(target) = &camera.target {
            // TODO: changed in bevy 0.12. is this supposed to be `NotLoaded` or `None`?
            if let Some(LoadState::NotLoaded) = asset_server.get_load_state(target) {
                commands.entity(entity).despawn();
            }
        }
    }
}
