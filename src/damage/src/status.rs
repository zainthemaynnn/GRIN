use std::time::Duration;

use bevy::prelude::*;
use bevy_asset_loader::prelude::*;

pub trait StatusEffect {
    pub const NAME: &'static str;
    pub const DESCRIPTION: &'static str;
}

pub trait Perk {
    pub const NAME: &'static str;
    pub const DESCRIPTION: &'static str;
}

pub struct StatusEffectPlugin;

impl Plugin for StatusEffectPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, apply_stagger);
    }
}

/// Brief stun.
#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct StaggerEffect {
    pub duration: f32,
}

impl StatusEffect for StaggerEffect {
    const NAME: &'static str = "Staggered";
    const DESCRIPTION: &'static str = r#"
Dizzy...
Mild stun. Non-stackable."#;
}

/// Brief stun.
#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct Stagger {
    pub max_duration: f32,
    pub remaining_duration: f32,
}

#[derive(Component)]
pub struct StaggerCycle {
    pub clips: Vec<Handle<AnimationClip>>,
    pub index: usize,
}

impl StaggerCycle {
    fn new(clips: Vec<Handle<AnimationClip>>) -> Self {
        assert!(
            !clips.is_empty(),
            "At least one stagger animation is required."
        );
        Self { clips, index: 0 }
    }

    /// Returns the next stagger animation.
    fn next(&mut self) -> &Handle<AnimationClip> {
        self.index = (self.index + 1) % self.clips.len();
        &self.clips[self.index]
    }
}

impl From<Vec<Handle<AnimationClip>>> for StaggerCycle {
    fn from(clips: Vec<Handle<AnimationClip>>) -> Self {
        Self::new(clips)
    }
}

#[derive(Resource, AssetCollection)]
pub struct StaggerAssets {
    #[asset(key = "asset.stagger.0")]
    pub stagger_0: Handle<AnimationClip>,
    #[asset(key = "asset.stagger.1")]
    pub stagger_1: Handle<AnimationClip>,
    #[asset(key = "asset.stagger.2")]
    pub stagger_2: Handle<AnimationClip>,
}

pub fn apply_stagger(
    mut stagger_query: Query<(Entity, &mut StaggerCycle), With<Stagger>>,
    children_query: Query<&Children>,
    mut animator_query: Query<&mut AnimationPlayer>,
) {
    for (e_stagger, mut stagger_cycle) in stagger_query.iter_mut() {
        for e_child in children_query.iter_descendants(e_stagger) {
            let Ok(mut animator) = animator_query.get_mut(e_child) else {
                continue;
            };

            animator.play_with_transition(stagger_cycle.next().clone(), Duration::from_millis(100));

            break;
        }
    }
}

#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct ChillEffect {
    pub duration: f32,
}

impl StatusEffect for ChillEffect {
    const NAME: &'static str = "Chilly";
    const DESCRIPTION: &'static str = r#"
Soup should do it. Pack some soup next time.
Everything except your combo is 0.5x slower."#;
}

#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct FreezeEffect {
    pub duration: f32,
}

impl StatusEffect for FreezeEffect {
    const NAME: &'static str = "Iced";
    const DESCRIPTION: &'static str = r#"
I guess you should just chill until it wears off.
Temporary damage immunity. Your movement is confined to a bouncing ice cube."#;
}

#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct StunEffect {
    pub duration: f32,
}

impl StatusEffect for StunEffect {
    const NAME: &'static str = "Knocked";
    const DESCRIPTION: &'static str = r#"
Heavy stun. If someone else isn't stunned, you're pretty screwed, right now."#;
}

#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct BurnEffect {
    pub duration: f32,
}

impl StatusEffect for BurnEffect {
    const NAME: &'static str = "Torched";
    const DESCRIPTION: &'static str = r#"
It's okay. Ceramic objects are pretty heat resistant.
Constant energy damage. 2x speed boost."#;
}

#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct NauseaEffect {
    pub duration: f32,
}

impl StatusEffect for NauseaEffect {
    const NAME: &'static str = "Nauseated";
    const DESCRIPTION: &'static str = r#"
Oh, that is... ugh... bro... why did they have to go and do that?
Blurry vision and -50% accuracy.
This lasts until you throw up somewhere with the reload key."#;
}

#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct ShatterEffect {
    pub duration: f32,
}

impl StatusEffect for ShatterEffect {
    const NAME: &'static str = "Shattered";
    const DESCRIPTION: &'static str = r#"
Your atoms are vibrating out of their sockets.
Double damage. Stacks reduced on hit, or when duration expires."#;
}

#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct FlashEffect {
    pub duration: f32,
}

impl StatusEffect for FlashEffect {
    const NAME: &'static str = "Flashed";
    const DESCRIPTION: &'static str = r#"
Memories slip away...
-20% accuracy +200% combo buildup."#;
}

#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct OrthogonalEffect {
    pub duration: f32,
}

impl StatusEffect for OrthogonalEffect {
    const NAME: &'static str = "Orthogonalized";
    const DESCRIPTION: &'static str = r#"
When they hit you with that `V = {(x, y, z) ∈ R^3 | xy = z = 0}`.
Movement is restricted to four directions."#;
}
