use bevy::{ecs::query::QuerySingleError, prelude::*};
use grin_input::camera::{CameraAlignment, LookInfo, PlayerCamera};
use grin_item::mechanics::firing::Target;

use crate::PlayerCharacter;

pub fn get_local_mouse_target(
    camera_query: &Query<&PlayerCamera>,
    humanoid_query: &Query<&GlobalTransform, With<PlayerCharacter>>,
    look_info: &LookInfo,
) -> Result<Target, QuerySingleError> {
    let (camera, g_plr_transform) = (camera_query.get_single()?, humanoid_query.get_single()?);

    let target_pos = match camera.alignment {
        CameraAlignment::FortyFive => look_info
            .vertical_target_point(g_plr_transform.translation(), g_plr_transform.up())
            .unwrap_or_default(),
        CameraAlignment::Shooter { .. } => look_info.target_point(),
    };
    Ok(Target::from_pair(g_plr_transform.translation(), target_pos))
}
