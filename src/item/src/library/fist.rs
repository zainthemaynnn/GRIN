//! This is the default "weapon" used while unarmed. It doesn't show up naturally in the game.
//!
//! It's not very different compared to standard melee weapons. The only notable difference is
//! that its `HandAlignment` can be both single and double.
//!
//! Folly's default time scale is adjusted to 2x, and has access to an additional combo.

use std::time::Duration;

use bevy::{prelude::*, utils::HashSet};
use bevy_asset_loader::prelude::{AssetCollection, LoadingStateAppExt};
use bevy_rapier3d::prelude::*;
use grin_asset::AssetLoadState;
use grin_damage::{impact::Impact, ContactDamage};
use grin_physics::{CollisionGroupExt, CollisionGroupsExt};
use grin_util::{animator, animator_mut};

use crate::{
    equip::{Grip, Handedness, Models, SlotAlignment},
    mechanics::{
        combo::{ComboPlugin, ComboStack},
        firing::{self, Active, FireRate, FiringBehavior, FiringPlugin, ShotFired},
        fx::on_hit_render_impact,
        hitbox::DamageCollisionGroups,
        util::insert_on_lmb,
    },
    models,
    plugin::{ItemPlugin, ItemSet, WeaponBundle},
    spawn::item_spawner,
};

pub use super::plugin::item_identifier_filters::Fist;
use super::plugin::ItemIdentifier;

pub struct FistPlugin;

#[derive(Resource, AssetCollection)]
pub struct FistAssets {
    // there's nothing visual here; just hitboxes
    #[asset(key = "scene.fist")]
    pub fist: Handle<Scene>,
    #[asset(key = "anim.punch.left")]
    pub lpunch: Handle<AnimationClip>,
    #[asset(key = "anim.punch.right")]
    pub rpunch: Handle<AnimationClip>,
    #[asset(key = "anim.punch.spin")]
    pub spin_punch: Handle<AnimationClip>,
}

impl FistAssets {
    pub fn attack_anim(&self, attack: FistCombo) -> Handle<AnimationClip> {
        match attack {
            FistCombo::LPunch => self.lpunch.clone(),
            FistCombo::RPunch => self.rpunch.clone(),
            FistCombo::SpinPunch => self.spin_punch.clone(),
        }
    }
}

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum FistSystemSet {
    Input,
    Fire,
    Effects,
}

impl Plugin for FistPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ItemPlugin::<Fist>::default(),
            FiringPlugin::<Fist>::from(HashSet::from([FiringBehavior::SemiAutomatic])),
            ComboPlugin::<FistCombo>::default(),
        ))
        .add_collection_to_loading_state::<_, FistAssets>(AssetLoadState::Loading)
        .configure_sets(
            Update,
            (
                FistSystemSet::Input
                    .run_if(in_state(AssetLoadState::Success))
                    .before(firing::semi_fire::<Fist>),
                FistSystemSet::Fire
                    .run_if(in_state(AssetLoadState::Success))
                    .after(firing::semi_fire::<Fist>),
                FistSystemSet::Effects.run_if(in_state(AssetLoadState::Success)),
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                item_spawner::<Fist, _, _>(|mut commands: Commands, assets: Res<FistAssets>| {
                    WeaponBundle::<FistCombo> {
                        identifier: ItemIdentifier::Fist,
                        models: models![
                            commands,
                            (Grip::Hand, assets.fist.clone()),
                            (Grip::Offhand, assets.fist.clone()),
                        ],
                        handedness: Handedness::Double,
                        // TODO: I think contact damage needs to be reworked
                        // also, `Debounce` isn't even implemented yet
                        contact_damage: ContactDamage::Debounce(Duration::from_millis(200)),
                        fire_rate: FireRate(Duration::from_millis(800)),
                        ..Default::default()
                    }
                })
                .in_set(ItemSet::Spawn),
                (insert_on_lmb::<Fist, Active>,)
                    .chain()
                    .in_set(FistSystemSet::Input),
                (|| Impact::from_burst_radius(2.0))
                    .pipe(on_hit_render_impact::<Fist>)
                    .in_set(FistSystemSet::Effects),
            ),
        );
    }
}

#[derive(Clone, Copy, Debug)]
pub enum FistCombo {
    LPunch,
    RPunch,
    SpinPunch,
}

/// Primary attack.
pub fn punch(
    assets: Res<FistAssets>,
    mut shot_events: EventReader<ShotFired<Fist>>,
    parent_query: Query<&Parent>,
    mut animator_query: Query<&mut AnimationPlayer>,
    mut item_query: Query<(
        &mut ComboStack<FistCombo>,
        &mut DamageCollisionGroups,
        &SlotAlignment,
    )>,
) {
    for ShotFired { entity: e_item, .. } in shot_events.read() {
        let (mut combo, mut collision_groups, alignment) = item_query.get_mut(*e_item).unwrap();
        let mut animator = animator_mut!(*e_item, parent_query, animator_query);

        let attack = match alignment {
            SlotAlignment::Double => match combo.sequence.last() {
                None | Some(FistCombo::SpinPunch) => FistCombo::LPunch,
                Some(FistCombo::LPunch) => FistCombo::RPunch,
                Some(FistCombo::RPunch) => FistCombo::SpinPunch,
            },
            SlotAlignment::Left => FistCombo::LPunch,
            SlotAlignment::Right => FistCombo::RPunch,
            SlotAlignment::None => continue,
        };

        animator.play(assets.attack_anim(attack));

        combo.push(attack, Duration::from_millis(1500));

        collision_groups.0 = CollisionGroups::from_group_default(Group::PLAYER_PROJECTILE);
    }
}

pub fn deactivate_colliders(
    mut item_query: Query<(Entity, &mut DamageCollisionGroups), With<Fist>>,
    parent_query: Query<&Parent>,
    animator_query: Query<&AnimationPlayer>,
) {
    for (e_item, mut collision_groups) in item_query.iter_mut() {
        let animator = animator!(e_item, parent_query, animator_query);
        if animator.is_finished() {
            collision_groups.0 = CollisionGroups::default();
        }
    }
}

pub fn sock_wind() {
    todo!();
}

pub fn sock() {
    todo!();
}

pub fn flurry() {
    todo!();
}