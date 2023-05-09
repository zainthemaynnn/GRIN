//! Module for items.
//!
//! Items (defined in submodules) should implement their *own* player input handling,
//! but this parent module does define some common input handlers.
//! Theoretically, both AI and players should be able to use an item, so
//! they should work purely based on components. Input should only insert components.

pub mod smg;

use std::marker::PhantomData;

use bevy::{
    app::PluginGroupBuilder, pbr::CubemapVisibleEntities,
    prelude::*, render::primitives::CubemapFrusta, utils::HashSet,
};
use bevy_asset_loader::{asset_collection::AssetCollection, prelude::LoadingStateAppExt};
use bevy_rapier3d::prelude::*;

use crate::{
    asset::AssetLoadState, character::camera::LookInfo, character::Player,
    collisions::CollisionGroupExt, render::sketched::SketchMaterial,
};

use self::smg::SMGPlugin;

pub struct ItemCommonPlugin;

impl Plugin for ItemCommonPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<MuzzleFlashEvent>()
            .add_collection_to_loading_state::<_, Sfx>(AssetLoadState::Loading)
            .add_collection_to_loading_state::<_, ProjectileAssets>(AssetLoadState::Loading)
            .add_systems((fade_muzzle_flashes, ignite_muzzle_flashes).chain());
    }
}

pub struct ItemPlugin<I: Send + Sync + 'static> {
    phantom_data: PhantomData<I>,
}

impl<I: Send + Sync + 'static> Default for ItemPlugin<I> {
    fn default() -> Self {
        Self {
            phantom_data: PhantomData::default(),
        }
    }
}

impl<I: Send + Sync + 'static> Plugin for ItemPlugin<I> {
    fn build(&self, app: &mut App) {
        app.add_event::<ItemSpawnEvent<I>>()
            .add_event::<ItemEquipEvent<I>>()
            .add_system(equip_items::<I>);
    }
}

pub struct ItemPlugins;

impl PluginGroup for ItemPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(ItemCommonPlugin)
            .add(SMGPlugin)
    }
}

#[derive(Resource, AssetCollection)]
pub struct ProjectileAssets {
    #[asset(key = "mesh.gun")]
    gun: Handle<Mesh>,
    #[asset(key = "mesh.bullet_5cm")]
    bullet_5cm: Handle<Mesh>,
    #[asset(key = "mat.bullet")]
    bullet_material: Handle<SketchMaterial>,
    #[asset(key = "mat.gun")]
    gun_material: Handle<SketchMaterial>,
}

#[derive(Resource, AssetCollection)]
pub struct Sfx {
    #[asset(key = "sfx.uzi")]
    uzi: Handle<AudioSource>,
}

#[derive(Component)]
struct Bullet;

#[derive(Bundle)]
struct ProjectileBundle {
    body: RigidBody,
    material_mesh: MaterialMeshBundle<SketchMaterial>,
    collider: Collider,
    velocity: Velocity,
    collision_groups: CollisionGroups,
    sensor: Sensor,
    active_events: ActiveEvents,
    gravity: GravityScale,
}

impl Default for ProjectileBundle {
    fn default() -> Self {
        Self {
            #[rustfmt::skip]
            collision_groups: CollisionGroups::new(
                Group::PLAYER_PROJECTILE,
                Group::all().difference(
                    Group::PLAYER_PROJECTILE
                        .union(Group::AVATAR)
                ),
            ),
            gravity: GravityScale(0.0),
            body: RigidBody::default(),
            material_mesh: MaterialMeshBundle::default(),
            collider: Collider::default(),
            velocity: Velocity::default(),
            sensor: Sensor,
            active_events: ActiveEvents::COLLISION_EVENTS,
        }
    }
}

pub struct ItemSpawnEvent<M> {
    pub parent_entity: Entity,
    pub phantom_data: PhantomData<M>,
}

impl<M> ItemSpawnEvent<M> {
    pub fn new(parent_entity: Entity) -> Self {
        Self {
            parent_entity,
            phantom_data: PhantomData::default(),
        }
    }
}

pub struct ItemEquipEvent<M> {
    pub parent_entity: Entity,
    pub item_entity: Entity,
    pub phantom_data: PhantomData<M>,
}

impl<M> ItemEquipEvent<M> {
    pub fn new(parent_entity: Entity, item_entity: Entity) -> Self {
        Self {
            parent_entity,
            item_entity,
            phantom_data: PhantomData::default(),
        }
    }
}

/// Commonly used for AI or weapon targetting.
#[derive(Component, Debug, Copy, Clone)]
pub struct Target {
    pub transform: Transform,
    pub distance: f32,
}

impl Default for Target {
    fn default() -> Self {
        Self {
            transform: Transform::default(),
            distance: std::f32::MAX,
        }
    }
}

impl Target {
    pub fn from_pair(origin: Vec3, target: Vec3) -> Self {
        Self {
            transform: Transform::from_translation(target),
            distance: target.distance(origin),
        }
    }
}

#[derive(Component, Debug, Copy, Clone, Default)]
pub struct Active(pub bool);

pub trait Item: Component + Sized {
    // Sending this event should spawn the item.
    type SpawnEvent: Event;
    // Sending this event should equip the item.
    type EquipEvent: Event;
}

#[derive(Component, Default)]
pub struct Weapon;

#[derive(Component, Default)]
pub struct Muzzle;

#[derive(Component)]
pub struct MuzzleFlash {
    color: Color,
    intensity: f32,
    fade_time: f32,
}

impl Default for MuzzleFlash {
    fn default() -> Self {
        Self {
            color: Color::ORANGE,
            intensity: 800.0,
            fade_time: 0.1,
        }
    }
}

#[derive(Bundle, Default)]
struct MuzzleFlashBundle {
    flash: MuzzleFlash,
    point_light: PointLight,
    cubemap_visible_entities: CubemapVisibleEntities,
    cubemap_frusta: CubemapFrusta,
}

#[derive(Bundle, Default)]
struct MuzzleBundle {
    muzzle: Muzzle,
    flash_bundle: MuzzleFlashBundle,
    transform: Transform,
    global_transform: GlobalTransform,
    visibility: Visibility,
    computed_visibility: ComputedVisibility,
}

#[derive(Bundle, Default)]
struct WeaponBundle {
    weapon: Weapon,
    material_mesh: MaterialMeshBundle<SketchMaterial>,
    target: Target,
    active: Active,
}

pub struct MuzzleFlashEvent(pub Entity);

fn fade_muzzle_flashes(mut flash_query: Query<(&MuzzleFlash, &mut PointLight)>, time: Res<Time>) {
    for (flash, mut point_light) in flash_query.iter_mut() {
        point_light.intensity = (point_light.intensity
            - (flash.intensity / flash.fade_time) * time.delta_seconds())
        .max(0.0);
    }
}

fn ignite_muzzle_flashes(
    mut flash_query: Query<(&MuzzleFlash, &mut PointLight)>,
    mut events: EventReader<MuzzleFlashEvent>,
) {
    for MuzzleFlashEvent(entity) in events.iter() {
        let Ok((flash, mut point_light)) = flash_query.get_mut(*entity) else {
            return;
        };
        point_light.color = flash.color;
        point_light.intensity = flash.intensity;
    }
}

/// Keeps references to currently bound items.
#[derive(Component, Default)]
pub struct Equipped(pub HashSet<Entity>);

/// This system updates the `Equipped` component when sending `ItemEquippedEvent`s.
/// 
/// Additionally, if the parent entity has the `Player` component,
/// it is propagated to the item.
pub fn equip_items<M: Send + Sync + 'static>(
    mut commands: Commands,
    mut events: EventReader<ItemEquipEvent<M>>,
    player_query: Query<(), With<Player>>,
    mut equipped_query: Query<&mut Equipped>,
) {
    for ItemEquipEvent {
        parent_entity,
        item_entity,
        ..
    } in events.iter()
    {
        match equipped_query.get_mut(*parent_entity) {
            // can't destructure `Mut` >:( >:(
            Ok(mut equipped) => {
                equipped.0.insert(*item_entity);
                if player_query.get(*parent_entity).is_ok() {
                    commands.get_or_spawn(*item_entity).insert(Player);
                }
            }
            Err(_) => println!("Equipped item to entity without `Equipped`."),
        }
    }
}

/// On `(With<Player>, With<T>)`,
/// sets the `Target` component to the user's mouse position.
pub fn insert_local_mouse_target<T: Component>(
    mut item_query: Query<(&mut Target, &GlobalTransform), (With<Player>, With<T>)>,
    look_info: Res<LookInfo>,
) {
    for (mut target, g_transform) in item_query.iter_mut() {
        *target = Target::from_pair(g_transform.translation(), look_info.target_point());
    }
}

/// On `(With<Player>, With<T>)`,
/// - If LMB is pressed, sets the `Active(true)` component.
/// - If LMB is not pressed, sets the `Active(false)` component.
pub fn activate_on_lmb<T: Component>(
    mut query: Query<&mut Active, (With<Player>, With<T>)>,
    mouse_buttons: Res<Input<MouseButton>>,
) {
    if mouse_buttons.pressed(MouseButton::Left) {
        for mut active in query.iter_mut() {
            *active = Active(true);
        }
    } else {
        for mut active in query.iter_mut() {
            *active = Active(false);
        }
    }
}

#[derive(Resource)]
struct Bag {}
