pub mod humanoid;

use bevy::{prelude::*, scene::InstanceId};
use grin_util::query::cloned_scene_initializer;
use humanoid::Humanoid;

pub struct GrinAnimationPlugin;

impl Plugin for GrinAnimationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, load_insta_poses);
    }
}

/// Makes a rig instantly assume the animation provided.
#[derive(Component, Clone)]
pub struct InstaPose {
    pub clip: Handle<AnimationClip>,
}

pub fn load_insta_poses(
    mut commands: Commands,
    humanoid_query: Query<(Entity, &Humanoid, &InstaPose), Added<Humanoid>>,
    mut animator_query: Query<&mut AnimationPlayer>,
) {
    for (e_rig, humanoid, InstaPose { clip }) in humanoid_query.iter() {
        let Ok(mut animator) = animator_query.get_mut(humanoid.armature) else {
            error!(
                msg="Could not load instapose: Animator not found.",
                e_rig=?e_rig,
            );
            continue;
        };

        animator.play(clip.clone());

        commands.entity(e_rig).remove::<InstaPose>();
    }
}
