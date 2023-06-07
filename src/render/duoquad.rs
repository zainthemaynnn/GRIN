// I'm never going to stop accidentally typing "dua" instead of "duo."
// is this just a me thing or what?
use std::f32::consts::TAU;

use bevy::prelude::{shape::Quad, *};

use crate::{
    character::{camera::PlayerCamera, AvatarLoadState},
    render::sketched::{NoOutline, SketchMaterial},
};

pub struct DuoQuadPlugin;

impl Plugin for DuoQuadPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems((
            init_duoquads,
            render_duoquads,
            inherit_materials,
        ));
    }
}

/// [One of these good old guys.](https://answers.unity.com/storage/temp/61619-laser-bolt-instructions.jpg)
///
/// Do they have an official name? I dunno. I made one up.
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
    pub computed: ComputedVisibility,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
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
        *transform = Transform::from_translation(mdpt).with_rotation(Quat::from_mat3(
            &Mat3::from_cols(axis.normalize(), perp.0, perp.1),
        ));

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

