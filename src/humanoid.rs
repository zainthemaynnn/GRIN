use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_rapier3d::prelude::*;
use rand::{distributions::Uniform, Rng};

use crate::{
    asset::AssetLoadState,
    collider,
    collisions::{CollisionGroupExt, CollisionGroupsExt},
    damage::Dead,
    render::sketched::SketchMaterial,
    time::CommandsExt,
};

pub const HUMANOID_HEIGHT: f32 = 2.625;

pub struct HumanoidPlugin;

impl Plugin for HumanoidPlugin {
    fn build(&self, app: &mut App) {
        app.add_collection_to_loading_state::<_, HumanoidAssets>(AssetLoadState::Loading)
            .add_systems(Update, dash.before(PhysicsSet::StepSimulation))
            .add_systems(
                Update,
                (
                    shatter_on_death.run_if(in_state(AssetLoadState::Success)),
                    // since scenes render in preupdate this'll actually have to wait a frame
                    // so ordering doesn't really matter
                    init_shattered_fragments.run_if(in_state(AssetLoadState::Success)),
                ),
            )
            .add_systems(
                Update,
                (
                    process_skeletons.run_if(in_state(AssetLoadState::Success)),
                    morph_moving_humanoids,
                ),
            );
    }
}

#[derive(Resource, AssetCollection)]
pub struct HumanoidAssets {
    #[asset(key = "mesh.mbody")]
    pub mbody: Handle<Mesh>,
    #[asset(key = "model.mbody_shatter")]
    pub mbody_shatter: Handle<Scene>,
    #[asset(key = "mesh.fbody")]
    pub fbody: Handle<Mesh>,
    #[asset(key = "mesh.head")]
    pub head: Handle<Mesh>,
    #[asset(key = "model.head_shatter")]
    pub head_shatter: Handle<Scene>,
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
    pub armature: Entity,
    pub dominant_hand_type: HumanoidDominantHand,
}

impl Humanoid {
    #[inline]
    pub fn dominant_hand(&self) -> Entity {
        match &self.dominant_hand_type {
            HumanoidDominantHand::Left => self.lhand,
            HumanoidDominantHand::Right => self.rhand,
        }
    }

    #[inline]
    pub fn part(&self, part: HumanoidPartType) -> Entity {
        match part {
            HumanoidPartType::Body => self.body,
            HumanoidPartType::Head => self.head,
            HumanoidPartType::LeftHand => self.lhand,
            HumanoidPartType::RightHand => self.rhand,
            HumanoidPartType::Armature => self.armature,
        }
    }

    #[inline]
    pub fn parts<'a, I>(&'a self, parts: I) -> impl Iterator<Item = Entity> + 'a
    where
        I: IntoIterator<Item = HumanoidPartType>,
        I::IntoIter: 'a,
    {
        parts.into_iter().map(|p| self.part(p))
    }
}

#[derive(Component, Default)]
pub struct Head;

#[derive(Component, Default)]
pub struct Body;

#[derive(Component, Default)]
pub struct Hand;

#[derive(Component)]
pub struct DominantHand;

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
    pub velocity: Velocity,
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
            rigid_body: RigidBody::KinematicPositionBased, // SCREW YOU
            controller: KinematicCharacterController::default(),
            velocity: Velocity::default(),
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
#[component(storage = "SparseSet")]
pub struct Shattered;

#[derive(Component)]
#[component(storage = "SparseSet")]
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
    humanoid_query: Query<(Entity, &Humanoid, &Velocity), (With<Dead>, Without<Shattered>)>,
    shatter_query: Query<(&GlobalTransform, &Handle<SketchMaterial>)>,
    child_query: Query<(&GlobalTransform, &Collider)>,
    mesh_query: Query<(Entity, &Handle<Mesh>, &Handle<SketchMaterial>)>,
    children_query: Query<&Children>,
) {
    for (e_humanoid, humanoid, velocity) in humanoid_query.iter() {
        commands.entity(e_humanoid).insert(Shattered);

        // cause the head to explode and the body to crumble
        // there's a little bit of speed on the body
        // so that it doesn't just fall straight down
        for (e_fragment, scene, speed) in [
            (
                children_query.get(humanoid.body).unwrap()[0],
                &assets.mbody_shatter,
                Uniform::new_inclusive(0.0, 2.0),
            ),
            (
                children_query.get(humanoid.head).unwrap()[0],
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
                        scene: scene.clone(),
                        transform: g_transform.compute_transform(),
                        ..Default::default()
                    },
                ))
                .set_time_parent(e_humanoid);

            commands
                .entity(e_fragment)
                .remove::<(Handle<Mesh>, Handle<SketchMaterial>, Collider)>();
        }

        // for everything else (hands, accessories), create debris copies in global space.
        // TODO?: this only works on things with colliders. perhaps this is worth a modification.
        for e_child in children_query.iter_descendants(e_humanoid) {
            if e_child == humanoid.head || e_child == humanoid.body {
                continue;
            }

            if let Ok((g_transform, collider)) = child_query.get(e_child) {
                // if the mesh is on the collider, use this entity
                // if the mesh is a child, use that entity
                if let Ok((e_mesh, mesh, material)) = mesh_query.get(e_child).or_else(|_| {
                    children_query
                        .get(e_child)
                        .and_then(|c| mesh_query.get(c[0]))
                }) {
                    commands.entity(e_child).remove::<Collider>();
                    commands
                        .entity(e_mesh)
                        .remove::<(Handle<Mesh>, Handle<SketchMaterial>)>();
                    commands
                        .spawn((
                            MaterialMeshBundle {
                                mesh: mesh.clone(),
                                material: material.clone(),
                                transform: g_transform.compute_transform(),
                                ..Default::default()
                            },
                            RigidBody::Dynamic,
                            collider.clone(),
                            CollisionGroups::from_group_default(Group::DEBRIS),
                            velocity.clone(),
                        ))
                        .set_time_parent(e_humanoid);
                }
            };
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
    pub armature: Option<Entity>,
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
            armature: self
                .armature
                .ok_or(HumanoidLoadError::Missing(HumanoidPartType::Armature))?,
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
    Armature,
}

impl HumanoidPartType {
    pub const HITBOX: [Self; 2] = [Self::Head, Self::Body];
    pub const HANDS: [Self; 2] = [Self::LeftHand, Self::RightHand];
    pub const ALL: [Self; 4] = [Self::Body, Self::Head, Self::LeftHand, Self::RightHand];

    /// The name of the corresponding mesh in GLTF file.
    pub fn node_id(&self) -> &str {
        match self {
            HumanoidPartType::Body => "Body",
            HumanoidPartType::Head => "Head",
            HumanoidPartType::LeftHand => "LeftHand",
            HumanoidPartType::RightHand => "RightHand",
            HumanoidPartType::Armature => "Armature",
        }
    }

    pub fn from_node_id(id: &str) -> Option<Self> {
        Some(match id {
            "Body" => HumanoidPartType::Body,
            "Head" => HumanoidPartType::Head,
            "LeftHand" => HumanoidPartType::LeftHand,
            "RightHand" => HumanoidPartType::RightHand,
            "Armature" => HumanoidPartType::Armature,
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
                    commands.entity(e_node).insert((
                        Head,
                        Collider::ball(0.5),
                        Velocity::default(),
                    ));

                    let e_mesh = children_query.get(e_node).unwrap()[0];
                    commands.entity(e_mesh).insert((
                        face.clone(),
                        match race {
                            HumanoidRace::Round => assets.head.clone(),
                            HumanoidRace::Square => todo!(),
                        },
                    ));
                }
                HumanoidPartType::Body => {
                    builder.body = Some(e_node);
                    commands
                        .entity(e_node)
                        .insert((Body, Collider::capsule_y(0.375, 0.5)));

                    let e_mesh = children_query.get(e_node).unwrap()[0];
                    commands.entity(e_mesh).insert((
                        clothing.clone(),
                        match race {
                            HumanoidRace::Round => match build {
                                HumanoidBuild::Male => assets.mbody.clone(),
                                HumanoidBuild::Female => assets.mbody.clone(),
                            },
                            HumanoidRace::Square => todo!(),
                        },
                    ));
                }
                HumanoidPartType::LeftHand => {
                    builder.lhand = Some(e_node);
                    commands.entity(e_node).insert((Hand, Collider::ball(0.15)));

                    let e_mesh = children_query.get(e_node).unwrap()[0];
                    commands
                        .entity(e_mesh)
                        .insert((assets.skin.clone(), hand_mesh.clone()));
                }
                HumanoidPartType::RightHand => {
                    builder.rhand = Some(e_node);
                    commands.entity(e_node).insert((Hand, Collider::ball(0.15)));

                    let e_mesh = children_query.get(e_node).unwrap()[0];
                    commands
                        .entity(e_mesh)
                        .insert((assets.skin.clone(), hand_mesh.clone()));
                }
                HumanoidPartType::Armature => {
                    builder.armature = Some(e_node);
                }
            }
        }

        match builder.build() {
            Ok(humanoid) => {
                commands.entity(e_skeleton).insert((
                    humanoid,
                    match race {
                        HumanoidRace::Round => Collider::capsule(
                            Vec3::new(0.0, 0.5, 0.0),
                            Vec3::new(0.0, 2.125, 0.0),
                            0.5,
                        ),
                        HumanoidRace::Square => todo!(),
                    },
                ));
            }
            Err(e) => error!("{}", e),
        }
    }
}

/// Correspond to shape keys on the head and torso.
///
/// As far as I know, keys with the same name use the same weights in Unreal.
/// This would be nice for bevy. I haven't complained about it yet.
#[repr(usize)]
pub enum HumanoidMorph {
    Front = 0,
    Back = 1,
    Left = 2,
    Right = 3,
}

/// Scales the torso's morph weights relative to the humanoid's speed in one direction.
///
/// Note that at a weight of `1.0` the humanoid's head will have translated about `1.0` units.
/// This is the hard cap, and should probably never happen, unless they're going at an extreme speed
/// for whatever reason.
pub const SPEED_MORPH_CONSTANT: f32 = 0.1;

/// Maximum rate of change for morph weights.
pub const MORPH_EASE_CONSTANT: f32 = 4.0;

/// Morphs humanoids according to their movement.
///
/// This'll make them "slant" towards their effective direction of movement, which increases
/// in intensity based on their speed. The slant is based on
/// `KinematicCharacterControllerOutput.effective_translation`, so movement from other causes
/// other than the character controller have no effect on morphing.

// god, I could have made a 2d game and just ran a spritesheet
pub fn morph_moving_humanoids(
    time: Res<Time>,
    humanoid_query: Query<(&Humanoid, &Transform, &KinematicCharacterControllerOutput)>,
    mut transform_query: Query<&mut Transform, Without<Humanoid>>,
    mut morph_query: Query<&mut MorphWeights>,
) {
    for (humanoid, transform, movement) in humanoid_query.iter() {
        let speed = movement.effective_translation.length() / time.delta_seconds();
        let translation_norm = movement.desired_translation.normalize_or_zero();

        // this should ease into the target weight smoothly
        let weight = |w0: f32, w1: f32| {
            let w1 = w1 * speed * SPEED_MORPH_CONSTANT;
            let d = w1 - w0;
            (w0 + d.abs().min(MORPH_EASE_CONSTANT * time.delta_seconds()) * d.signum())
                .clamp(-1.0, 1.0)
        };

        let Ok(mut morph_weights) = morph_query.get_mut(humanoid.body) else {
            error!("Missing morph targets on humanoid part: {:?}.", humanoid.body);
            continue;
        };

        let weights = morph_weights.weights_mut();

        let weight_x = weight(
            weights[HumanoidMorph::Right as usize] - weights[HumanoidMorph::Left as usize],
            transform.right().dot(translation_norm),
        );
        let weight_z = weight(
            weights[HumanoidMorph::Front as usize] - weights[HumanoidMorph::Back as usize],
            transform.forward().dot(translation_norm),
        );

        // weights need to be converted to [0.0, 1.0]
        // the sign is just used to determine the correct key

        // front/back weights
        if weight_z > 0.0 {
            weights[HumanoidMorph::Front as usize] = weight_z;
            weights[HumanoidMorph::Back as usize] = 0.0;
        } else {
            weights[HumanoidMorph::Front as usize] = 0.0;
            weights[HumanoidMorph::Back as usize] = -weight_z;
        }

        // right/left weights
        if weight_z > 0.0 {
            weights[HumanoidMorph::Right as usize] = weight_x;
            weights[HumanoidMorph::Left as usize] = 0.0;
        } else {
            weights[HumanoidMorph::Right as usize] = 0.0;
            weights[HumanoidMorph::Left as usize] = -weight_x;
        }

        let mut armature_transform = transform_query.get_mut(humanoid.armature).unwrap();
        armature_transform.translation =
            Vec3::new(weight_x, armature_transform.translation.y, weight_z);
    }
}
