use bevy::{pbr::CubemapVisibleEntities, prelude::*, render::primitives::CubemapFrusta};
use bevy_asset_loader::prelude::*;
use bevy_rapier3d::plugin::RapierContext;
use grin_asset::AssetLoadState;
use grin_damage::{impact::Impact, DamageEvent};
use grin_render::sketched::SketchMaterial;

use super::util::try_find_deepest_contact_point;

pub struct ItemFxPlugin;

impl Plugin for ItemFxPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<MuzzleFlashEvent>()
            .add_collection_to_loading_state::<_, Sfx>(AssetLoadState::Loading)
            .add_collection_to_loading_state::<_, ProjectileAssets>(AssetLoadState::Loading)
            .add_systems(Update, (fade_muzzle_flashes, ignite_muzzle_flashes).chain());
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

pub fn on_hit_render_impact<T: Component>(
    In(impact): In<Impact>,
    mut commands: Commands,
    rapier_context: Res<RapierContext>,
    item_query: Query<&GlobalTransform, With<T>>,
    mut damage_events: EventReader<DamageEvent>,
) {
    for damage_event in damage_events.read() {
        let Ok(contact) =
            try_find_deepest_contact_point(damage_event, &rapier_context, &item_query)
        else {
            return;
        };
        commands.spawn((
            TransformBundle::from_transform(Transform::from_translation(contact)),
            impact.clone(),
        ));
    }
}