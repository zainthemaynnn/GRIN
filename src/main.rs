use std::{env, io};

use bevy::{
    audio::{AudioPlugin, SpatialScale},
    diagnostic::LogDiagnosticsPlugin,
    log::{Level, LogPlugin},
    prelude::*,
    window::CursorGrabMode,
};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use grin_ai::{spawn::EnemySpawn, AiPlugins};
use grin_asset::{texture_array, AssetLoadState, DynamicAssetPlugin};
use grin_character::{CharacterPlugins, CharacterSet};
use grin_damage::plugin::DamagePlugins;
use grin_dialogue::{DialogueEvent, DialogueMap};
use grin_item::{
    library::plugin::ItemLibrary,
    plugin::{ItemPlugins, ItemSet},
};
use grin_map::{Map, MapLoadState, MapPlugin};
use grin_physics::GrinPhysicsPlugin;
use grin_render::RenderFXPlugins;
use grin_rig::{humanoid::HumanoidPlugin, GrinAnimationPlugin};
use grin_time::{scaling::TimeScalePlugin, RewindComponentPlugin, RewindPlugin};
use grin_util::{
    event::{DefaultSpawnable, TweenEventPlugin},
    spatial::SpatialPlugin,
};

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

    let default_plugins = DefaultPlugins.set(AudioPlugin {
        default_spatial_scale: SpatialScale::new(16.0),
        ..Default::default()
    });

    #[cfg(debug_assertions)]
    let default_plugins = default_plugins.set(LogPlugin {
        level: Level::DEBUG,
        filter:
            "info,wgpu_core=warn,wgpu_hal=warn,naga=warn,grin=debug,grin_ai=warn,bevy_gltf=error"
                .into(),
        ..Default::default()
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
            LogDiagnosticsPlugin::default(),
            WorldInspectorPlugin::new(),
            TweenEventPlugin,
            GrinPhysicsPlugin {
                debug_enabled: true,
                ..Default::default()
            },
            RenderFXPlugins,
            HumanoidPlugin,
            ItemPlugins,
            ItemLibrary,
            CharacterPlugins,
            AiPlugins,
            DamagePlugins,
            //DialoguePlugin,
            MapPlugin {
                navmesh_debugging: None,
            },
        ))
        .add_plugins((
            TimeScalePlugin,
            RewindPlugin::default(),
            RewindComponentPlugin::<Transform>::default(),
            SpatialPlugin,
            GrinAnimationPlugin,
        ))
        .add_systems(OnEnter(AssetLoadState::Success), load_scene)
        .add_systems(
            OnEnter(AssetLoadState::Success),
            grin_character::kit::smirk::Smirk::spawn_default().before(CharacterSet::Spawn),
        )
        .add_systems(
            OnEnter(MapLoadState::Success),
            |mut events: EventWriter<EnemySpawn<grin_ai::Dummy>>| {
                events.send(EnemySpawn {
                    transform: Transform::from_xyz(0.0, 1E-2, -10.0),
                    ..Default::default()
                });
            },
        )
        //.add_systems(OnEnter(DialogueAssetLoadState::Success), test_dialogue)
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
            scene: asset_server.load("gltf/rooftop.glb#Scene0"),
            ..Default::default()
        },
    ));
}

fn test_dialogue(mut events: EventWriter<DialogueEvent>, dialogue_map: Res<DialogueMap>) {
    events.send(DialogueEvent::Say(dialogue_map.0["test_1"].clone()));
}
