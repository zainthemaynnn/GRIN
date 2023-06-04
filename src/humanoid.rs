use bevy::{gltf::Gltf, prelude::*};
use bevy_asset_loader::prelude::*;
use bevy_rapier3d::prelude::*;
use rand::{distributions::Uniform, Rng};

use crate::{
    asset::AssetLoadState,
    collider,
    collisions::{CollisionGroupExt, CollisionGroupsExt},
    damage::Dead,
    render::sketched::SketchMaterial,
    time::{CommandsExt, SetTimeParent},
};

pub const HUMANOID_HEIGHT: f32 = 2.625;

pub struct HumanoidPlugin;

impl Plugin for HumanoidPlugin {
    fn build(&self, app: &mut App) {
        app.add_collection_to_loading_state::<_, HumanoidAssets>(AssetLoadState::Loading)
            .add_systems((hand_update, dash, jump).before(PhysicsSet::StepSimulation))
            .add_systems((
                shatter_on_death.run_if(in_state(AssetLoadState::Success)),
                // since scenes render in preupdate this'll actually have to wait a frame
                // so ordering doesn't really matter
                init_shattered_fragments.run_if(in_state(AssetLoadState::Success)),
            ));
    }
}

#[derive(Resource, AssetCollection)]
pub struct HumanoidAssets {
    #[asset(key = "mesh.mbody")]
    pub mbody: Handle<Mesh>,
    #[asset(key = "mesh.mbody_shatter")]
    pub mbody_shatter: Handle<Gltf>,
    #[asset(key = "mesh.fbody")]
    pub fbody: Handle<Mesh>,
    #[asset(key = "mesh.head")]
    pub head: Handle<Mesh>,
    #[asset(key = "mesh.head_shatter")]
    pub head_shatter: Handle<Gltf>,
    #[asset(key = "mesh.hand")]
    pub hand: Handle<Mesh>,
    #[asset(key = "mat.body_gray")]
    pub body_gray: Handle<SketchMaterial>,
    #[asset(key = "mat.skin")]
    pub skin: Handle<SketchMaterial>,
}

/// Root object for standard player character.
#[derive(Component)]
pub struct Humanoid {
    pub head: Entity,
    pub lhand: Entity,
    pub rhand: Entity,
    pub dominant_hand_type: HumanoidHandType,
}

impl Humanoid {
    pub fn dominant_hand(&self) -> Entity {
        match &self.dominant_hand_type {
            HumanoidHandType::Left => self.lhand,
            HumanoidHandType::Right => self.rhand,
        }
    }
}

/// Optional head for standard player character.
#[derive(Component, Default)]
pub struct Head;

#[derive(Component, Default, Clone)]
pub struct HandOffsets {
    pub rest: Vec3,
    pub aim_single: Vec3,
}

impl HandOffsets {
    pub fn right() -> Self {
        Self {
            rest: Vec3::new(0.75, 0.875, 0.0),
            aim_single: Vec3::new(0.85, 1.25, -0.75),
        }
    }

    pub fn left() -> Self {
        Self {
            rest: Vec3::new(-0.75, 0.875, 0.0),
            aim_single: Vec3::new(-0.85, 1.25, -0.75),
        }
    }
}

/// Optional hand for standard player character.
#[derive(Component, Clone)]
pub struct Hand {
    pub offset: Vec3,
    pub velocity_influence: Vec3,
}

impl From<&HandOffsets> for Hand {
    fn from(offsets: &HandOffsets) -> Self {
        Self {
            offset: offsets.rest,
            velocity_influence: Vec3::new(0.01, 0.0, 0.1),
        }
    }
}

#[derive(Component)]
pub struct DominantHand;

#[derive(Bundle)]
pub struct HumanoidBundle<M: Material> {
    pub humanoid: Humanoid,
    pub rigid_body: RigidBody,
    pub controller: KinematicCharacterController,
    pub velocity: Velocity,
    pub collider: Collider,
    pub mesh: Handle<Mesh>,
    pub material: Handle<M>,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub visibility: Visibility,
    pub computed_visibility: ComputedVisibility,
}

impl<M: Material> From<Humanoid> for HumanoidBundle<M> {
    fn from(humanoid: Humanoid) -> Self {
        Self {
            humanoid,
            rigid_body: RigidBody::KinematicPositionBased,
            controller: KinematicCharacterController::default(),
            velocity: Velocity::default(),
            transform: Transform::default(),
            collider: Collider::default(),
            mesh: Handle::default(),
            material: Handle::default(),
            global_transform: GlobalTransform::default(),
            visibility: Visibility::default(),
            computed_visibility: ComputedVisibility::default(),
        }
    }
}

#[derive(Bundle)]
pub struct HeadBundle<M: Material> {
    pub head: Head,
    pub collider: Collider,
    pub mesh: Handle<Mesh>,
    pub velocity: Velocity,
    pub material: Handle<M>,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub visibility: Visibility,
    pub computed_visibility: ComputedVisibility,
}

impl<M: Material> Default for HeadBundle<M> {
    fn default() -> Self {
        Self {
            transform: Transform::from_xyz(0.0, 2.125, 0.0),
            head: Head::default(),
            collider: Collider::default(),
            velocity: Velocity::default(),
            mesh: Handle::default(),
            material: Handle::default(),
            global_transform: GlobalTransform::default(),
            visibility: Visibility::default(),
            computed_visibility: ComputedVisibility::default(),
        }
    }
}

#[derive(Bundle)]
pub struct HandBundle<M: Material> {
    pub hand: Hand,
    pub offsets: HandOffsets,
    pub collider: Collider,
    pub mesh: Handle<Mesh>,
    pub material: Handle<M>,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub visibility: Visibility,
    pub computed_visibility: ComputedVisibility,
}

impl<M: Material> From<HandOffsets> for HandBundle<M> {
    fn from(offsets: HandOffsets) -> Self {
        let hand = Hand::from(&offsets);
        Self {
            transform: Transform::from_translation(hand.offset),
            hand,
            offsets,
            collider: Collider::default(),
            mesh: Handle::default(),
            material: Handle::default(),
            global_transform: GlobalTransform::default(),
            visibility: Visibility::default(),
            computed_visibility: ComputedVisibility::default(),
        }
    }
}

/// A helper struct used to create humanoids.
pub struct HumanoidBuilder<'a> {
    pub body: Entity,
    pub head: Entity,
    pub lhand: Entity,
    pub rhand: Entity,
    pub assets: &'a HumanoidAssets,
    pub meshes: &'a Assets<Mesh>,
    pub race: HumanoidRace,
    pub build: HumanoidBuild,
    pub dominant: HumanoidHandType,
    pub face: Handle<SketchMaterial>,
    pub clothing: Handle<SketchMaterial>,
    pub transform: Transform,
}

pub enum HumanoidRace {
    Round,
    Square,
}

pub enum HumanoidBuild {
    Male,
    Female,
}

pub enum HumanoidHandType {
    Left,
    Right,
}

impl<'a> HumanoidBuilder<'a> {
    pub fn new(
        commands: &mut Commands,
        assets: &'a HumanoidAssets,
        meshes: &'a Assets<Mesh>,
    ) -> Self {
        let head = commands.spawn(HeadBundle::<SketchMaterial>::default()).id();
        let lhand = commands
            .spawn(HandBundle::<SketchMaterial>::from(HandOffsets::left()))
            .id();
        let rhand = commands
            .spawn(HandBundle::<SketchMaterial>::from(HandOffsets::right()))
            .id();
        let body = commands
            .spawn(HumanoidBundle::<SketchMaterial>::from(Humanoid {
                head,
                lhand,
                rhand,
                dominant_hand_type: HumanoidHandType::Right,
            }))
            .push_children(&[head, lhand, rhand])
            .id();

        Self {
            body,
            head,
            lhand,
            rhand,
            assets,
            meshes,
            race: HumanoidRace::Round,
            build: HumanoidBuild::Male,
            dominant: HumanoidHandType::Right,
            face: assets.skin.clone(),
            clothing: assets.body_gray.clone(),
            transform: Transform::default(),
        }
    }

    pub fn with_race(&mut self, race: HumanoidRace) -> &mut Self {
        self.race = race;
        self
    }

    pub fn with_build(&mut self, build: HumanoidBuild) -> &mut Self {
        self.build = build;
        self
    }

    pub fn with_dominant_hand(&mut self, dominant: HumanoidHandType) -> &mut Self {
        self.dominant = dominant;
        self
    }

    pub fn with_face(&mut self, face: Handle<SketchMaterial>) -> &mut Self {
        self.face = face;
        self
    }

    pub fn with_clothing(&mut self, clothing: Handle<SketchMaterial>) -> &mut Self {
        self.clothing = clothing;
        self
    }

    pub fn with_transform(&mut self, transform: Transform) -> &mut Self {
        self.transform = transform;
        self
    }

    #[allow(non_snake_case)]
    pub fn build(&self, commands: &mut Commands) {
        // apply torso
        let body_mesh_handle = match &self.race {
            HumanoidRace::Round => match &self.build {
                HumanoidBuild::Male => self.assets.mbody.clone(),
                HumanoidBuild::Female => self.assets.fbody.clone(),
            },
            HumanoidRace::Square => match &self.build {
                HumanoidBuild::Male => todo!(),
                HumanoidBuild::Female => todo!(),
            },
        };
        commands.get_or_spawn(self.body).insert((
            body_mesh_handle.clone(),
            self.clothing.clone(),
            collider!(self.meshes, &body_mesh_handle),
            self.transform,
        ));

        // apply head
        let head_mesh_handle = match &self.race {
            HumanoidRace::Round => self.assets.head.clone(),
            HumanoidRace::Square => todo!(),
        };
        commands.get_or_spawn(self.head).insert((
            head_mesh_handle.clone(),
            self.face.clone(),
            collider!(self.meshes, &head_mesh_handle),
        ));

        // apply hands
        let mesh_HANDle = match &self.race {
            HumanoidRace::Round => self.assets.hand.clone(),
            HumanoidRace::Square => todo!(),
        };

        commands.get_or_spawn(self.lhand).insert((
            mesh_HANDle.clone(),
            self.assets.skin.clone(),
            collider!(self.meshes, &mesh_HANDle),
        ));
        commands.get_or_spawn(self.rhand).insert((
            mesh_HANDle.clone(),
            self.assets.skin.clone(),
            collider!(self.meshes, &mesh_HANDle),
        ));

        commands
            .get_or_spawn(self.dominant_entity())
            .insert(DominantHand);
    }

    /// Returns the dominant hand for this humanoid.
    pub fn dominant_entity(&self) -> Entity {
        match &self.dominant {
            HumanoidHandType::Left => self.lhand,
            HumanoidHandType::Right => self.rhand,
        }
    }
}

fn hand_update(
    mut hands: Query<(&mut Transform, &Hand, &Parent)>,
    body: Query<&Velocity, With<Humanoid>>,
    time: Res<Time>,
) {
    for (mut transform, hand, parent) in hands.iter_mut() {
        if let Ok(velocity) = body.get(parent.get()) {
            let pos0 = transform.translation;
            let pos1 = hand.offset
                - velocity.linvel
                    * (transform.forward() * hand.velocity_influence.z
                        + transform.right() * hand.velocity_influence.x);
            let t = 5.0 * time.delta_seconds();
            transform.translation = pos0 + (pos1 - pos0) * t;
        }
    }
}

#[derive(Component)]
pub struct Shatter {
    pub material: Handle<SketchMaterial>,
    pub inherited_velocity: Vec3,
    pub speed: Uniform<f32>,
}

/// For any `Humanoid` with `Dead`, this will
/// - Shatter `Humanoid` and `Humanoid.head` into fragments.
/// - Drop any other descendants with colliders on the ground.
/// - Remove the `Handle<Mesh>`, `Handle<Material>` and `Collider`
/// from the humanoid and all descendants.
///
/// Any meshes/colliders created by this systems are copies.
/// It doesn't despawn the original entities.
pub fn shatter_on_death(
    mut commands: Commands,
    assets: Res<HumanoidAssets>,
    gltf: Res<Assets<Gltf>>,
    humanoid_query: Query<(Entity, &Humanoid, &Velocity), (With<Dead>, With<Handle<Mesh>>)>,
    shatter_query: Query<(&GlobalTransform, &Handle<SketchMaterial>)>,
    child_query: Query<(&GlobalTransform, &Handle<SketchMaterial>, &Collider)>,
    children_query: Query<&Children>,
) {
    for (entity, humanoid, velocity) in humanoid_query.iter() {
        // cause the head to explode and the body to crumble
        // there's a little bit of speed on the body
        // so that it doesn't just fall straight down
        for (e_fragment, gltf_handle, speed) in [
            (
                entity,
                &assets.mbody_shatter,
                Uniform::new_inclusive(0.0, 2.0),
            ),
            (
                humanoid.head,
                &assets.head_shatter,
                Uniform::new_inclusive(64.0, 86.0),
            ),
        ] {
            let (g_transform, material) = shatter_query.get(e_fragment).unwrap();
            commands
                .spawn((
                    Shatter {
                        material: material.clone(),
                        inherited_velocity: velocity.linvel,
                        speed,
                    },
                    SceneBundle {
                        // AVERAGE RUST PROGRAM
                        scene: gltf
                            .get(&gltf_handle)
                            .unwrap()
                            .default_scene
                            .as_ref()
                            .unwrap()
                            .clone(),
                        transform: g_transform.compute_transform(),
                        ..Default::default()
                    },
                ))
                .set_time_parent(entity);

            commands
                .entity(entity)
                .remove::<(Handle<Mesh>, Handle<SketchMaterial>, Collider)>();
        }

        // for everything else (hands, accessories), create debris copies in global space
        for entity in children_query.iter_descendants(entity) {
            if entity == humanoid.head {
                continue;
            }

            if let Ok((g_transform, material, collider)) = child_query.get(entity) {
                commands
                    .spawn((
                        material.clone(),
                        g_transform.compute_transform(),
                        collider.clone(),
                        CollisionGroups::from_group_default(Group::DEBRIS),
                        velocity.clone(),
                    ))
                    .set_time_parent(entity);
            };

            commands
                .entity(entity)
                .remove::<(Handle<Mesh>, Handle<SketchMaterial>, Collider)>();
        }
    }
}

pub fn init_shattered_fragments(
    mut commands: Commands,
    query: Query<(Entity, &Shatter, &Children)>,
    children_query: Query<&Children>,
    transform_query: Query<&GlobalTransform>,
    mesh_query: Query<(Entity, &Handle<Mesh>)>,
    meshes: Res<Assets<Mesh>>,
) {
    // this looks more complicated than it should be
    // cause `Velocity` is always in global space
    for (
        entity,
        Shatter {
            material,
            inherited_velocity,
            speed,
        },
        scene_children,
    ) in query.iter()
    {
        commands.entity(entity).remove::<Shatter>();
        let entity0 = *scene_children.first().unwrap();
        let g_transform0 = transform_query.get(entity0).unwrap();
        for child in children_query.get(entity0).unwrap() {
            let g_transform1 = transform_query.get(*child).unwrap();
            let (entity1, mesh) = mesh_query
                .get(*children_query.get(*child).unwrap().first().unwrap())
                .unwrap();
            commands.entity(entity1).insert((
                material.clone(),
                RigidBody::Dynamic,
                CollisionGroups::from_group_default(Group::DEBRIS),
                // TODO?: can I optimize these fragment colliders and still make them look good?
                collider!(meshes, mesh),
                // fly outwards from parent translation
                Velocity {
                    linvel: *inherited_velocity
                        + (g_transform1.translation() - g_transform0.translation()).normalize()
                            * rand::thread_rng().sample(speed),
                    ..Default::default()
                },
            ));
        }
    }
}
