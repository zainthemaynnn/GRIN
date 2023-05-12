pub mod camera;
pub mod eightball;

use std::array::IntoIter;
use std::marker::PhantomData;

use crate::asset::AssetLoadState;
use crate::humanoid::{Head, HumanoidAssets, HumanoidBuilder};
use crate::render::sketched::SketchMaterial;

use crate::collisions::CollisionGroupExt;
use crate::item::{Equipped, Item};
use crate::render::RenderLayer;
use bevy::prelude::*;
use bevy::render::camera::Viewport;
use bevy::render::view::RenderLayers;
use bevy_asset_loader::prelude::*;
use bevy_rapier3d::prelude::*;

use self::camera::{LookInfo, PlayerCamera, PlayerCameraPlugin};
use self::eightball::{EightBall, EightBallPlugin};

pub struct CharacterPlugin;

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum CharacterSet {
    Spawn,
}

fn set_avatar_load_state(state: AvatarLoadState) -> impl Fn(ResMut<NextState<AvatarLoadState>>) {
    move |mut next_state| {
        next_state.set(state);
    }
}

impl Plugin for CharacterPlugin {
    fn build(&self, app: &mut App) {
        app.add_state::<AvatarLoadState>()
            .add_plugin(PlayerCameraPlugin)
            .add_plugin(EightBallPlugin)
            .add_collection_to_loading_state::<_, AvatarAssets>(AssetLoadState::Loading)
            .configure_set(CharacterSet::Spawn.run_if(in_state(AssetLoadState::Success)))
            .add_system(
                <EightBall as Character>::spawn
                    .before(CharacterSet::Spawn)
                    .in_schedule(OnEnter(AssetLoadState::Success)),
            )
            .add_systems(
                (
                    apply_system_buffers,
                    set_avatar_load_state(AvatarLoadState::Loaded),
                )
                    .chain()
                    .after(CharacterSet::Spawn)
                    .in_schedule(OnEnter(AssetLoadState::Success)),
            )
            .add_systems((insert_status_viewport,).in_schedule(OnEnter(AvatarLoadState::Loaded)))
            .add_system(char_update.in_set(OnUpdate(AvatarLoadState::Loaded)));
    }
}

pub struct CharacterSpawnEvent<C: Character> {
    phantom_data: PhantomData<C>,
}

impl<C: Character> Default for CharacterSpawnEvent<C> {
    fn default() -> Self {
        CharacterSpawnEvent {
            phantom_data: PhantomData::default(),
        }
    }
}

pub trait Character: Component + Sized {
    type StartItem: Item;

    /// A system that by default, sends a `CharacterSpawnEvent<Character>` for other systems to handle.
    fn spawn(mut events: EventWriter<CharacterSpawnEvent<Self>>) {
        events.send_default();
    }
}

#[derive(Resource, AssetCollection)]
pub struct AvatarAssets {
    #[asset(key = "mesh.pizza_shades")]
    pizza_shades: Handle<Mesh>,
    #[asset(key = "mat.shades")]
    matte_shades: Handle<SketchMaterial>,
    #[asset(key = "mat.smirk")]
    face_smirk: Handle<SketchMaterial>,
    #[asset(key = "mat.meh")]
    face_meh: Handle<SketchMaterial>,
}

impl<'a> HumanoidBuilder<'a> {
    fn new_player(
        commands: &mut Commands,
        assets: &'a HumanoidAssets,
        meshes: &'a Assets<Mesh>,
    ) -> Self {
        let humanoid = Self::new(commands, assets, meshes);
        commands
            .get_or_spawn(humanoid.head)
            .insert(AvatarSimulationBundle::default());
        commands
            .get_or_spawn(humanoid.lhand)
            .insert(AvatarSimulationBundle::default());
        commands
            .get_or_spawn(humanoid.rhand)
            .insert(AvatarSimulationBundle::default());
        commands
            .get_or_spawn(humanoid.body)
            .insert((
                AvatarSimulationBundle::default(),
                PlayerCharacter,
                RigidBody::KinematicPositionBased,
                KinematicCharacterController::default(),
                Equipped::default(),
            ))
            .with_children(|parent| {
                parent.spawn((
                    PlayerCamera,
                    Camera3dBundle {
                        transform: Transform::from_xyz(0.0, 6.0, 12.0)
                            .looking_at(Vec3::NEG_Z, Vec3::Y),
                        ..Default::default()
                    },
                ));
            });
        humanoid
    }
}

#[derive(Component, Default)]
pub struct PlayerCharacter;

#[derive(Component, Default)]
pub struct PlayerHead;

#[derive(Component, Default)]
pub struct Player;

#[derive(Bundle)]
struct AvatarSimulationBundle {
    render_layers: RenderLayers,
    collision_groups: CollisionGroups,
    player: Player,
}

impl Default for AvatarSimulationBundle {
    fn default() -> Self {
        Self {
            render_layers: RenderLayers::from_layers(&[
                RenderLayer::STANDARD as u8,
                RenderLayer::AVATAR as u8,
            ]),
            collision_groups: CollisionGroups::new(
                Group::PLAYER,
                Group::all().difference(Group::PLAYER),
            ),
            player: Player::default(),
        }
    }
}

#[derive(Component)]
struct ViewportCamera;

fn insert_status_viewport(
    mut commands: Commands,
    windows: Query<&Window>,
    body: Query<Entity, With<PlayerCharacter>>,
    head: Query<&Transform, (With<Head>, With<Player>)>,
) {
    let window = windows.single();
    let head = head.single();
    commands
        .spawn(Camera3dBundle {
            transform: Transform::from_translation(head.translation + Vec3::new(0.0, 0.0, -2.0))
                .looking_to(Vec3::Z, Vec3::Y),
            camera: Camera {
                viewport: Some(Viewport {
                    physical_position: UVec2 {
                        x: 0,
                        y: window.physical_height() - 240,
                    },
                    physical_size: UVec2 { x: 240, y: 240 },
                    ..Default::default()
                }),
                order: 1,
                ..Default::default()
            },
            camera_3d: Camera3d {
                clear_color: bevy::core_pipeline::clear_color::ClearColorConfig::None,
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(ViewportCamera)
        .insert(RenderLayers::layer(RenderLayer::AVATAR as u8))
        .set_parent(body.single());
}

fn char_update(
    input: Res<Input<KeyCode>>,
    mut character: Query<
        (&mut KinematicCharacterController, &mut Transform),
        (With<PlayerCharacter>, Without<PlayerHead>),
    >,
    mut head: Query<&mut Transform, (With<PlayerHead>, Without<PlayerCharacter>)>,
    look_info: Res<LookInfo>,
    time: Res<Time>,
) {
    if let Ok((mut char_controller, mut transform)) = character.get_single_mut() {
        if let Ok(mut head_transform) = head.get_single_mut() {
            let target_point = look_info.target_point();
            let target_local = Transform::from_matrix(
                Mat4::from_translation(target_point) * transform.compute_matrix().inverse(),
            );
            let look =
                (target_local.translation - head_transform.translation) * Vec3::new(0.0, 1.0, 1.0);
            let right = head_transform.right();
            head_transform.look_to(look.normalize(), (-look).normalize().cross(right));
        }

        let mut movement = Vec3::ZERO;
        if input.pressed(KeyCode::W) {
            movement += transform.forward();
        }
        if input.pressed(KeyCode::A) {
            movement += transform.left();
        }
        if input.pressed(KeyCode::S) {
            movement += transform.back();
        }
        if input.pressed(KeyCode::D) {
            movement += transform.right();
        }

        char_controller.translation =
            Some(movement.normalize_or_zero() * 3.0 * time.delta_seconds());
        transform.rotation = Quat::from_rotation_y(look_info.yaw);
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
