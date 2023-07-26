use std::ops::Range;

use bevy::{ecs::event::ManualEventReader, input::mouse::MouseMotion, prelude::*};
use bevy_rapier3d::{
    na::clamp,
    prelude::{CollisionGroups, Group, QueryFilter, RapierContext},
};

use crate::humanoid::Humanoid;

use super::{AvatarLoadState, Player};

pub struct PlayerCameraPlugin;

impl Plugin for PlayerCameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LookInfo>()
            .init_resource::<MouseOpts>()
            .add_systems(Update, (handle_mouse, cam_update).chain())
            .add_systems(OnEnter(AvatarLoadState::Loaded), spawn_camera);
    }
}

/// Camera for standard player character.
#[derive(Component)]
pub struct PlayerCamera {
    pub target: Entity,
    pub alignment: CameraAlignment,
}

#[derive(Default)]
pub enum CameraAlignment {
    /// Angled downwards.
    #[default]
    FortyFive,
    Shooter,
}

#[derive(Resource, Default)]
pub struct LookInfo {
    pub reader_motion: ManualEventReader<MouseMotion>,
    pub pitch: f32,
    pub yaw: f32,
    pub viewport_ray: Ray,
    pub y_ray: Ray,
    pub target_distance: f32,
}

impl LookInfo {
    pub fn target_point(&self) -> Vec3 {
        self.viewport_ray.get_point(self.target_distance)
    }

    pub fn vertical_target_point(&self, plane_origin: Vec3, plane_normal: Vec3) -> Option<Vec3> {
        self.y_ray
            .intersect_plane(plane_origin, plane_normal)
            .map(|t| self.y_ray.get_point(t))
    }
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
    /// Maximum mouse target distance.
    target_distance_cap: f32,
}

impl Default for MouseOpts {
    fn default() -> Self {
        Self {
            // I *think* this is what CS:GO uses?
            sens_x: 0.022,
            sens_y: 0.022,
            pitch_bounds: Some(-20.0_f32.to_radians()..70.0_f32.to_radians()),
            target_distance_cap: 128.0,
        }
    }
}

/// Writes to the `LookInfo` resource based on mouse input.
fn handle_mouse(
    mut mouse_info: ResMut<LookInfo>,
    mouse_opts: Res<MouseOpts>,
    motion: Res<Events<MouseMotion>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<PlayerCamera>>,
    window_query: Query<&Window>,
    rapier_context: Res<RapierContext>,
) {
    if camera_query.get_single().is_err() {
        return;
    }

    let window = window_query.single();

    let look_info = mouse_info.as_mut();
    for event in look_info.reader_motion.iter(&motion) {
        look_info.yaw -= (event.delta.x * mouse_opts.sens_x).to_radians();
        look_info.pitch -= (event.delta.y * mouse_opts.sens_y).to_radians();
    }
    if let Some(pitch_bounds) = &mouse_opts.pitch_bounds {
        look_info.pitch = clamp(look_info.pitch, pitch_bounds.start, pitch_bounds.end);
    }
    let (camera, camera_transform) = camera_query.single();
    if let Some(size) = camera.logical_viewport_size() {
        look_info.viewport_ray = camera
            .viewport_to_world(camera_transform, size / 2.0)
            .unwrap_or_default();

        if let Some(cursor_pos) = window.cursor_position() {
            look_info.y_ray = camera
                .viewport_to_world(camera_transform, cursor_pos)
                .unwrap_or_default();
        }

        look_info.target_distance = rapier_context
            .cast_ray(
                look_info.viewport_ray.origin,
                look_info.viewport_ray.direction,
                mouse_opts.target_distance_cap,
                false,
                QueryFilter::new().groups(CollisionGroups::new(
                    Group::all(),
                    Group::all().difference(Group::GROUP_1),
                )),
            )
            .map_or(mouse_opts.target_distance_cap, |hit| hit.1);
    }
}

fn spawn_camera(mut commands: Commands, query: Query<Entity, (With<Humanoid>, With<Player>)>) {
    let e_plr = query.single();
    commands.spawn((
        PlayerCamera {
            target: e_plr,
            alignment: CameraAlignment::FortyFive,
        },
        Camera3dBundle {
            transform: Transform::from_xyz(0.0, 32.0, 0.0).looking_to(Vec3::NEG_Z, Vec3::Y),
            ..Default::default()
        },
    ));
}

fn cam_update(
    mut query: Query<(&mut Transform, &PlayerCamera)>,
    transform_query: Query<&GlobalTransform, Without<PlayerCamera>>,
    look_info: Res<LookInfo>,
) {
    if let Ok((mut transform, PlayerCamera { target, alignment })) = query.get_single_mut() {
        let g_target_transform = transform_query.get(*target).unwrap();
        match alignment {
            CameraAlignment::FortyFive => {
                let target = g_target_transform.translation();
                let offset = Vec3::new(0.0, 24.0, 24.0);
                let origin = target + offset;
                let look = (-offset).normalize();
                *transform =
                    Transform::from_translation(origin).looking_to(look, look.cross(Vec3::NEG_X));
            }
            CameraAlignment::Shooter => {
                transform.rotation =
                    Quat::from_euler(EulerRot::YXZ, look_info.yaw, look_info.pitch, 0.0);
                transform.translation = g_target_transform.transform_point(
                    Vec3::new(0.0, 3.0, 0.0)
                        + Vec3::new(0.0, -look_info.pitch.sin(), look_info.pitch.cos()) * 12.0,
                );
            }
        }
    }
}

#[derive(Component)]
struct DebugMouseMarker;

fn add_debug_mouse_marker(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        DebugMouseMarker,
        MaterialMeshBundle {
            mesh: meshes.add(Mesh::from(shape::UVSphere {
                radius: 0.25,
                ..Default::default()
            })),
            material: materials.add(StandardMaterial::from(Color::RED)),
            ..Default::default()
        },
    ));
}

fn update_debug_mouse_marker(
    mut marker_query: Query<&mut Transform, With<DebugMouseMarker>>,
    look_info: Res<LookInfo>,
) {
    let mut transform = marker_query.single_mut();
    *transform = Transform::from_translation(look_info.target_point());
}

pub struct DebugMouseTargetPlugin;

impl Plugin for DebugMouseTargetPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, add_debug_mouse_marker)
            .add_systems(Update, update_debug_mouse_marker);
    }
}
