//! Module for items.
//!
//! Items (defined in submodules) should implement their *own* player input handling,
//! but this parent module does define some common input handlers.
//! Theoretically, both AI and players should be able to use an item, so
//! they should work purely based on components. Input should only insert components.

pub mod firing;
pub mod smg;

use std::marker::PhantomData;

use bevy::{
    app::PluginGroupBuilder, pbr::CubemapVisibleEntities, prelude::*,
    render::primitives::CubemapFrusta, utils::HashSet,
};
use bevy_asset_loader::{asset_collection::AssetCollection, prelude::LoadingStateAppExt};

use crate::{
    asset::AssetLoadState,
    character::camera::LookInfo,
    character::Player,
    humanoid::{DominantHand, Hand, HandOffsets},
    render::sketched::SketchMaterial,
};

use self::smg::SMGPlugin;

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
            .configure_set(ItemSet::Spawn.run_if(in_state(AssetLoadState::Success)))
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

impl<I: Item + Send + Sync + 'static> Plugin for ItemPlugin<I> {
    fn build(&self, app: &mut App) {
        app.add_event::<<I as Item>::SpawnEvent>()
            .add_event::<<I as Item>::EquipEvent>()
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
}

#[derive(Resource, AssetCollection)]
pub struct Sfx {
    #[asset(key = "sfx.uzi")]
    pub uzi: Handle<AudioSource>,
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
struct MuzzleFlashBundle {
    pub flash: MuzzleFlash,
    pub point_light: PointLight,
    pub cubemap_visible_entities: CubemapVisibleEntities,
    pub cubemap_frusta: CubemapFrusta,
}

#[derive(Bundle, Default)]
struct MuzzleBundle {
    pub muzzle: Muzzle,
    pub flash_bundle: MuzzleFlashBundle,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub visibility: Visibility,
    pub computed_visibility: ComputedVisibility,
}

#[derive(Bundle, Default)]
struct WeaponBundle {
    pub weapon: Weapon,
    pub target: Target,
    pub aiming: Aiming,
    pub accuracy: Accuracy,
    pub material_mesh: MaterialMeshBundle<SketchMaterial>,
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
pub fn set_local_mouse_target<T: Component>(
    mut item_query: Query<(&mut Target, &GlobalTransform), (With<Player>, With<T>)>,
    look_info: Res<LookInfo>,
) {
    for (mut target, g_transform) in item_query.iter_mut() {
        *target = Target::from_pair(g_transform.translation(), look_info.target_point());
    }
}

/// On `(With<Player>, With<T>)`,
/// - If `mouse_button` is pressed, sets `C::from(true)`.
/// - If `mouse_button` is not pressed, sets `C::from(false)`.
pub fn set_on_mouse_button<T: Component, C: Component + From<bool>>(
    mouse_button: MouseButton,
    mut query: Query<&mut C, (With<Player>, With<T>)>,
    mouse_buttons: Res<Input<MouseButton>>,
) {
    if mouse_buttons.pressed(mouse_button) {
        for mut cmpt in query.iter_mut() {
            *cmpt = C::from(true);
        }
    } else {
        for mut cmpt in query.iter_mut() {
            *cmpt = C::from(false);
        }
    }
}

/// On `(With<Player>, With<T>)`,
/// - If LMB is pressed, sets `C::from(true)`.
/// - If LMB is not pressed, sets `C::from(false)`.
pub fn set_on_lmb<T: Component, C: Component + From<bool>>(
    query: Query<&mut C, (With<Player>, With<T>)>,
    mouse_buttons: Res<Input<MouseButton>>,
) {
    set_on_mouse_button(MouseButton::Left, query, mouse_buttons);
}

/// On `(With<Player>, With<T>)`,
/// - If RMB is pressed, sets `C::from(true)`.
/// - If RMB is not pressed, sets `C::from(false)`.
pub fn set_on_rmb<T: Component, C: Component + From<bool>>(
    query: Query<&mut C, (With<Player>, With<T>)>,
    mouse_buttons: Res<Input<MouseButton>>,
) {
    set_on_mouse_button(MouseButton::Right, query, mouse_buttons);
}

#[derive(Component, Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct Active(pub bool);

impl From<bool> for Active {
    fn from(value: bool) -> Self {
        Self(value)
    }
}

#[derive(Component, Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct Aiming(pub bool);

impl From<bool> for Aiming {
    fn from(value: bool) -> Self {
        Self(value)
    }
}

pub fn aim_single<T: Component>(
    item_query: Query<(&Parent, &Aiming), (With<T>, Changed<Aiming>)>,
    mut hands_query: Query<(&mut Hand, &HandOffsets), With<DominantHand>>,
) {
    for (parent, Aiming(aiming)) in item_query.iter() {
        let Ok((mut hand, offsets)) = hands_query.get_mut(parent.get()) else {
            println!("`aim_single` requires the item to be parented to a (`humanoid::Hand, humanoid::HandOffsets`).");
            return;
        };

        if *aiming {
            hand.offset = offsets.aim_single;
        } else {
            hand.offset = offsets.rest;
        }
    }
}
