//! Module for items.
//!
//! Items (defined in submodules) should implement their *own* player input handling,
//! but this parent module does define some common input handlers.
//! Theoretically, both AI and players should be able to use an item, so
//! they should work purely based on components. Input should only insert components.

pub mod firing;
pub mod melee;
pub mod sledge;
pub mod smg;

use std::{marker::PhantomData, time::Duration};

use bevy::{
    app::PluginGroupBuilder, ecs::query::QueryEntityError, pbr::CubemapVisibleEntities, prelude::*,
    render::primitives::CubemapFrusta, utils::HashSet,
};
use bevy_asset_loader::{asset_collection::AssetCollection, prelude::LoadingStateAppExt};
use bevy_rapier3d::prelude::*;
use grin_asset::AssetLoadState;
use grin_damage::{impact::Impact, DamageEvent};
use grin_input::camera::{CameraAlignment, LookInfo, PlayerCamera};
use grin_physics::{CollisionGroupExt, CollisionGroupsExt};
use grin_render::sketched::SketchMaterial;
use grin_rig::humanoid::HumanoidDominantHand;
use grin_util::event::Spawnable;

use self::{sledge::SledgePlugin, smg::SMGPlugin};

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum ItemSet {
    Spawn,
}

pub struct ItemCommonPlugin;

impl Plugin for ItemCommonPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<MuzzleFlashEvent>()
            .add_collection_to_loading_state::<_, Sfx>(AssetLoadState::Loading)
            .add_collection_to_loading_state::<_, ProjectileAssets>(AssetLoadState::Loading)
            .add_collection_to_loading_state::<_, AimAssets>(AssetLoadState::Loading)
            .configure_sets(
                Update,
                ItemSet::Spawn.run_if(in_state(AssetLoadState::Success)),
            )
            .add_systems(Update, (fade_muzzle_flashes, ignite_muzzle_flashes).chain());
    }
}

pub struct ItemPlugin<I: Send + Sync + 'static> {
    pub phantom_data: PhantomData<I>,
}

impl<I: Send + Sync + 'static> Default for ItemPlugin<I> {
    fn default() -> Self {
        Self {
            phantom_data: PhantomData::default(),
        }
    }
}

impl<I: Item + Send + Sync + 'static> Plugin for ItemPlugin<I> {
    fn build(&self, app: &mut App) {
        app.add_event::<<I as Item>::SpawnEvent>()
            .add_event::<<I as Item>::EquipEvent>()
            .add_systems(Update, equip_items::<I>);
    }
}

pub struct ItemPlugins;

impl PluginGroup for ItemPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(ItemCommonPlugin)
            .add(SMGPlugin)
            .add(SledgePlugin)
    }
}

#[derive(Resource, AssetCollection)]
pub struct ProjectileAssets {
    #[asset(key = "mesh.gun")]
    pub gun: Handle<Mesh>,
    #[asset(key = "mesh.bullet_5cm")]
    pub bullet_5cm: Handle<Mesh>,
    #[asset(key = "mesh.bullet_8cm")]
    pub bullet_8cm: Handle<Mesh>,
    #[asset(key = "mesh.bullet_10cm")]
    pub bullet_10cm: Handle<Mesh>,
    #[asset(key = "mat.bullet")]
    pub bullet_material: Handle<SketchMaterial>,
    #[asset(key = "mat.gun")]
    pub gun_material: Handle<SketchMaterial>,
    #[asset(key = "mat.laser")]
    pub laser_material: Handle<SketchMaterial>,
}

#[derive(Resource, AssetCollection)]
pub struct Sfx {
    #[asset(key = "sfx.uzi")]
    pub uzi: Handle<AudioSource>,
}

#[derive(Event, Clone)]
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

#[derive(Event, Clone)]
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

/// Collision groups to use when dealing damage.
#[derive(Component, Copy, Clone)]
pub struct DamageCollisionGroups(pub CollisionGroups);

impl Default for DamageCollisionGroups {
    fn default() -> Self {
        Self(CollisionGroups::from_group_default(
            Group::PLAYER_PROJECTILE,
        ))
    }
}

impl From<&DamageCollisionGroups> for CollisionGroups {
    fn from(value: &DamageCollisionGroups) -> Self {
        value.0
    }
}

pub trait Item: Component + Sized + Spawnable {
    /// Sending this event should spawn the item.
    type SpawnEvent: Event;
    /// Sending this event should equip the item.
    type EquipEvent: Event;
}

#[derive(Component, Default)]
pub struct Weapon;

#[derive(Component, Default)]
pub struct Muzzle;

#[derive(Component)]
pub struct MuzzleFlash {
    pub color: Color,
    pub intensity: f32,
    pub fade_time: f32,
}

impl Default for MuzzleFlash {
    fn default() -> Self {
        Self {
            color: Color::ORANGE,
            intensity: 800.0,
            fade_time: 0.08,
        }
    }
}

#[derive(Bundle, Default)]
pub struct MuzzleFlashBundle {
    pub flash: MuzzleFlash,
    pub point_light: PointLight,
    pub cubemap_visible_entities: CubemapVisibleEntities,
    pub cubemap_frusta: CubemapFrusta,
}

#[derive(Bundle, Default)]
pub struct MuzzleBundle {
    pub muzzle: Muzzle,
    pub flash_bundle: MuzzleFlashBundle,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub visibility: Visibility,
    pub inherited_visibility: InheritedVisibility,
    pub view_visibility: ViewVisibility,
}

#[derive(Bundle, Default)]
pub struct WeaponBundle {
    pub weapon: Weapon,
    pub target: Target,
    pub accuracy: Accuracy,
    pub damage_collision_groups: DamageCollisionGroups,
}

#[derive(Event)]
pub struct MuzzleFlashEvent(pub Entity);

pub fn fade_muzzle_flashes(
    mut flash_query: Query<(&MuzzleFlash, &mut PointLight)>,
    time: Res<Time>,
) {
    for (flash, mut point_light) in flash_query.iter_mut() {
        point_light.intensity = (point_light.intensity
            - (flash.intensity / flash.fade_time) * time.delta_seconds())
        .max(0.0);
    }
}

pub fn ignite_muzzle_flashes(
    mut flash_query: Query<(&MuzzleFlash, &mut PointLight)>,
    mut events: EventReader<MuzzleFlashEvent>,
) {
    for MuzzleFlashEvent(entity) in events.read() {
        let Ok((flash, mut point_light)) = flash_query.get_mut(*entity) else {
            return;
        };
        point_light.color = flash.color;
        point_light.intensity = flash.intensity;
    }
}

/// Enables user input for this item.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct InputHandler;

/// On `(With<InputHandler>, With<T>)`,
/// - If `mouse_button` is pressed, inserts `C`.
/// - If `mouse_button` is not pressed, removes `C`.
pub fn insert_on_mouse_button<T: Component, C: Component + Default>(
    commands: &mut Commands,
    query: &Query<Entity, (With<T>, With<InputHandler>)>,
    mouse_buttons: &Input<MouseButton>,
    mouse_button: MouseButton,
) {
    if mouse_buttons.pressed(mouse_button) {
        for entity in query.iter() {
            commands.entity(entity).insert(C::default());
        }
    } else {
        for entity in query.iter() {
            commands.entity(entity).remove::<C>();
        }
    }
}

/// On `(With<InputHandler>, With<T>)`,
/// - If LMB is pressed, inserts `C`.
/// - If LMB is not pressed, removes `C`.
pub fn insert_on_lmb<T: Component, C: Component + Default>(
    mut commands: Commands,
    query: Query<Entity, (With<T>, With<InputHandler>)>,
    mouse_buttons: Res<Input<MouseButton>>,
) {
    insert_on_mouse_button::<T, C>(&mut commands, &query, &mouse_buttons, MouseButton::Left);
}

/// On `(With<InputHandler>, With<T>)`,
/// - If RMB is pressed, inserts `C`.
/// - If RMB is not pressed, removes `C`.
pub fn insert_on_rmb<T: Component, C: Component + Default>(
    mut commands: Commands,
    query: Query<Entity, (With<T>, With<InputHandler>)>,
    mouse_buttons: Res<Input<MouseButton>>,
) {
    insert_on_mouse_button::<T, C>(&mut commands, &query, &mouse_buttons, MouseButton::Right);
}

/// Keeps references to currently bound items.
#[derive(Component, Default)]
pub struct Equipped(pub HashSet<Entity>);

/// This system updates the `Equipped` component when sending `ItemEquippedEvent`s.
pub fn equip_items<M: Send + Sync + 'static>(
    _commands: Commands,
    mut events: EventReader<ItemEquipEvent<M>>,
    mut equipped_query: Query<&mut Equipped>,
) {
    for ItemEquipEvent {
        parent_entity,
        item_entity,
        ..
    } in events.read()
    {
        match equipped_query.get_mut(*parent_entity) {
            // can't destructure `Mut` >:( >:(
            Ok(mut equipped) => {
                info!("Equipped item {:?} to {:?}.", parent_entity, item_entity);
                equipped.0.insert(*item_entity);
            }
            Err(_) => error!("Equipped item to entity without `Equipped`."),
        }
    }
}

/// Returns the first ancestor with an `Equipped` component.
pub fn find_item_owner(
    e_item: Entity,
    parent_query: &Query<&Parent, With<Equipped>>,
) -> Option<Entity> {
    parent_query.iter_ancestors(e_item).next()
}

/// On `(With<InputHandler>, With<T>)`,
/// sets the `Target` component to the user's mouse position.
pub fn set_local_mouse_target<T: Component>(
    camera_query: Query<&PlayerCamera>,
    mut item_query: Query<(&mut Target, &GlobalTransform), (With<InputHandler>, With<T>)>,
    look_info: Res<LookInfo>,
) {
    let Ok(camera) = camera_query.get_single() else {
        return;
    };

    for (mut target, g_transform) in item_query.iter_mut() {
        let target_pos = match camera.alignment {
            CameraAlignment::FortyFive => look_info
                .vertical_target_point(g_transform.translation(), g_transform.up())
                .unwrap_or_default(),
            CameraAlignment::Shooter => look_info.target_point(),
        };
        *target = Target::from_pair(g_transform.translation(), target_pos);
    }
}

/// Idle animation.
#[derive(Component, Default)]
pub enum IdleType {
    #[default]
    Idle,
}

/// Aim animation.
#[derive(Component, Default)]
pub enum AimType {
    #[default]
    RangedSingle,
}

#[derive(Resource, AssetCollection)]
pub struct AimAssets {
    #[asset(key = "anim.idle")]
    pub idle: Handle<AnimationClip>,
    #[asset(key = "anim.pistol.right")]
    pub ranged_single_rt: Handle<AnimationClip>,
    #[asset(key = "anim.pistol.left")]
    pub ranged_single_lt: Handle<AnimationClip>,
}

/// Plays the aim animations on `item::Active`.
pub fn aim_on_active<T: Component>(
    mut commands: Commands,
    assets: Res<AimAssets>,
    item_query: Query<(Entity, &AimType), (With<T>, With<Active>, Without<Aiming>)>,
    humanoid_query: Query<&HumanoidDominantHand>,
    mut animator_query: Query<&mut AnimationPlayer>,
    parent_query: Query<&Parent>,
) {
    for (e_item, aim_type) in item_query.iter() {
        let dominant = parent_query
            .iter_ancestors(e_item)
            .find_map(|e| humanoid_query.get(e).ok())
            .unwrap();

        for e_parent in parent_query.iter_ancestors(e_item) {
            let Ok(mut animator) = animator_query.get_mut(e_parent) else {
                continue;
            };

            let clip = match aim_type {
                AimType::RangedSingle => match dominant {
                    HumanoidDominantHand::Left => &assets.ranged_single_lt,
                    HumanoidDominantHand::Right => &assets.ranged_single_rt,
                },
            };

            animator.play_with_transition(clip.clone(), Duration::from_secs_f32(0.1));
            commands.entity(e_item).insert(Aiming);

            break;
        }
    }
}

/// Plays the un-aim animations on un-`item::Active`.
pub fn unaim_on_unactive<T: Component>(
    mut commands: Commands,
    assets: Res<AimAssets>,
    item_query: Query<(Entity, &IdleType), (With<T>, Without<Active>, With<Aiming>)>,
    mut animator_query: Query<&mut AnimationPlayer>,
    parent_query: Query<&Parent>,
) {
    for (e_item, idle_type) in item_query.iter() {
        for e_parent in parent_query.iter_ancestors(e_item) {
            let Ok(mut animator) = animator_query.get_mut(e_parent) else {
                continue;
            };

            let clip = match idle_type {
                IdleType::Idle => &assets.idle,
            };

            animator.play_with_transition(clip.clone(), Duration::from_secs_f32(0.1));
            commands.entity(e_item).remove::<Aiming>();

            break;
        }
    }
}

/// Whether the item is being "used."
#[derive(Component, Debug, Copy, Clone, Eq, PartialEq, Default)]
#[component(storage = "SparseSet")]
pub struct Active;

/// Whether the item's aiming animation is playing.
#[derive(Component, Debug, Copy, Clone, Eq, PartialEq, Default)]
#[component(storage = "SparseSet")]
pub struct Aiming;

/// For most items, affects the accuracy of projectiles in different ways. A higher number is better.
///
/// `1.0` is the default. Can't go below zero.
#[derive(Component, Debug, Copy, Clone, PartialEq)]
pub struct Accuracy(pub f32);

impl Default for Accuracy {
    fn default() -> Self {
        Self(1.0)
    }
}

impl From<f32> for Accuracy {
    fn from(value: f32) -> Self {
        Self(value)
    }
}

#[derive(Debug)]
pub enum DamageContactError {
    EventMismatch(DamageEvent),
    ItemQueryMismatch(QueryEntityError),
    NoContactPair(Entity, Entity),
    NoContact(Entity, Entity),
}

/// Helper function for finding a collision point.
pub fn try_find_deepest_contact_point<T: Component>(
    damage_event: &DamageEvent,
    rapier_context: &RapierContext,
    item_query: &Query<&GlobalTransform, With<T>>,
) -> Result<Vec3, DamageContactError> {
    let &DamageEvent::Contact { e_damage, e_hit, .. } = damage_event else {
        return Err(DamageContactError::EventMismatch(damage_event.clone()));
    };
    let g_item_transform = item_query
        .get(e_damage)
        .map_err(|e| DamageContactError::ItemQueryMismatch(e))?;
    let contact_pair = rapier_context
        .contact_pair(e_hit, e_damage)
        .ok_or(DamageContactError::NoContactPair(e_damage, e_hit))?;
    let contact = contact_pair
        .find_deepest_contact()
        .ok_or(DamageContactError::NoContact(e_damage, e_hit))?;
    let contact_point = g_item_transform.transform_point(contact.1.local_p1());
    Ok(contact_point)
}

pub fn on_hit_render_impact<T: Component>(
    In(impact): In<Impact>,
    mut commands: Commands,
    rapier_context: Res<RapierContext>,
    item_query: Query<&GlobalTransform, With<T>>,
    mut damage_events: EventReader<DamageEvent>,
) {
    for damage_event in damage_events.read() {
        let Ok(contact) = try_find_deepest_contact_point(damage_event, &rapier_context, &item_query) else {
            return;
        };
        commands.spawn((
            TransformBundle::from_transform(Transform::from_translation(contact)),
            impact.clone(),
        ));
    }
}
