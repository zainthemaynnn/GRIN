mod ai;
mod asset;
mod character;
mod collisions;
mod damage;
mod dialogue;
mod humanoid;
mod item;
mod render;
mod sound;
mod time;
mod util;

use std::{env, io};

use ai::{dummy::Dummy, AIPlugins};
use asset::FallbackImage;
use bevy::{
    diagnostic::LogDiagnosticsPlugin,
    log::{Level, LogPlugin},
    prelude::*,
    render::render_resource::Extent3d,
    window::CursorGrabMode,
};
use bevy_hanabi::HanabiPlugin;
use bevy_rapier3d::prelude::*;
use character::{Character, CharacterPlugin, CharacterSet};
use damage::DamagePlugin;
use humanoid::HumanoidPlugin;
use dialogue::{DialoguePlugin, DialogueEvent, DialogueMap, asset_gen::DialogueAssetLoadState};
use image::io::Reader as ImageReader;
use item::{ItemPlugins, ItemSet};
use render::{sketched::SketchMaterial, RenderFXPlugins};
use sound::SoundPlugin;
use time::{RewindComponentPlugin, RewindPlugin};

use crate::asset::{AssetLoadState, DynamicAssetPlugin};

fn main() -> Result<(), io::Error> {
    let mut app = App::new();

    #[cfg(debug_assertions)]
    if env::var("GENERATE_ASSETS").is_ok() {
        texture_array![1usize, "skin"]
            .save("assets/textures/generated/skin.png")
            .unwrap();
        texture_array![2usize, "skin", "eyes", "grin"]
            .save("assets/textures/generated/grin.png")
            .unwrap();
        texture_array![2usize, "skin", "smirk"]
            .save("assets/textures/generated/smirk.png")
            .unwrap();
        texture_array![2usize, "skin", "eyes", "meh"]
            .save("assets/textures/generated/meh.png")
            .unwrap();
        texture_array![2usize, "skin", "eyes", "grizz"]
            .save("assets/textures/generated/grizz.png")
            .unwrap();
        texture_array![2usize, "skin", "eyes", "smile"]
            .save("assets/textures/generated/smile.png")
            .unwrap();
    }

    let default_plugins = DefaultPlugins.set(AssetPlugin {
        watch_for_changes: true,
        ..Default::default()
    });

    #[cfg(debug_assertions)]
    let default_plugins = default_plugins.set(LogPlugin {
        level: Level::INFO,
        filter: "info,wgpu_core=warn,wgpu_hal=warn,grin=info".into(),
    });

    #[cfg(not(debug_assertions))]
    let default_plugins = default_plugins.set(LogPlugin {
        level: Level::DEBUG,
        filter: "info,wgpu_core=warn,wgpu_hal=warn,grin=debug".into(),
    });

    app.add_plugins(default_plugins);

    #[cfg(debug_assertions)]
    app.init_resource::<Msaa>()
        .init_resource::<AmbientLight>()
        .add_plugin(DynamicAssetPlugin)
        .add_plugin(RapierPhysicsPlugin::<NoUserData>::default())
        .add_plugin(LogDiagnosticsPlugin::default())
        .add_plugins(RenderFXPlugins)
        .add_plugin(HanabiPlugin)
        .add_plugin(HumanoidPlugin)
        .add_plugins(ItemPlugins)
        .add_plugin(CharacterPlugin)
        .add_plugins(AIPlugins)
        .add_plugin(DamagePlugin)
        .add_plugin(SoundPlugin)
        .add_systems((load_scene, Dummy::spawn).in_schedule(OnEnter(AssetLoadState::Success)))
        .add_plugin(DialoguePlugin)
        .add_plugin(RewindPlugin::default())
        .add_plugin(RewindComponentPlugin::<Transform>::default())
        .add_system(load_scene.in_schedule(OnEnter(AssetLoadState::Success)))
        .add_system(test_dialogue.in_schedule(OnEnter(DialogueAssetLoadState::Success)))
        // ensure that all humanoids exist before potentially adding items directly to them
        .add_system(
            apply_system_buffers
                .after(CharacterSet::Spawn)
                .before(ItemSet::Spawn),
        )
        .add_system(bevy::window::close_on_esc);

    app.run();

    Ok(())
}

fn load_scene(
    mut commands: Commands,
    mut windows: Query<&mut Window>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<SketchMaterial>>,
    fallback: Res<FallbackImage>,
) {
    let mut window = windows.single_mut();
    window.cursor.grab_mode = CursorGrabMode::Locked;
    //window.cursor.visible = false;
    window.cursor.icon = CursorIcon::Crosshair;

    let _extent = Extent3d {
        width: window.physical_width(),
        height: window.physical_height(),
        ..Default::default()
    };

    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: 1500.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..default()
    });

    let plane = Mesh::from(shape::Plane::from_size(50.0));
    commands.spawn((
        MaterialMeshBundle {
            mesh: meshes.add(plane.clone()),
            material: materials.add(SketchMaterial {
                base_color: Color::GREEN,
                base_color_texture: Some(fallback.texture.clone()),
                ..Default::default()
            }),
            transform: Transform::from_xyz(0.0, -1e-4, 0.0),
            ..Default::default()
        },
        Collider::from_bevy_mesh(&plane, &ComputedColliderShape::TriMesh).unwrap(),
    ));
}

fn test_dialogue(mut events: EventWriter<DialogueEvent>, dialogue_map: Res<DialogueMap>) {
    events.send(DialogueEvent::Say(dialogue_map.0["test_1"].clone()));
}
