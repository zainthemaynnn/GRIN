// TODO: see if this old thing actually works :P

use bevy::prelude::*;
use bevy_tweening::{component_animator_system, Lens};

pub struct BeamPlugin;

impl Plugin for BeamPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                render_beams,
                set_beam_radius,
                component_animator_system::<Beam>,
            ),
        );
    }
}

/// Cylindrical (capsule-ical?) beam.
///
/// This is less efficient than `render::DuoQuad`. However, it looks better with large volumes.
///
/// Note that `Transform.scale` is linked to the radius of beam and thus manipulating it directly
/// might lead to overwrites. Consider setting `Beam.radius` instead.
#[derive(Component)]
pub struct Beam {
    pub origin: Vec3,
    pub target: Vec3,
    pub initial_radius: f32,
    pub radius: f32,
}

impl Beam {
    pub fn new(origin: Vec3, target: Vec3, radius: f32) -> Self {
        Self {
            origin,
            target,
            initial_radius: radius,
            radius,
        }
    }
}

impl Default for Beam {
    fn default() -> Self {
        Self::new(Vec3::default(), Vec3::default(), 1.0)
    }
}

#[derive(Bundle, Default)]
pub struct BeamBundle<M: Material> {
    pub beam: Beam,
    pub material: Handle<M>,
    pub visibility: Visibility,
    pub computed: ComputedVisibility,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
}

#[derive(Component, Default)]
pub struct BeamRadiusLens {
    pub start: f32,
    pub end: f32,
}

impl Lens<Beam> for BeamRadiusLens {
    fn lerp(&mut self, target: &mut Beam, ratio: f32) {
        target.radius = self.start + (self.end - self.start) * ratio;
    }
}

pub fn render_beams(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut beam_query: Query<(Entity, &Beam, &mut Transform)>,
) {
    for (
        e_beam,
        Beam {
            origin,
            target,
            initial_radius,
            ..
        },
        mut transform,
    ) in beam_query.iter_mut()
    {
        let axis = *target - *origin;
        let mdpt = origin.lerp(*target, 0.5);
        let perp = axis.normalize().any_orthonormal_pair();
        *transform = Transform {
            translation: mdpt,
            rotation: Quat::from_mat3(&Mat3::from_cols(perp.0, axis.normalize(), perp.1)),
            scale: Vec3::ONE,
        };

        let h_mesh = meshes.add(Mesh::from(shape::Capsule {
            radius: *initial_radius,
            depth: axis.length(),
            ..Default::default()
        }));
        commands.entity(e_beam).insert(h_mesh);
    }
}

// sane person: `Transform.scale` is bad practice :(((
// me (gangsta): we do a little trolling B)
pub fn set_beam_radius(mut beam_query: Query<(&Beam, &mut Transform), Changed<Beam>>) {
    for (beam, mut beam_transform) in beam_query.iter_mut() {
        let horiz_scale = beam.radius / beam.initial_radius;
        beam_transform.scale = Vec3::new(horiz_scale, 1.0, horiz_scale);
    }
}
