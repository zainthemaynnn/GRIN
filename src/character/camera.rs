use std::ops::Range;

use bevy::{ecs::event::ManualEventReader, input::mouse::MouseMotion, prelude::*};
use bevy_rapier3d::{
    na::clamp,
    prelude::{CollisionGroups, Group, QueryFilter, RapierContext},
};

pub struct PlayerCameraPlugin;

impl Plugin for PlayerCameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LookInfo>()
            .init_resource::<MouseOpts>()
            .add_systems((handle_mouse, cam_update).chain());
    }
}

/// Camera for standard player character.
#[derive(Component, Default)]
pub struct PlayerCamera;

#[derive(Resource, Default)]
pub struct LookInfo {
    pub reader_motion: ManualEventReader<MouseMotion>,
    pub pitch: f32,
    pub yaw: f32,
    pub viewport_ray: Ray,
    pub target_distance: f32,
}

impl LookInfo {
    pub fn target_point(&self) -> Vec3 {
        self.viewport_ray.get_point(self.target_distance)
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
    mut window_query: Query<&mut Window>,
    rapier_context: Res<RapierContext>,
) {
    if camera_query.get_single().is_err() {
        return;
    }

    let mut window = window_query.single_mut();
    let (x, y) = (window.resolution.width(), window.resolution.height());
    window.set_cursor_position(Some(Vec2 { x, y } / 2.0));

    let mut look_info = mouse_info.as_mut();
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
            .unwrap_or(Ray::default());

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

fn cam_update(mut query: Query<&mut Transform, With<PlayerCamera>>, look_info: Res<LookInfo>) {
    if let Ok(mut transform) = query.get_single_mut() {
        transform.rotation = Quat::from_rotation_x(look_info.pitch);
        transform.translation = Vec3::new(0.0, 3.0, 0.0)
            + Vec3::new(0.0, -look_info.pitch.sin(), look_info.pitch.cos()) * 12.0;
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
        app.add_startup_system(add_debug_mouse_marker)
            .add_system(update_debug_mouse_marker);
    }
}
