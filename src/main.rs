mod asset;
mod character;
mod collisions;
mod humanoid;
mod render;
mod util;
mod weapon;

use std::io;

use asset::FallbackImage;
use bevy::{
    diagnostic::FrameTimeDiagnosticsPlugin, prelude::*, render::render_resource::Extent3d,
    window::CursorGrabMode,
};
use bevy_hanabi::HanabiPlugin;
use bevy_rapier3d::prelude::*;
use character::CharacterPlugin;
use humanoid::*;
use image::io::Reader as ImageReader;
use render::sketched::{
    GlobalMeshOutline, OutlineBundle, OutlineVolume, SketchEffectPlugin, SketchMaterial,
};
use weapon::WeaponsPlugin;

use crate::asset::{AssetLoadState, DynamicAssetPlugin};

fn main() -> Result<(), io::Error> {
    let mut app = App::new();

    if cfg!(debug_assertions) {
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

    app.add_plugins(DefaultPlugins.set(AssetPlugin {
        watch_for_changes: true,
        ..Default::default()
    }));

    app.init_resource::<Msaa>()
        .init_resource::<AmbientLight>()
        .add_plugin(DynamicAssetPlugin)
        .add_plugin(RapierPhysicsPlugin::<NoUserData>::default())
        .add_plugin(FrameTimeDiagnosticsPlugin)
        .add_plugin(HanabiPlugin)
        .add_plugin(SketchEffectPlugin {
            outline: GlobalMeshOutline {
                standard: OutlineBundle {
                    outline: OutlineVolume {
                        colour: Color::BLACK,
                        width: 8.0,
                        visible: true,
                    },
                    ..Default::default()
                },
                mini: OutlineBundle {
                    outline: OutlineVolume {
                        colour: Color::BLACK,
                        width: 4.0,
                        visible: true,
                    },
                    ..Default::default()
                },
            },
            autofill_sketch_effect: true,
        })
        .add_plugin(HumanoidPlugin)
        .add_plugin(WeaponsPlugin)
        .add_plugin(CharacterPlugin)
        .add_system(load_scene.in_schedule(OnEnter(AssetLoadState::Success)))
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
