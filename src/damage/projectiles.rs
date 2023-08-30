// TODO: I don't know whether to use raycasts or colliders. possibly both.
// there is a 50:50 chance of a projectile physics overhaul coming?
// I need to see 1) the performance difference and 2) the accuracy of CCD first.

use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_rapier3d::prelude::*;

use crate::{
    asset::AssetLoadState,
    damage::{ContactDamage, Damage},
    physics::{CollisionGroupExt, CollisionGroupsExt},
    render::sketched::{GlobalMeshOutline, NoOutline, SketchMaterial},
    util::{distr, vectors},
};

pub struct ProjectilePlugin;

impl Plugin for ProjectilePlugin {
    fn build(&self, app: &mut App) {
        app.add_collection_to_loading_state::<_, ProjectileAssets>(AssetLoadState::Loading)
            .add_systems(
                Update,
                (
                    spawn_orb_projectiles.run_if(in_state(AssetLoadState::Success)),
                    spawn_bullet_projectiles.run_if(in_state(AssetLoadState::Success)),
                    curve_trajectories,
                    target_trajectories,
                ),
            );
    }
}

#[derive(Resource, AssetCollection)]
pub struct ProjectileAssets {
    #[asset(key = "mat.red_half_unlit")]
    pub red_half_unlit: Handle<SketchMaterial>,
    #[asset(key = "mat.red_unlit")]
    pub red_unlit: Handle<SketchMaterial>,
    #[asset(key = "mat.orange_half_unlit")]
    pub orange_half_unlit: Handle<SketchMaterial>,
    #[asset(key = "mat.orange_unlit")]
    pub orange_unlit: Handle<SketchMaterial>,
    #[asset(key = "mat.red_half_unlit")]
    pub yellow_half_unlit: Handle<SketchMaterial>,
    #[asset(key = "mat.yellow_unlit")]
    pub yellow_unlit: Handle<SketchMaterial>,
    #[asset(key = "mat.green_half_unlit")]
    pub green_half_unlit: Handle<SketchMaterial>,
    #[asset(key = "mat.green_unlit")]
    pub green_unlit: Handle<SketchMaterial>,
    #[asset(key = "mat.blue_half_unlit")]
    pub blue_half_unlit: Handle<SketchMaterial>,
    #[asset(key = "mat.blue_unlit")]
    pub blue_unlit: Handle<SketchMaterial>,
    #[asset(key = "mat.violet_half_unlit")]
    pub violet_half_unlit: Handle<SketchMaterial>,
    #[asset(key = "mat.violet_unlit")]
    pub violet_unlit: Handle<SketchMaterial>,
    #[asset(key = "mat.white_half_unlit")]
    pub white_half_unlit: Handle<SketchMaterial>,
    #[asset(key = "mat.white_unlit")]
    pub white_unlit: Handle<SketchMaterial>,
}

impl ProjectileAssets {
    fn solid_color(&self, color: ProjectileColor) -> &Handle<SketchMaterial> {
        match color {
            ProjectileColor::Red => &self.red_half_unlit,
            ProjectileColor::Orange => &self.orange_half_unlit,
            ProjectileColor::Yellow => &self.yellow_half_unlit,
            ProjectileColor::Green => &self.green_half_unlit,
            ProjectileColor::Blue => &self.blue_half_unlit,
            ProjectileColor::Violet => &self.violet_half_unlit,
            ProjectileColor::White => &self.white_half_unlit,
        }
    }

    fn half_color(&self, color: ProjectileColor) -> &Handle<SketchMaterial> {
        match color {
            ProjectileColor::Red => &self.red_unlit,
            ProjectileColor::Orange => &self.orange_unlit,
            ProjectileColor::Yellow => &self.yellow_unlit,
            ProjectileColor::Green => &self.green_unlit,
            ProjectileColor::Blue => &self.blue_unlit,
            ProjectileColor::Violet => &self.violet_unlit,
            ProjectileColor::White => &self.white_unlit,
        }
    }
}

#[derive(Component)]
pub struct BulletProjectile;

#[derive(Component)]
pub struct OrbProjectile;

#[derive(Component, Default, Clone, Copy, PartialEq, Eq)]
pub enum ProjectileColor {
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Violet,
    #[default]
    White,
}

#[derive(Bundle)]
pub struct ProjectileBundle {
    pub color: ProjectileColor,
    pub body: RigidBody,
    pub collider: Collider,
    pub collision_groups: CollisionGroups,
    pub velocity: Velocity,
    pub sensor: Sensor,
    pub active_events: ActiveEvents,
    pub gravity: GravityScale,
    pub damage: Damage,
    pub contact_damage: ContactDamage,
    pub ccd: Ccd,
    pub mass_properties: ColliderMassProperties,
    pub spatial_constraints: LockedAxes,
    pub visibility: Visibility,
    pub computed: ComputedVisibility,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
}

impl ProjectileBundle {
    pub fn player_default() -> Self {
        Self {
            collision_groups: CollisionGroups::from_group_default(Group::PLAYER_PROJECTILE),
            ..Default::default()
        }
    }

    pub fn enemy_default() -> Self {
        Self {
            collision_groups: CollisionGroups::from_group_default(Group::ENEMY_PROJECTILE),
            ..Default::default()
        }
    }
}

impl Default for ProjectileBundle {
    fn default() -> Self {
        Self {
            active_events: ActiveEvents::COLLISION_EVENTS,
            gravity: GravityScale(0.0),
            collision_groups: CollisionGroups::default(),
            body: RigidBody::Dynamic,
            collider: Collider::polyline(
                vectors::circle(Vec3::X, Vec3::Y, 16, &distr::linear).collect(),
                None,
            ),
            velocity: Velocity::default(),
            sensor: Sensor::default(),
            damage: Damage::default(),
            contact_damage: ContactDamage::default(),
            ccd: Ccd::default(),
            color: ProjectileColor::Red,
            mass_properties: ColliderMassProperties::default(),
            spatial_constraints: LockedAxes::TRANSLATION_LOCKED_Y,
            visibility: Visibility::default(),
            computed: ComputedVisibility::default(),
            transform: Transform::default(),
            global_transform: GlobalTransform::default(),
        }
    }
}

pub fn spawn_orb_projectiles(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    assets: Res<ProjectileAssets>,
    outline: Res<GlobalMeshOutline>,
    query: Query<(Entity, Option<&ProjectileColor>), Added<OrbProjectile>>,
) {
    for (e_projectile, color) in query.iter() {
        let color = color.copied().unwrap_or_default();
        commands
            .entity(e_projectile)
            .insert((
                meshes.add(Mesh::from(shape::UVSphere {
                    radius: 0.5,
                    ..Default::default()
                })),
                assets.solid_color(color).clone(),
                outline.standard.clone(),
                Collider::default(),
            ))
            .with_children(|parent| {
                parent.spawn((
                    MaterialMeshBundle {
                        mesh: meshes.add(Mesh::from(shape::UVSphere {
                            radius: 0.35,
                            ..Default::default()
                        })),
                        material: assets.half_color(color).clone(),
                        ..Default::default()
                    },
                    NoOutline,
                ));
            });
    }
}

pub fn spawn_bullet_projectiles(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    assets: Res<ProjectileAssets>,
    outline: Res<GlobalMeshOutline>,
    query: Query<(Entity, Option<&ProjectileColor>), Added<BulletProjectile>>,
) {
    for (e_projectile, color) in query.iter() {
        let color = color.copied().unwrap_or_default();
        commands.get_or_spawn(e_projectile).insert((
            meshes.add(Mesh::from(shape::UVSphere {
                radius: 0.5,
                ..Default::default()
            })),
            assets.solid_color(ProjectileColor::White).clone(),
            {
                let mut outline = outline.standard.clone();
                outline.outline.colour = Color::RED;
                //outline.outline.width = 3.0;
                outline
            },
            Collider::default(),
        ));
    }
}

#[derive(Component)]
pub struct CurvedTrajectory {
    pub rate: f32,
}

pub fn curve_trajectories(time: Res<Time>, mut query: Query<(&mut Velocity, &CurvedTrajectory)>) {
    for (mut velocity, curve) in query.iter_mut() {
        velocity.linvel =
            Quat::from_rotation_y(curve.rate * time.delta_seconds()).mul_vec3(velocity.linvel);
        velocity.angvel = Vec3::Y * curve.rate;
    }
}

#[derive(Component)]
pub struct TargettedTrajectory {
    pub entity: Entity,
}

pub fn target_trajectories(
    time: Res<Time>,
    mut query: Query<(&Transform, &mut Velocity, &TargettedTrajectory)>,
    transform_query: Query<&GlobalTransform>,
) {
    for (transform, velocity, curve) in query.iter_mut() {
        todo!();
    }
}
