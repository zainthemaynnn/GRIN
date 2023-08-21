mod ai;
mod asset;
mod character;
mod damage;
mod dialogue;
mod humanoid;
mod item;
mod map;
mod physics;
mod render;
mod sound;
mod time;
mod util;

use std::{env, io, time::Duration};

use ai::{boombox::BoomBox, dummy::Dummy, AIPlugins};
use asset::{AssetLoadState, DynamicAssetPlugin};
use bevy::{
    asset::ChangeWatcher,
    diagnostic::LogDiagnosticsPlugin,
    log::{Level, LogPlugin},
    prelude::*,
    window::CursorGrabMode,
};
use bevy_hanabi::HanabiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_landmass::LandmassPlugin;
use bevy_rapier3d::prelude::*;
use bevy_tweening::TweeningPlugin;
use character::{CharacterPlugin, CharacterSet};
use damage::DamagePlugin;
use dialogue::{asset_gen::DialogueAssetLoadState, DialogueEvent, DialogueMap, DialoguePlugin};
use humanoid::{HumanoidPlugin, HUMANOID_HEIGHT};
use image::io::Reader as ImageReader;
use item::{ItemPlugins, ItemSet};
use map::{Map, MapPlugin};
use physics::GrinPhysicsPlugin;
use render::RenderFXPlugins;
use sound::SoundPlugin;
use time::{scaling::TimeScalePlugin, RewindComponentPlugin, RewindPlugin};
use util::tween::TweenEventPlugin;

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
        watch_for_changes: Some(ChangeWatcher {
            delay: Duration::from_secs(5),
        }),
        ..Default::default()
    });

    #[cfg(debug_assertions)]
    let default_plugins = default_plugins.set(LogPlugin {
        level: Level::DEBUG,
        filter: "info,wgpu_core=warn,wgpu_hal=warn,grin=debug,naga=warn".into(),
    });

    #[cfg(not(debug_assertions))]
    let default_plugins = default_plugins.set(LogPlugin {
        level: Level::INFO,
        filter: "info,wgpu_core=warn,wgpu_hal=warn,grin=info,naga=warn".into(),
    });

    app.add_plugins(default_plugins);

    app.init_resource::<Msaa>()
        .init_resource::<AmbientLight>()
        .add_plugins((
            DynamicAssetPlugin,
            RapierPhysicsPlugin::<NoUserData>::default(),
            LogDiagnosticsPlugin::default(),
            TweeningPlugin,
            TweenEventPlugin,
            HanabiPlugin,
            GrinPhysicsPlugin,
            RenderFXPlugins,
            HumanoidPlugin,
            ItemPlugins,
            CharacterPlugin,
            AIPlugins,
            DamagePlugin,
            SoundPlugin,
            DialoguePlugin,
        ))
        .add_plugins((
            TimeScalePlugin,
            RewindPlugin::default(),
            RewindComponentPlugin::<Transform>::default(),
            //RapierDebugRenderPlugin::default(),
            WorldInspectorPlugin::new(),
            LandmassPlugin,
            MapPlugin {
                navmesh_debugging: Some(Color::WHITE),
            },
        ))
        .insert_resource(RapierConfiguration {
            gravity: Vec3::NEG_Y * 9.81 * (HUMANOID_HEIGHT / 1.8),
            ..Default::default()
        })
        .add_systems(
            OnEnter(AssetLoadState::Success),
            (
                load_scene,
                Dummy::spawn_at(Transform::from_xyz(10.0, 1E-2, 0.0)),
                BoomBox::spawn_at(Transform::from_xyz(-10.0, 1E-2, 0.0)),
            ),
        )
        .add_systems(OnEnter(DialogueAssetLoadState::Success), test_dialogue)
        .add_systems(
            Update,
            (
                // ensure that all humanoids exist before potentially adding items directly to them
                apply_deferred
                    .after(CharacterSet::Spawn)
                    .before(ItemSet::Spawn),
                bevy::window::close_on_esc,
            ),
        );

    app.run();

    Ok(())
}

fn load_scene(
    mut commands: Commands,
    mut windows: Query<&mut Window>,
    asset_server: Res<AssetServer>,
) {
    let mut window = windows.single_mut();
    window.cursor.grab_mode = CursorGrabMode::Locked;
    //window.cursor.visible = false;
    window.cursor.icon = CursorIcon::Crosshair;

    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: 1500.0,
            shadows_enabled: true,
            ..Default::default()
        },
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..Default::default()
    });

    commands.spawn((
        Map,
        SceneBundle {
            scene: asset_server.load("meshes/cubes.glb#Scene0"),
            ..Default::default()
        },
    ));
}

fn test_dialogue(mut events: EventWriter<DialogueEvent>, dialogue_map: Res<DialogueMap>) {
    events.send(DialogueEvent::Say(dialogue_map.0["test_1"].clone()));
}
