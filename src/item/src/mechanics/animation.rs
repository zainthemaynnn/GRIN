use std::time::Duration;

use bevy::prelude::*;
use bevy_asset_loader::{asset_collection::AssetCollection, loading_state::LoadingStateAppExt};
use grin_asset::AssetLoadState;
use grin_rig::humanoid::HumanoidDominantHand;

use super::firing::Active;

pub struct ItemAnimationPlugin;

impl Plugin for ItemAnimationPlugin {
    fn build(&self, app: &mut App) {
        app.add_collection_to_loading_state::<_, AimAssets>(AssetLoadState::Loading);
    }
}

/// Whether the item's aiming animation is playing.
#[derive(Component, Debug, Copy, Clone, Eq, PartialEq, Default)]
#[component(storage = "SparseSet")]
pub struct Aiming;

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
