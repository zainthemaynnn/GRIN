use bevy::{gltf::Gltf, prelude::*};
use bevy_asset_loader::prelude::*;
use bevy_rapier3d::prelude::*;
use rand::{distributions::Uniform, Rng};

use crate::{
    asset::AssetLoadState,
    collider,
    collisions::{ColliderRef, CollisionGroupExt, CollisionGroupsExt},
    damage::Dead,
    render::sketched::SketchMaterial,
    time::CommandsExt,
};

pub const HUMANOID_HEIGHT: f32 = 2.625;

pub struct HumanoidPlugin;

impl Plugin for HumanoidPlugin {
    fn build(&self, app: &mut App) {
        app.add_collection_to_loading_state::<_, HumanoidAssets>(AssetLoadState::Loading)
            .add_system(dash.before(PhysicsSet::StepSimulation))
            .add_systems((
                shatter_on_death.run_if(in_state(AssetLoadState::Success)),
                // since scenes render in preupdate this'll actually have to wait a frame
                // so ordering doesn't really matter
                init_shattered_fragments.run_if(in_state(AssetLoadState::Success)),
            ))
            .add_system(process_skeletons.in_set(OnUpdate(AssetLoadState::Success)));
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
    #[asset(key = "rig.humanoid_skeleton")]
    pub skeleton: Handle<Scene>,
}

/// Root object for humanoid rigs.
#[derive(Component, Debug)]
pub struct Humanoid {
    pub body: Entity,
    pub head: Entity,
    pub lhand: Entity,
    pub rhand: Entity,
    pub dominant_hand_type: HumanoidDominantHand,
}

impl Humanoid {
    pub fn dominant_hand(&self) -> Entity {
        match &self.dominant_hand_type {
            HumanoidDominantHand::Left => self.lhand,
            HumanoidDominantHand::Right => self.rhand,
        }
    }
}

#[derive(Component, Default)]
pub struct Head;

#[derive(Bundle, Default)]
pub struct HeadBundle<M: Material> {
    pub head: Head,
    pub mesh: Handle<Mesh>,
    pub material: Handle<M>,
}

#[derive(Component, Default)]
pub struct Body;

#[derive(Bundle, Default)]
pub struct BodyBundle<M: Material> {
    pub body: Body,
    pub mesh: Handle<Mesh>,
    pub material: Handle<M>,
}

#[derive(Component, Default)]
pub struct Hand;

#[derive(Component)]
pub struct DominantHand;

#[derive(Bundle, Default)]
pub struct HandBundle<M: Material> {
    pub hand: Hand,
    pub mesh: Handle<Mesh>,
    pub material: Handle<M>,
    pub velocity: Velocity,
}

#[derive(Component, Default, Clone, Copy, Eq, PartialEq)]
pub enum HumanoidRace {
    #[default]
    Round,
    Square,
}

#[derive(Component, Clone, Copy, Eq, PartialEq)]
pub enum HumanoidBuild {
    Male,
    Female,
}

impl HumanoidBuild {
    pub fn random(rng: &mut impl Rng) -> Self {
        match rng.gen_bool(0.5) {
            true => HumanoidBuild::Male,
            false => HumanoidBuild::Female,
        }
    }
}

impl Default for HumanoidBuild {
    fn default() -> Self {
        Self::random(&mut rand::thread_rng())
    }
}

#[derive(Component, Debug, Clone, Copy, Eq, PartialEq)]
pub enum HumanoidDominantHand {
    Left,
    Right,
}

impl HumanoidDominantHand {
    pub const LEFT_HANDED_PROBABILITY: f64 = 0.1;

    pub fn random(rng: &mut impl Rng) -> Self {
        match rng.gen_bool(Self::LEFT_HANDED_PROBABILITY) {
            true => HumanoidDominantHand::Left,
            false => HumanoidDominantHand::Right,
        }
    }
}

impl Default for HumanoidDominantHand {
    fn default() -> Self {
        Self::random(&mut rand::thread_rng())
    }
}

#[derive(Component, Default, Clone, Eq, PartialEq)]
pub struct HumanoidFace(pub Option<Handle<SketchMaterial>>);

impl From<Handle<SketchMaterial>> for HumanoidFace {
    fn from(value: Handle<SketchMaterial>) -> Self {
        Self(Some(value))
    }
}

#[derive(Component, Default, Clone, Eq, PartialEq)]
pub struct HumanoidClothing(pub Option<Handle<SketchMaterial>>);

impl From<Handle<SketchMaterial>> for HumanoidClothing {
    fn from(value: Handle<SketchMaterial>) -> Self {
        Self(Some(value))
    }
}

#[derive(Bundle, Clone)]
pub struct HumanoidBundle {
    pub skeleton_gltf: Handle<Scene>,
    pub skeleton: Skeleton,
    pub face: HumanoidFace,
    pub clothing: HumanoidClothing,
    pub race: HumanoidRace,
    pub build: HumanoidBuild,
    pub dominant_hand: HumanoidDominantHand,
    pub rigid_body: RigidBody,
    pub controller: KinematicCharacterController,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub visibility: Visibility,
    pub computed_visibility: ComputedVisibility,
}

impl Default for HumanoidBundle {
    fn default() -> Self {
        Self {
            skeleton_gltf: Handle::default(),
            skeleton: Skeleton::default(),
            face: HumanoidFace::default(),
            clothing: HumanoidClothing::default(),
            race: HumanoidRace::default(),
            build: HumanoidBuild::default(),
            dominant_hand: HumanoidDominantHand::default(),
            rigid_body: RigidBody::KinematicPositionBased,
            controller: KinematicCharacterController::default(),
            transform: Transform::default(),
            global_transform: GlobalTransform::default(),
            visibility: Visibility::default(),
            computed_visibility: ComputedVisibility::default(),
        }
    }
}

#[derive(Component)]
pub struct Dash {
    pub velocity: Vec3,
    pub time: f32,
}

// most simple 3D dash mechanic in history???
pub fn dash(
    mut commands: Commands,
    mut humanoid_query: Query<
        (Entity, &mut KinematicCharacterController, &mut Dash),
        With<Humanoid>,
    >,
    time: Res<Time>,
) {
    for (entity, mut char_controller, mut dash) in humanoid_query.iter_mut() {
        dash.time -= time.delta_seconds();
        if dash.time > 0.0 {
            let mut t = char_controller.translation.unwrap_or_default();
            t += dash.velocity * time.delta_seconds();
            char_controller.translation = Some(t);
        } else {
            commands.get_or_spawn(entity).remove::<Dash>();
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
                .entity(e_fragment)
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

#[derive(Component, Default, Clone, Copy)]
pub struct Skeleton;

#[derive(Default)]
pub struct HumanoidBuilder {
    pub body: Option<Entity>,
    pub head: Option<Entity>,
    pub lhand: Option<Entity>,
    pub rhand: Option<Entity>,
    pub dominant_hand_type: Option<HumanoidDominantHand>,
}

impl HumanoidBuilder {
    fn build(self) -> Result<Humanoid, HumanoidLoadError> {
        Ok(Humanoid {
            body: self
                .body
                .ok_or(HumanoidLoadError::Missing(HumanoidPartType::Body))?,
            head: self
                .head
                .ok_or(HumanoidLoadError::Missing(HumanoidPartType::Head))?,
            lhand: self
                .lhand
                .ok_or(HumanoidLoadError::Missing(HumanoidPartType::LeftHand))?,
            rhand: self
                .rhand
                .ok_or(HumanoidLoadError::Missing(HumanoidPartType::RightHand))?,
            dominant_hand_type: self
                .dominant_hand_type
                .ok_or(HumanoidLoadError::NoDominant)?,
        })
    }
}

pub enum HumanoidPartType {
    Body,
    Head,
    LeftHand,
    RightHand,
}

impl HumanoidPartType {
    /// The name of the corresponding mesh in GLTF file.
    pub fn node_id(&self) -> &str {
        match self {
            HumanoidPartType::Body => "BodyMesh",
            HumanoidPartType::Head => "HeadMesh",
            HumanoidPartType::LeftHand => "LeftHandMesh",
            HumanoidPartType::RightHand => "RightHandMesh",
        }
    }

    pub fn from_node_id(id: &str) -> Option<Self> {
        Some(match id {
            "BodyMesh" => HumanoidPartType::Body,
            "HeadMesh" => HumanoidPartType::Head,
            "LeftHandMesh" => HumanoidPartType::LeftHand,
            "RightHandMesh" => HumanoidPartType::RightHand,
            _ => None?,
        })
    }
}

pub enum HumanoidLoadError {
    Missing(HumanoidPartType),
    NoDominant,
}

impl std::fmt::Display for HumanoidLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HumanoidLoadError::Missing(part) => f.write_fmt(format_args!(
                "Missing node in GLTF humanoid: `{}`.",
                part.node_id()
            )),
            HumanoidLoadError::NoDominant => {
                f.write_str("Missing a dominant hand for this humanoid.")
            }
        }
    }
}

/// Initializes humanoid skeletons.
///
/// - Inserts `Humanoid`.
/// - Updates meshes and textures to be in line with cosmetic components.
/// - Assigns dominant hand.
/// - Inserts colliders.
pub fn process_skeletons(
    mut commands: Commands,
    assets: Res<HumanoidAssets>,
    skeleton_query: Query<
        (
            Entity,
            &HumanoidRace,
            &HumanoidBuild,
            &HumanoidDominantHand,
            &HumanoidFace,
            &HumanoidClothing,
        ),
        (With<Skeleton>, With<Children>, Without<Humanoid>),
    >,
    children_query: Query<&Children>,
    name_query: Query<&Name>,
) {
    for (e_skeleton, race, build, dominant_hand, face, clothing) in skeleton_query.iter() {
        let mut builder = HumanoidBuilder::default();
        builder.dominant_hand_type = Some(dominant_hand.clone());

        let face = face.0.clone().unwrap_or(assets.skin.clone());
        let clothing = clothing.0.clone().unwrap_or(assets.body_gray.clone());
        let hand_mesh = match race {
            HumanoidRace::Round => assets.hand.clone(),
            HumanoidRace::Square => todo!(),
        };

        // technically don't need to clone stuff in here
        // but I'm not feeling like writing unsafe today
        for e_node in children_query.iter_descendants(e_skeleton) {
            let Ok(name) = name_query.get(e_node) else {
                continue;
            };
            let Some(part_type) = HumanoidPartType::from_node_id(name.as_str()) else {
                continue;
            };

            match part_type {
                HumanoidPartType::Head => {
                    builder.head = Some(e_node);
                    let mesh = match race {
                        HumanoidRace::Round => assets.head.clone(),
                        HumanoidRace::Square => todo!(),
                    };
                    commands.entity(e_node).insert(HeadBundle {
                        mesh,
                        material: face.clone(),
                        ..Default::default()
                    });
                    commands
                        .spawn((
                            Collider::ball(1.0),
                            ColliderRef(e_node),
                            CollisionGroups::default(),
                        ))
                        .set_parent(e_skeleton);
                }
                HumanoidPartType::Body => {
                    builder.body = Some(e_node);
                    let mesh = match race {
                        HumanoidRace::Round => match build {
                            HumanoidBuild::Male => assets.mbody.clone(),
                            HumanoidBuild::Female => assets.fbody.clone(),
                        },
                        HumanoidRace::Square => todo!(),
                    };
                    commands.entity(e_node).insert(BodyBundle {
                        mesh,
                        material: clothing.clone(),
                        ..Default::default()
                    });
                    commands
                        .spawn((
                            Collider::capsule_y(0.375, 0.5),
                            ColliderRef(e_node),
                            CollisionGroups::default(),
                        ))
                        .set_parent(e_skeleton);
                }
                HumanoidPartType::LeftHand => {
                    builder.lhand = Some(e_node);
                    commands.entity(e_node).insert(HandBundle {
                        mesh: hand_mesh.clone(),
                        material: assets.skin.clone(),
                        ..Default::default()
                    });
                    commands
                        .spawn((
                            Collider::ball(0.15),
                            ColliderRef(e_node),
                            CollisionGroups::default(),
                        ))
                        .set_parent(e_skeleton);
                }
                HumanoidPartType::RightHand => {
                    builder.rhand = Some(e_node);
                    commands.entity(e_node).insert(HandBundle {
                        mesh: hand_mesh.clone(),
                        material: assets.skin.clone(),
                        ..Default::default()
                    });
                    commands
                        .spawn((
                            Collider::ball(0.15),
                            ColliderRef(e_node),
                            CollisionGroups::default(),
                        ))
                        .set_parent(e_skeleton);
                }
            }
        }

        match builder.build() {
            Ok(humanoid) => {
                commands
                    .entity(e_skeleton)
                    .insert((humanoid, Collider::default()));
            }
            Err(e) => error!("{}", e),
        }
    }
}
