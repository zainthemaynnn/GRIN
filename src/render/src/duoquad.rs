// I'm never going to stop accidentally typing "dua" instead of "duo."
// is this just a me thing or what?
use std::{f32::consts::TAU, marker::PhantomData};

use super::sketched::{NoOutline, SketchMaterial};
use bevy::prelude::{shape::Quad, *};
use bevy_tweening::{component_animator_system, Lens};

pub struct DuoQuadPlugin;

impl Plugin for DuoQuadPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                init_duoquads,
                render_duoquads,
                inherit_materials,
                component_animator_system::<DuoQuad>,
                component_animator_system::<DuoQuadOrigin>,
                component_animator_system::<DuoQuadTarget>,
                render_endpoint::<DuoQuadOrigin>,
                render_endpoint::<DuoQuadTarget>,
            ),
        );
    }
}

/// [One of these good old guys.](https://europe1.discourse-cdn.com/unity/original/3X/f/5/f59ebcb69f41239818c24faaf762124f791d6241.jpeg)
/// Do they have an official name? I dunno. I made one up.
///
/// This is more efficient than `render::Beam`. However, it looks ugly with large volumes.
#[derive(Component, Default)]
pub struct DuoQuad {
    pub origin: Vec3,
    pub target: Vec3,
    pub radius: f32,
}

#[derive(Component, Default)]
pub struct DuoSubQuad;

#[derive(Bundle, Default)]
pub struct DuoQuadBundle {
    pub duoquad: DuoQuad,
    pub material: Handle<SketchMaterial>,
    pub visibility: Visibility,
    pub inherited_visibility: InheritedVisibility,
    pub view_visibility: ViewVisibility,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
}

#[derive(Component, Default)]
pub struct DuoQuadRadiusLens {
    pub start: f32,
    pub end: f32,
}

impl Lens<DuoQuad> for DuoQuadRadiusLens {
    fn lerp(&mut self, target: &mut DuoQuad, ratio: f32) {
        target.radius = self.start + (self.end - self.start) * ratio;
    }
}

pub fn init_duoquads(mut commands: Commands, query: Query<Entity, Added<DuoQuad>>) {
    for entity in query.iter() {
        commands.entity(entity).with_children(|parent| {
            parent.spawn((DuoSubQuad, NoOutline));
            parent.spawn((DuoSubQuad, NoOutline));
        });
    }
}

pub fn render_duoquads(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    // TODO: only re-render on change/addition
    // for some reason it doesn't work when I try to add query filters
    // this is pretty important for performance but ordering systems pisses me off too much right now
    mut duoquad_query: Query<(&DuoQuad, &mut Transform, &Handle<SketchMaterial>, &Children)>,
    quad_query: Query<(), With<DuoSubQuad>>,
) {
    for (
        DuoQuad {
            origin,
            target,
            radius,
        },
        mut transform,
        h_material,
        children,
    ) in duoquad_query.iter_mut()
    {
        let axis = *target - *origin;
        let mdpt = origin.lerp(*target, 0.5);
        let h_mesh = meshes.add(Mesh::from(Quad::new(Vec2::new(axis.length(), *radius))));
        let mut it = children.iter().filter(|e| quad_query.get(**e).is_ok());

        let perp = axis.normalize().any_orthonormal_pair();
        // this was the point where I found out `Quat::from_rotation_axes` is private.
        // maybe creating a quaternion from three axes is doing it wrong?
        // I'm not a good enough programmer or mathematician to figure that out
        // so I'm just gonna do it with a `Mat3`.
        *transform = Transform {
            translation: mdpt,
            rotation: Quat::from_mat3(&Mat3::from_cols(axis.normalize(), perp.0, perp.1)),
            scale: Vec3::ONE,
        };

        commands
            .entity(*it.next().unwrap())
            .insert(MaterialMeshBundle {
                mesh: h_mesh.clone(),
                material: h_material.clone(),
                ..Default::default()
            });

        commands
            .entity(*it.next().unwrap())
            .insert(MaterialMeshBundle {
                mesh: h_mesh.clone(),
                material: h_material.clone(),
                transform: Transform::from_rotation(Quat::from_rotation_x(TAU / 4.0)),
                ..Default::default()
            });
    }
}

pub fn inherit_materials(
    mut commands: Commands,
    duoquad_query: Query<
        (&Handle<SketchMaterial>, &Children),
        (With<DuoQuad>, Changed<Handle<SketchMaterial>>),
    >,
    quad_query: Query<Entity, With<DuoSubQuad>>,
) {
    for (h_material, children) in duoquad_query.iter() {
        for e_quad in children.iter().filter(|e| quad_query.get(**e).is_ok()) {
            commands.entity(*e_quad).insert(h_material.clone());
        }
    }
}

/// One of the endpoints on a `DuoQuad`.
pub trait DuoQuadEndpoint: Component + Clone {
    /// Radius of the quad at this location.
    fn radius(&self) -> f32;
    /// Sets the radius of the quad at this location.
    fn set_radius(&mut self, radius: f32);
    /// Translation of this endpoint *in the reference frame of the parent.*
    fn endpoint(&self, duoquad: &DuoQuad) -> Vec3;
}

/// Inserts a quad at the starting point of a `DuoQuad`.
#[derive(Component, Clone)]
pub struct DuoQuadOrigin {
    pub radius: f32,
}

impl DuoQuadEndpoint for DuoQuadOrigin {
    fn radius(&self) -> f32 {
        self.radius
    }

    fn set_radius(&mut self, radius: f32) {
        self.radius = radius;
    }

    fn endpoint(&self, duoquad: &DuoQuad) -> Vec3 {
        duoquad.origin
    }
}

/// Inserts a quad at the endpoint of a `DuoQuad`.
#[derive(Component, Clone)]
pub struct DuoQuadTarget {
    pub radius: f32,
}

impl DuoQuadEndpoint for DuoQuadTarget {
    fn radius(&self) -> f32 {
        self.radius
    }

    fn set_radius(&mut self, radius: f32) {
        self.radius = radius;
    }

    fn endpoint(&self, duoquad: &DuoQuad) -> Vec3 {
        duoquad.origin
    }
}

#[derive(Component, Default)]
pub struct DuoQuadEndpointLens<E: DuoQuadEndpoint> {
    pub start: f32,
    pub end: f32,
    pub phantom_data: PhantomData<E>,
}

impl<E: DuoQuadEndpoint> Lens<E> for DuoQuadEndpointLens<E> {
    fn lerp(&mut self, target: &mut E, ratio: f32) {
        target.set_radius(self.start + (self.end - self.start) * ratio);
    }
}

pub fn render_endpoint<T: DuoQuadEndpoint>(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    duoquad_query: Query<(&DuoQuad, &Transform, &Children)>,
    endpoint_query: Query<&T>,
    camera_query: Query<&Transform, With<Camera>>,
) {
    for cam_transform in camera_query.iter() {
        for (duoquad, transform, children) in duoquad_query.iter() {
            for child in children.iter() {
                if let Ok(endpoint) = endpoint_query.get(*child) {
                    let mesh_handle =
                        meshes.add(Mesh::from(Quad::new(Vec2::splat(endpoint.radius() * 2.0))));
                    commands.entity(*child).insert((
                        mesh_handle,
                        // convert to local reference frame
                        Transform::from_translation(
                            endpoint.endpoint(duoquad) - transform.translation,
                        )
                        // face camera
                        .with_rotation(cam_transform.rotation),
                        NoOutline,
                    ));
                    break;
                }
            }
        }
    }
}
