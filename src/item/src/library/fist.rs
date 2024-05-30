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
use grin_damage::{hit::ContactDamage, hitbox::HitboxManager, impact::Impact};
use grin_rig::humanoid::{Humanoid, HumanoidDominantHand};

use crate::{
    equip::{EquippedTo, Grip, Handedness, Models, SlotAlignment},
    mechanics::{
        animation::{AnimatorSystemParams, ReadOnlyAnimatorSystemParams},
        combo::{ComboPlugin, ComboStack},
        firing::{Active, FireRate, FiringBehavior, FiringPlugin, ShotFired},
        fx::on_hit_spawn,
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
    #[asset(key = "sfx.punch.swing")]
    pub swing_audio: Handle<AudioSource>,
    #[asset(key = "sfx.punch.hit")]
    pub hit_audio: Handle<AudioSource>,
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

impl Plugin for FistPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ItemPlugin::<Fist>::default(),
            FiringPlugin::<Fist>::from(HashSet::from([FiringBehavior::SemiAutomatic])),
            ComboPlugin::<FistCombo>::default(),
        ))
        .add_collection_to_loading_state::<_, FistAssets>(AssetLoadState::Loading)
        .add_systems(
            PreUpdate,
            insert_on_lmb::<Fist, Active>.in_set(ItemSet::Input),
        )
        .add_systems(
            Update,
            (
                item_spawner::<Fist, _, _, _>(|mut commands: Commands, assets: Res<FistAssets>| {
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
                punch
                    .in_set(ItemSet::Fire)
                    .run_if(in_state(AssetLoadState::Success)),
                on_hit_spawn(|assets: Res<FistAssets>| {
                    (
                        Impact::from_burst_radius(2.0),
                        AudioBundle {
                            source: assets.hit_audio.clone(),
                            settings: PlaybackSettings::default().with_spatial(true),
                        },
                    )
                })
                .in_set(ItemSet::Effects)
                .run_if(in_state(AssetLoadState::Success)),
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
    mut commands: Commands,
    assets: Res<FistAssets>,
    mut shot_events: EventReader<ShotFired<Fist>>,
    mut animator_params: AnimatorSystemParams,
    mut item_query: Query<(
        &mut ComboStack<FistCombo>,
        &HitboxManager,
        &Models,
        &EquippedTo,
        &SlotAlignment,
    )>,
    humanoid_query: Query<&Humanoid>,
) {
    for ShotFired { entity: e_item, .. } in shot_events.read() {
        let (mut combo, hitboxes, models, EquippedTo { target: e_user }, alignment) =
            item_query.get_mut(*e_item).unwrap();
        let dominant = humanoid_query.get(*e_user).unwrap().dominant_hand_type;
        let mut animator = animator_params.get_mut(*e_item).unwrap();

        // TODO: this is too convoluted. the animations should be able to be
        // flipped, so I can use `DomPunch` and `OffPunch` instead of `L` and `R`.
        let attack = match alignment {
            SlotAlignment::Double => match combo.sequence.last() {
                None | Some(FistCombo::SpinPunch) => match dominant {
                    HumanoidDominantHand::Left => FistCombo::LPunch,
                    HumanoidDominantHand::Right => FistCombo::RPunch,
                },
                Some(FistCombo::LPunch) => match dominant {
                    HumanoidDominantHand::Left => FistCombo::RPunch,
                    HumanoidDominantHand::Right => FistCombo::SpinPunch,
                },
                Some(FistCombo::RPunch) => match dominant {
                    HumanoidDominantHand::Left => FistCombo::SpinPunch,
                    HumanoidDominantHand::Right => FistCombo::LPunch,
                },
            },
            SlotAlignment::Left => FistCombo::LPunch,
            SlotAlignment::Right => FistCombo::RPunch,
        };

        let grips = match attack {
            FistCombo::LPunch => match dominant {
                HumanoidDominantHand::Left => vec![Grip::Hand],
                HumanoidDominantHand::Right => vec![Grip::Offhand],
            },
            FistCombo::RPunch => match dominant {
                HumanoidDominantHand::Left => vec![Grip::Offhand],
                HumanoidDominantHand::Right => vec![Grip::Hand],
            },
            FistCombo::SpinPunch => vec![Grip::Hand, Grip::Offhand],
        };

        let colliders = match attack {
            FistCombo::LPunch => vec!["LFist"],
            FistCombo::RPunch => vec!["RFist"],
            FistCombo::SpinPunch => vec!["LFist", "RFist"],
        }
        .into_iter()
        .map(|s| hitboxes.colliders[&Name::new(s)]);

        // play sounds
        for grip in grips {
            commands.entity(models.targets[&grip]).insert(AudioBundle {
                source: assets.swing_audio.clone(),
                settings: PlaybackSettings::default().with_spatial(true),
            });
        }

        // play animation
        animator.play(assets.attack_anim(attack));

        // increment combo
        combo.push(attack, Duration::from_millis(1500));

        // enable collisions
        for e_collider in colliders {
            commands.entity(e_collider).remove::<ColliderDisabled>();
        }
    }
}

pub fn deactivate_colliders(
    mut commands: Commands,
    item_query: Query<(Entity, &HitboxManager), With<Fist>>,
    animator_params: ReadOnlyAnimatorSystemParams,
) {
    for (e_item, hitboxes) in item_query.iter() {
        let animator = animator_params.get(e_item).unwrap();
        if animator.is_finished() {
            for &e_hitbox in hitboxes.colliders.values() {
                commands.entity(e_hitbox).insert(ColliderDisabled);
            }
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
