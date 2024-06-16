pub mod humanoid;

use bevy::{animation::RepeatAnimation, prelude::*};
use humanoid::Humanoid;

pub struct GrinAnimationPlugin;

impl Plugin for GrinAnimationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, load_idle);
    }
}

/// Makes a rig instantly assume the animation provided.
#[derive(Component, Clone)]
pub struct Idle {
    pub clip: Handle<AnimationClip>,
}

pub fn load_idle(
    mut commands: Commands,
    // TODO: apply query filters here
    // component will probably have to be moved to the armature itself
    // in order to track `Changed<AnimationPlayer>`,
    // which is a bit annoying + too lazy to do RN.
    humanoid_query: Query<(Entity, &Humanoid, Ref<Idle>)>,
    mut animator_query: Query<&mut AnimationPlayer>,
) {
    for (e_rig, humanoid, idle) in humanoid_query.iter() {
        let Ok(mut animator) = animator_query.get_mut(humanoid.armature) else {
            error!(
                msg="Could not load idle animation: Animator not found.",
                e_rig=?e_rig,
            );
            continue;
        };

        // HACK: second condition checks whether the animator was never played
        // (there may be some edge cases where this occurs naturally; will need
        // to figure this one out).
        if animator.is_finished() || animator.elapsed() == 0.0 || idle.is_changed() {
            animator
                .set_repeat(RepeatAnimation::Forever)
                .play(idle.clip.clone());
        }
    }
}
