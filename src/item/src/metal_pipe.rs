use std::time::Duration;

use bevy::prelude::*;
use bevy_asset_loader::prelude::{AssetCollection, LoadingStateAppExt};
use bevy_rapier3d::prelude::*;
use grin_asset::AssetLoadState;
use grin_damage::{impact::Impact, ContactDamage, Damage, DamageVariant};
use grin_physics::{collider, CollisionGroupExt, CollisionGroupsExt};
use grin_render::sketched::SketchMaterial;
use grin_rig::humanoid::Humanoid;
use grin_util::{animator_mut, event::Spawnable};

use crate::{
    equip_to_humanoid, find_item_owner,
    firing::{self, FireRate, FiringPlugin, FiringType, SemiFireBundle, ShotFired},
    insert_on_lmb,
    melee::{
        init_hitboxes, update_hammer_winds, Charging, GltfHitboxAutoGen, HitboxManager, Swinging,
        Wind, Winding,
    },
    on_hit_render_impact, Active, Equipped, Item, ItemEquipEvent, ItemPlugin, ItemSet,
    ItemSpawnEvent, WeaponBundle,
};

pub struct MetalPipePlugin;

#[derive(Resource, AssetCollection)]
pub struct MetalPipeAssets {
    #[asset(key = "gltf.metal_pipe")]
    pub pipe: Handle<Scene>,
    #[asset(key = "anim.bonk_combo.0")]
    pub bonk: Handle<AnimationClip>,
}

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum MetalPipeSystemSet {
    Input,
    Fire,
    Effects,
}

impl Plugin for MetalPipePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ItemPlugin::<MetalPipe>::default(),
            FiringPlugin::<MetalPipe>::from(FiringType::SemiAutomatic),
        ))
        .add_collection_to_loading_state::<_, MetalPipeAssets>(AssetLoadState::Loading)
        .configure_sets(
            Update,
            (
                MetalPipeSystemSet::Input
                    .run_if(in_state(AssetLoadState::Success))
                    .before(firing::semi_fire::<MetalPipe>),
                MetalPipeSystemSet::Fire
                    .run_if(in_state(AssetLoadState::Success))
                    .after(firing::semi_fire::<MetalPipe>),
                MetalPipeSystemSet::Effects.run_if(in_state(AssetLoadState::Success)),
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                spawn
                    .pipe(equip_to_humanoid::<MetalPipe>)
                    .in_set(ItemSet::Spawn),
                (insert_on_lmb::<MetalPipe, Active>,)
                    .chain()
                    .in_set(MetalPipeSystemSet::Input),
                (|| Impact::from_burst_radius(2.0))
                    .pipe(on_hit_render_impact::<MetalPipe>)
                    .in_set(MetalPipeSystemSet::Effects),
            ),
        );
    }
}

#[derive(Component, Clone, Default)]
pub struct MetalPipe;

impl Spawnable for MetalPipe {
    type Event = ItemSpawnEvent<MetalPipe>;
}

impl Item for MetalPipe {
    type SpawnEvent = ItemSpawnEvent<MetalPipe>;
    type EquipEvent = ItemEquipEvent<MetalPipe>;
}

pub fn spawn(mut commands: Commands, assets: Res<MetalPipeAssets>) -> Entity {
    commands
        .spawn((
            MetalPipe::default(),
            WeaponBundle::default(),
            SemiFireBundle {
                fire_rate: FireRate(2.0),
                ..Default::default()
            },
            SceneBundle {
                scene: assets.pipe.clone(),
                ..Default::default()
            },
            GltfHitboxAutoGen,
        ))
        .id()
}

/// Primary attack.
pub fn bonk(
    assets: Res<MetalPipeAssets>,
    mut shot_events: EventReader<ShotFired<MetalPipe>>,
    parent_query: Query<&Parent>,
    mut animator_query: Query<&mut AnimationPlayer>,
    mut item_query: Query<&mut CollisionGroups, With<MetalPipe>>,
) {
    for ShotFired { entity: e_item, .. } in shot_events.read() {
        let mut animator = animator_mut!(*e_item, parent_query, animator_query);

        animator.start(assets.bonk.clone());

        let Ok(mut collision_groups) = item_query.get_mut(*e_item) else {
            continue;
        };

        *collision_groups = CollisionGroups::from_group_default(Group::PLAYER_PROJECTILE);
    }
}
