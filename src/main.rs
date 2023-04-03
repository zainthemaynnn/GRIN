mod outlines;
mod texture_ops;

use std::{array::IntoIter, io, ops::Range};

use bevy::{
    asset::LoadState,
    ecs::event::ManualEventReader,
    input::mouse::MouseMotion,
    prelude::*,
    reflect::TypeUuid,
    render::render_resource::{AsBindGroup, Extent3d, ShaderRef},
    window::CursorGrabMode,
};
use bevy_mod_outline::{
    AutoGenerateOutlineNormalsPlugin, InheritOutlineDepth, OutlineAnimation, OutlineBundle,
    OutlinePlugin, OutlineVolume,
};
use bevy_rapier3d::{na::clamp, prelude::*};
use image::io::Reader as ImageReader;
use outlines::{GlobalMeshOutline, GlobalMeshOutlinePlugin};
use texture_ops::{TextureArrayBuilder, TextureBuilder};

const PHYSICS_UPDATE_SECS: f32 = 1. / 60.;

fn main() -> Result<(), io::Error> {
    // shouldn't be generated at runtime but who's watching?
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

    let mut app = App::new();

    app.init_resource::<Msaa>()
        .init_resource::<MouseOpts>()
        .init_resource::<LookInfo>()
        .init_resource::<AmbientLight>()
        .init_resource::<TextureHandles>()
        .init_resource::<MeshHandles>()
        .add_state::<AppState>()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            watch_for_changes: true,
            ..Default::default()
        }))
        .add_plugin(RapierPhysicsPlugin::<NoUserData>::default())
        .add_plugin(MaterialPlugin::<AnimatedMaterial>::default())
        .add_plugin(RapierDebugRenderPlugin::default())
        .add_plugin(OutlinePlugin)
        .add_plugin(AutoGenerateOutlineNormalsPlugin)
        .add_plugin(GlobalMeshOutlinePlugin {
            bundle: OutlineBundle {
                outline: OutlineVolume {
                    colour: Color::BLACK,
                    width: 8.0,
                    visible: true,
                },
                animation: OutlineAnimation {
                    rate_millis: 1000.0,
                },
                ..Default::default()
            },
        })
        .add_system(load_assets.in_schedule(OnEnter(AppState::Setup)))
        .add_system(check_assets.in_set(OnUpdate(AppState::Setup)))
        .add_system(reinterpret_array_textures.in_schedule(OnEnter(AppState::Finished)))
        .add_system(load_scene.in_schedule(OnEnter(AppState::Finished)))
        .add_systems((animate_materials, handle_mouse, char_update, cam_update).chain())
        .add_system(bevy::window::close_on_esc);

    app.run();

    Ok(())
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
enum AppState {
    #[default]
    Setup,
    Finished,
}

impl States for AppState {
    type Iter = IntoIter<AppState, 2>;

    fn variants() -> Self::Iter {
        [AppState::Setup, AppState::Finished].into_iter()
    }
}

#[derive(Resource, Default)]
struct TextureHandles {
    skin: Handle<Image>,
    face_smirk: Handle<Image>,
    face_meh: Handle<Image>,
}

#[derive(Resource, Default)]
struct MeshHandles {
    mbody: Handle<Mesh>,
    fbody: Handle<Mesh>,
    head: Handle<Mesh>,
    hand: Handle<Mesh>,
    shades: Handle<Mesh>,
}

fn load_assets(
    mut textures: ResMut<TextureHandles>,
    mut meshes: ResMut<MeshHandles>,
    asset_server: Res<AssetServer>,
) {
    textures.skin = asset_server.load("textures/generated/skin.png");
    textures.face_smirk = asset_server.load("textures/generated/smirk.png");
    textures.face_meh = asset_server.load("textures/generated/meh.png");

    meshes.mbody = asset_server.load("meshes/mbody.glb#Mesh0/Primitive0");
    meshes.fbody = asset_server.load("meshes/fbody.glb#Mesh0/Primitive0");
    meshes.hand = asset_server.load("meshes/hand.glb#Mesh0/Primitive0");
    meshes.head = asset_server.load("meshes/mbody.glb#Mesh1/Primitive0");
    meshes.shades = asset_server.load("meshes/pizza_shades.glb#Mesh0/Primitive0");
}

fn check_assets(
    mut next_state: ResMut<NextState<AppState>>,
    textures: Res<TextureHandles>,
    meshes: ResMut<MeshHandles>,
    asset_server: Res<AssetServer>,
) {
    println!("loading assets");

    macro_rules! as_untyped_ids {
        [$( $handle:expr ),*] => {
            {
                let mut v = Vec::new();
                $( v.push($handle.clone_untyped().id()); )*
                v
            }
        };
    }

    if let LoadState::Loaded = asset_server.get_group_load_state(as_untyped_ids![
        textures.face_smirk,
        textures.face_meh,
        meshes.mbody,
        meshes.fbody,
        meshes.head,
        meshes.hand
    ]) {
        println!("loaded");
        next_state.set(AppState::Finished);
    }
}

fn reinterpret_array_textures(
    texture_handles: Res<TextureHandles>,
    mut textures: ResMut<Assets<Image>>,
) {
    textures
        .get_mut(&texture_handles.face_smirk)
        .unwrap()
        .reinterpret_stacked_2d_as_array(2);
    textures
        .get_mut(&texture_handles.face_meh)
        .unwrap()
        .reinterpret_stacked_2d_as_array(2);
}

fn animate_materials(
    mut materials: ResMut<Assets<AnimatedMaterial>>,
    time: Res<Time>,
) {
    for (_, mut material) in materials.iter_mut() {
        material.t = time.elapsed_seconds_f64() as f32;
    }
}

/// Root object for standard player character.
#[derive(Component)]
struct PlayerCharacter;

/// Camera for standard player character.
#[derive(Component)]
struct PlayerCamera;

/// Optional head for standard player character.
#[derive(Component)]
struct PlayerHead;

/// Optional hand for standard player character.
#[derive(Component)]
struct PlayerHand;

#[derive(Resource, Default)]
struct LookInfo {
    reader_motion: ManualEventReader<MouseMotion>,
    pitch: f32,
    yaw: f32,
}

/// Mouse settings.
#[derive(Resource)]
struct MouseOpts {
    /// Mouse X sensitivity in degrees/px.
    sens_x: f32,
    /// Mouse Y sensitivity in degrees/px.
    sens_y: f32,
    /// Constraints for pitch angle.
    pitch_bounds: Option<Range<f32>>,
}

impl Default for MouseOpts {
    fn default() -> Self {
        Self {
            // I *think* this is what CS:GO uses?
            sens_x: 0.022,
            sens_y: 0.022,
            pitch_bounds: Some(-30.0_f32.to_radians()..70.0_f32.to_radians()),
        }
    }
}

/// Writes to the `LookInfo` resource based on mouse input.
fn handle_mouse(
    mut mouse_info: ResMut<LookInfo>,
    mouse_opts: Res<MouseOpts>,
    motion: Res<Events<MouseMotion>>,
) {
    let mut look_info = mouse_info.as_mut();
    for event in look_info.reader_motion.iter(&motion) {
        look_info.yaw -= (event.delta.x * mouse_opts.sens_x).to_radians();
        look_info.pitch -= (event.delta.y * mouse_opts.sens_y).to_radians();
    }
    if let Some(pitch_bounds) = &mouse_opts.pitch_bounds {
        look_info.pitch = clamp(look_info.pitch, pitch_bounds.start, pitch_bounds.end);
    }
}

// TODO: replace with optimized meshes
fn create_mbody_collider() -> Collider {
    unimplemented!()
}

fn create_fbody_collider() -> Collider {
    unimplemented!()
}

fn load_scene(
    mut commands: Commands,
    mut windows: Query<&mut Window>,
    texture_handles: Res<TextureHandles>,
    mesh_handles: Res<MeshHandles>,
    outline: Res<GlobalMeshOutline>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut amaterials: ResMut<Assets<AnimatedMaterial>>,
) {
    let mut window = windows.single_mut();
    window.cursor.grab_mode = CursorGrabMode::Locked;
    window.cursor.visible = false;
    window.set_cursor_position(None);

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

    commands.spawn((PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Plane::from_size(50.0))),
        material: materials.add(StandardMaterial {
            base_color: Color::SEA_GREEN,
            ..Default::default()
        }),
        ..Default::default()
    },));

    commands
        .spawn((
            RigidBody::KinematicPositionBased,
            Velocity::default(),
            KinematicCharacterController::default(),
            PlayerCharacter,
            MaterialMeshBundle {
                mesh: mesh_handles.mbody.clone(),
                material: materials.add(StandardMaterial {
                    base_color: Color::GRAY,
                    ..Default::default()
                }),
                transform: Transform::from_xyz(0.0, 1.75 / 2.0, 0.0),
                ..Default::default()
            },
            Collider::from_bevy_mesh(
                meshes.get(&mesh_handles.mbody).unwrap(),
                &ComputedColliderShape::TriMesh,
            )
            .unwrap(),
            outline.bundle.clone(),
        ))
        .with_children(|parent| {
            parent.spawn((
                Camera3dBundle {
                    transform: Transform::from_xyz(0.0, 2.0, -6.0).looking_at(Vec3::Z, Vec3::Y),
                    ..Default::default()
                },
                PlayerCamera,
            ));

            parent
                .spawn((
                    PlayerHead,
                    MaterialMeshBundle {
                        mesh: mesh_handles.head.clone(),
                        material: amaterials.add(AnimatedMaterial {
                            texture: texture_handles.face_smirk.clone(),
                            t: 0.0,
                        }),
                        transform: Transform::from_xyz(0.0, 1.25, 0.0),
                        ..Default::default()
                    },
                    Collider::from_bevy_mesh(
                        meshes.get(&mesh_handles.head).unwrap(),
                        &ComputedColliderShape::TriMesh,
                    )
                    .unwrap(),
                    outline.bundle.clone(),
                    InheritOutlineDepth,
                ))
                .with_children(|parent| {
                    parent.spawn((
                        PbrBundle {
                            mesh: mesh_handles.shades.clone(),
                            material: materials.add(StandardMaterial {
                                base_color: Color::BLACK,
                                emissive: Color::rgb(0.05, 0.05, 0.05),
                                perceptual_roughness: 0.2,
                                reflectance: 1.0,
                                ..Default::default()
                            }),
                            transform: Transform::from_xyz(0.0, 0.0, 0.5),
                            ..Default::default()
                        },
                        {
                            let mut outline_bundle = outline.bundle.clone();
                            outline_bundle.outline.width = 5.0;
                            outline_bundle
                        },
                    ));
                });

            for ofst in [-0.75, 0.75] {
                parent.spawn((
                    PlayerHand,
                    MaterialMeshBundle {
                        mesh: mesh_handles.hand.clone(),
                        material: materials.add(StandardMaterial {
                            base_color_texture: Some(texture_handles.skin.clone()),
                            ..Default::default()
                        }),
                        transform: Transform::from_xyz(ofst, 0.0, 0.0),
                        ..Default::default()
                    },
                    outline.bundle.clone(),
                ));
            }
        });
}

fn char_update(
    input: Res<Input<KeyCode>>,
    mut query: ParamSet<(
        Query<(&mut KinematicCharacterController, &mut Transform), With<PlayerCharacter>>,
        Query<&mut Transform, With<PlayerHead>>,
    )>,
    look_info: Res<LookInfo>,
) {
    if let Ok((mut char_controller, mut transform)) = query.p0().get_single_mut() {
        let pos0 = char_controller.translation.unwrap_or(Vec3::ZERO);
        let mut movement = Vec3::ZERO;
        if input.pressed(KeyCode::W) {
            movement += Vec3::Z;
        }
        if input.pressed(KeyCode::A) {
            movement += Vec3::X;
        }
        if input.pressed(KeyCode::S) {
            movement -= Vec3::Z;
        }
        if input.pressed(KeyCode::D) {
            movement -= Vec3::X;
        }
        char_controller.translation =
            Some(pos0 + movement.normalize_or_zero() * 3.0 * PHYSICS_UPDATE_SECS);
        transform.rotation = Quat::from_rotation_y(look_info.yaw);

        let mut p1 = query.p1();
        if let Ok(mut head_transform) = p1.get_single_mut() {
            head_transform.rotation = Quat::from_rotation_x(-look_info.pitch * 0.75);
        }
    }
}

fn cam_update(mut query: Query<&mut Transform, With<PlayerCamera>>, look_info: Res<LookInfo>) {
    if let Ok(mut transform) = query.get_single_mut() {
        let (y, _x, z) = transform.rotation.to_euler(EulerRot::YXZ);
        transform.rotation = Quat::from_euler(EulerRot::YXZ, y, look_info.pitch, z);
    }
}

#[derive(AsBindGroup, TypeUuid, Debug, Clone)]
#[uuid = "9c5a0ddf-1eaf-41b4-9832-ed736fd26af3"]
pub struct AnimatedMaterial {
    #[texture(0, dimension = "2d_array")]
    #[sampler(1)]
    texture: Handle<Image>,
    #[uniform(2)]
    t: f32,
}

impl Material for AnimatedMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/animated_material.wgsl".into()
    }
}
