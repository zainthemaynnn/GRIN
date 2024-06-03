pub mod kit;
pub mod util;

use std::marker::PhantomData;

use bevy::{app::PluginGroupBuilder, prelude::*, render::view::RenderLayers};
use bevy_rapier3d::prelude::*;
use grin_asset::AssetLoadState;
use grin_damage::health::{Health, HealthBundle};
use grin_input::camera::{CameraAlignment, LookInfo, PlayerCamera, PlayerCameraPlugin};
use grin_item::{equip::Equipped, mechanics::util::InputHandler, spawn::ItemSpawnEvent};
use grin_physics::{CollisionGroupExt, CollisionGroupsExt, PhysicsTime};
use grin_render::{
    gopro::{add_gopro, GoProSettings},
    RenderLayer,
};
use grin_rig::humanoid::{Dash, Humanoid, HumanoidRace, HUMANOID_HEIGHT, HUMANOID_RADIUS};
use grin_util::{event::Spawnable, vectors::Vec3Ext};

use kit::{grin::GrinPlugin, smirk::SmirkPlugin};

pub const CHARACTER_WALKSPEED: f32 = 6.0;

pub struct MasterCharacterPlugin;

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum CharacterSet {
    Spawn,
    Init,
    Load,
}

impl Plugin for MasterCharacterPlugin {
    fn build(&self, app: &mut App) {
        app.add_state::<AvatarLoadState>()
            .add_plugins(PlayerCameraPlugin::<PlayerCharacter>::default())
            .configure_sets(
                Update,
                (
                    CharacterSet::Spawn.run_if(in_state(AssetLoadState::Success)),
                    CharacterSet::Init.run_if(
                        in_state(AssetLoadState::Success)
                            .and_then(in_state(AvatarLoadState::NotLoaded)),
                    ),
                    CharacterSet::Load.run_if(
                        in_state(AssetLoadState::Success)
                            .and_then(in_state(AvatarLoadState::NotLoaded)),
                    ),
                ),
            )
            .add_systems(PostUpdate, init_character_model.in_set(CharacterSet::Init))
            .add_systems(
                Update,
                apply_deferred
                    .after(CharacterSet::Init)
                    .before(CharacterSet::Load),
            )
            .add_systems(
                Update,
                set_avatar_load_state_on_humanoid_load.in_set(CharacterSet::Load),
            )
            .add_systems(OnEnter(AvatarLoadState::Loaded), insert_status_viewport)
            .add_systems(
                Update,
                (
                    input_walk,
                    input_dash.before(grin_rig::humanoid::dash),
                    enable_input_for_player_items,
                )
                    .run_if(in_state(AvatarLoadState::Loaded)),
            );
    }
}

pub struct GenericHumanoidCharacterPlugin<T: Character> {
    phantom_data: PhantomData<T>,
}

impl<T: Character> Default for GenericHumanoidCharacterPlugin<T> {
    fn default() -> Self {
        Self {
            phantom_data: PhantomData::default(),
        }
    }
}

impl<T: Character> Plugin for GenericHumanoidCharacterPlugin<T> {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AvatarLoadState::Loaded),
            equip_spawn_item_on_humanoid_load::<T>.in_set(CharacterSet::Load),
        );
    }
}

pub fn set_avatar_load_state_on_humanoid_load(
    character_query: Query<(), (With<Player>, With<Humanoid>)>,
    mut next_state: ResMut<NextState<AvatarLoadState>>,
) {
    if !character_query.is_empty() {
        info!("Character loaded; advancing state");
        next_state.set(AvatarLoadState::Loaded);
    }
}

pub fn equip_spawn_item_on_humanoid_load<T: Character>(
    character_query: Query<Entity, (With<T>, With<Humanoid>)>,
    mut weapon_events: EventWriter<ItemSpawnEvent<<T as Character>::StartItem>>,
) {
    for e_character in character_query.iter() {
        weapon_events.send(ItemSpawnEvent::<<T as Character>::StartItem> {
            parent_entity: Some(e_character),
            ..Default::default()
        });
    }
}

pub fn enable_input_for_player_items(
    mut commands: Commands,
    character_query: Query<&Equipped, (With<PlayerCharacter>, Changed<Equipped>)>,
) {
    for equipped in character_query.iter() {
        for e_item in [equipped.left, equipped.right] {
            commands.entity(e_item).insert((
                InputHandler,
                CollisionGroups::from_group_default(Group::PLAYER_PROJECTILE),
            ));
        }
    }
}

// TODO: disable for unequip! what's a good way to do this?
// will probably need to keep track of dropped items in a vec somewhere.

pub struct CharacterPlugins;

impl PluginGroup for CharacterPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(MasterCharacterPlugin)
            .add(GrinPlugin)
            .add(SmirkPlugin)
    }
}

pub trait Character: Component + Sized + Spawnable {
    type StartItem: Component;
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
    mesh_query: Query<(), With<Handle<Mesh>>>,
    children_query: Query<&Children>,
) {
    let Ok((e_humanoid, humanoid, race)) = player_query.get_single_mut() else {
        return;
    };

    // PLACEHOLDERS
    let left = commands.spawn_empty().id();
    let right = commands.spawn_empty().id();

    commands.entity(e_humanoid).insert((
        Player,
        HealthBundle {
            health: Health(100.0),
            ..Default::default()
        },
        Equipped { left, right },
        RigidBody::KinematicPositionBased,
        Velocity::default(),
        CollisionGroups::from_group_default(Group::PLAYER),
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

    commands
        .entity(humanoid.head)
        .insert(SpatialListener::new(1.0));

    for e_child in children_query.iter_descendants(e_humanoid) {
        if mesh_query.get(e_child).is_ok() {
            commands.entity(e_child).insert(RenderLayers::from_layers(&[
                RenderLayer::STANDARD as u8,
                RenderLayer::AVATAR as u8,
            ]));
        }
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
            CameraAlignment::Shooter { .. } => {
                transform.rotation = Quat::from_rotation_y(look_info.yaw)
            }
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

#[derive(States, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum AvatarLoadState {
    #[default]
    NotLoaded,
    Loaded,
}
