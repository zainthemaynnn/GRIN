pub mod camera;
pub mod eightball;

use std::array::IntoIter;
use std::marker::PhantomData;

use crate::asset::AssetLoadState;
use crate::damage::{DamageBuffer, Health, HealthBundle};
use crate::humanoid::{
    Dash, Humanoid, HumanoidPartType, HumanoidRace, HUMANOID_HEIGHT, HUMANOID_RADIUS,
};
use crate::render::gopro::{add_gopro, GoProSettings};
use crate::render::sketched::SketchMaterial;
use crate::sound::Ears;

use crate::item::{Equipped, Item};
use crate::physics::{CollisionGroupExt, CollisionGroupsExt, PhysicsTime};
use crate::render::RenderLayer;
use crate::util::event::Spawnable;
use crate::util::vectors::Vec3Ext;
use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy_asset_loader::prelude::*;
use bevy_rapier3d::prelude::*;

use self::camera::{CameraAlignment, LookInfo, PlayerCamera, PlayerCameraPlugin};
use self::eightball::{EightBall, EightBallPlugin};

pub const CHARACTER_WALKSPEED: f32 = 24.0;

pub struct CharacterPlugin;

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum CharacterSet {
    Spawn,
    Init,
    Load,
}

#[derive(Default, Event)]
pub struct AvatarLoadEvent;

pub fn set_avatar_load_state(
    load_events: EventReader<AvatarLoadEvent>,
    mut next_state: ResMut<NextState<AvatarLoadState>>,
) {
    if !load_events.is_empty() {
        info!("character loaded; advancing state");
        next_state.set(AvatarLoadState::Loaded);
    }
}

impl Plugin for CharacterPlugin {
    fn build(&self, app: &mut App) {
        app.add_state::<AvatarLoadState>()
            .add_event::<AvatarLoadEvent>()
            .add_plugins((PlayerCameraPlugin, EightBallPlugin))
            .add_collection_to_loading_state::<_, AvatarAssets>(AssetLoadState::Loading)
            .configure_sets(
                Update,
                (
                    CharacterSet::Spawn.run_if(in_state(AssetLoadState::Success)),
                    CharacterSet::Init.run_if(in_state(AssetLoadState::Success)),
                    CharacterSet::Load.run_if(
                        in_state(AssetLoadState::Success)
                            .and_then(in_state(AvatarLoadState::NotLoaded)),
                    ),
                ),
            )
            .add_systems(
                OnEnter(AssetLoadState::Success),
                <EightBall as Character>::spawn.before(CharacterSet::Spawn),
            )
            .add_systems(Update, init_character_model.in_set(CharacterSet::Init))
            .add_systems(
                Update,
                (apply_deferred, set_avatar_load_state)
                    .chain()
                    .in_set(CharacterSet::Load),
            )
            .add_systems(OnEnter(AvatarLoadState::Loaded), insert_status_viewport)
            .add_systems(
                Update,
                (input_walk, input_dash.before(crate::humanoid::dash))
                    .run_if(in_state(AvatarLoadState::Loaded)),
            );
    }
}


    }
}

pub trait Character: Component + Sized + Spawnable {
    type StartItem: Item;
}

#[derive(Resource, AssetCollection)]
pub struct AvatarAssets {
    #[asset(key = "mesh.pizza_shades")]
    pub pizza_shades: Handle<Mesh>,
    #[asset(key = "mat.shades")]
    pub matte_shades: Handle<SketchMaterial>,
    #[asset(key = "mat.smirk")]
    pub face_smirk: Handle<SketchMaterial>,
    #[asset(key = "mat.meh")]
    pub face_meh: Handle<SketchMaterial>,
}

#[derive(Component, Default)]
pub struct PlayerCharacter;

#[derive(Component, Copy, Clone, Default)]
pub struct Player;

pub fn init_character_model(
    mut commands: Commands,
    mut player_query: Query<
        (Entity, &Humanoid, &HumanoidRace),
        (With<PlayerCharacter>, Without<Player>),
    >,
    children_query: Query<&Children>,
) {
    let Ok((e_humanoid, humanoid, race)) = player_query.get_single_mut() else {
        return;
    };

    let render_layers =
        RenderLayers::from_layers(&[RenderLayer::STANDARD as u8, RenderLayer::AVATAR as u8]);

    commands.entity(e_humanoid).insert((
        Player,
        Equipped::default(),
        HealthBundle {
            health: Health(100.0),
            ..Default::default()
        },
        RigidBody::KinematicPositionBased,
        Velocity::default(),
        KinematicCharacterController {
            custom_shape: Some((
                match race {
                    HumanoidRace::Round => Collider::capsule_y(
                        HUMANOID_HEIGHT / 2.0 - HUMANOID_RADIUS,
                        HUMANOID_RADIUS,
                    ),
                    HumanoidRace::Square => {
                        Collider::cuboid(HUMANOID_RADIUS, HUMANOID_HEIGHT / 2.0, HUMANOID_RADIUS)
                    }
                },
                Vec3::Y * HUMANOID_HEIGHT / 2.0,
                Quat::default(),
            )),
            filter_groups: Some({
                let mut groups = CollisionGroups::from_group_default(Group::PLAYER);
                // body parts can still hit projectiles, but the controller shouldn't detect them at all
                groups.filters -= Group::ENEMY_PROJECTILE;
                groups
            }),
            ..Default::default()
        },
    ));

    commands.entity(humanoid.head).insert(Ears(0.5));

    for e_part in humanoid.parts(HumanoidPartType::ALL) {
        commands.entity(e_part).insert(Player);
        commands
            .entity(children_query.get(e_part).unwrap()[0])
            .insert(render_layers);
    }

    for e_part in humanoid.parts(HumanoidPartType::HITBOX) {
        commands.entity(e_part).insert((
            DamageBuffer::default(),
            CollisionGroups::from_group_default(Group::PLAYER),
        ));
    }

    for e_part in humanoid.parts(HumanoidPartType::HANDS) {
        commands
            .entity(e_part)
            .insert(CollisionGroups::from_group_default(Group::NONE));
    }
}

pub fn insert_status_viewport(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    player_query: Query<Entity, With<PlayerCharacter>>,
) {
    let e_avatar = player_query.single();

    let image = add_gopro(
        &mut commands,
        &mut images,
        GoProSettings {
            entity: e_avatar,
            transform: Transform::from_translation(Vec3::new(0.0, 2.125, -2.0))
                .looking_to(Vec3::Z, Vec3::Y),
            size: UVec2::splat(240),
            render_layers: RenderLayers::layer(RenderLayer::AVATAR as u8),
        },
    );

    commands.spawn(ImageBundle {
        image: image.into(),
        style: Style {
            position_type: PositionType::Absolute,
            bottom: Val::Percent(0.0),
            left: Val::Percent(0.0),
            width: Val::Px(240.0),
            height: Val::Px(240.0),
            ..Default::default()
        },
        ..Default::default()
    });
}

pub fn input_walk(
    input: Res<Input<KeyCode>>,
    camera_query: Query<(&GlobalTransform, &PlayerCamera), Without<PlayerCharacter>>,
    mut character: Query<
        (&mut KinematicCharacterController, &mut Transform),
        (With<PlayerCharacter>, Without<Dash>),
    >,
    look_info: Res<LookInfo>,
    time: Res<PhysicsTime>,
) {
    if let Ok((mut char_controller, mut transform)) = character.get_single_mut() {
        let (cam_transform, camera) = camera_query.single();

        let mut movement = Vec3::ZERO;
        if input.pressed(KeyCode::W) {
            movement += cam_transform.forward().xz_flat();
        }
        if input.pressed(KeyCode::A) {
            movement += cam_transform.left().xz_flat();
        }
        if input.pressed(KeyCode::S) {
            movement += cam_transform.back().xz_flat();
        }
        if input.pressed(KeyCode::D) {
            movement += cam_transform.right().xz_flat();
        }

        char_controller.translation = Some(
            char_controller.translation.unwrap_or_default()
                + movement.normalize_or_zero() * CHARACTER_WALKSPEED * time.0.delta_seconds(),
        );
        match camera.alignment {
            CameraAlignment::FortyFive => {
                if let Some(target) =
                    look_info.vertical_target_point(transform.translation, transform.up())
                {
                    // normalizing the Y here, just want it to follow the XZ coords
                    let target = Vec3::new(target.x, transform.translation.y, target.z);
                    transform.look_at(target, Vec3::Y);
                }
            }
            CameraAlignment::Shooter => transform.rotation = Quat::from_rotation_y(look_info.yaw),
        }
    }
}

pub fn input_dash(
    mut commands: Commands,
    character: Query<(Entity, &Velocity), With<PlayerCharacter>>,
    input: Res<Input<KeyCode>>,
    mut cooldown: Local<f32>,
    time: Res<Time>,
) {
    if *cooldown <= 0.0 {
        if input.pressed(KeyCode::ShiftLeft) {
            let (entity, velocity) = character.single();
            commands.entity(entity).insert(Dash {
                velocity: velocity.linvel * 2.0,
                time: 0.2,
            });
        }
        *cooldown = 0.4;
    } else {
        *cooldown -= time.delta_seconds();
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum AvatarLoadState {
    #[default]
    NotLoaded,
    Loaded,
}

impl States for AvatarLoadState {
    type Iter = IntoIter<AvatarLoadState, 2>;

    fn variants() -> Self::Iter {
        [Self::NotLoaded, Self::Loaded].into_iter()
    }
}
