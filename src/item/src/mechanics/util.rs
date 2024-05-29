use bevy::{ecs::query::QueryEntityError, prelude::*};
use bevy_rapier3d::prelude::*;
use grin_damage::hit::DamageEvent;
use grin_input::camera::{CameraAlignment, LookInfo, PlayerCamera};

use crate::equip::{Equipped, SlotAlignment};

use super::firing::Target;

/// Returns the first ancestor with an `Equipped` component.
pub fn find_item_owner(
    e_item: Entity,
    parent_query: &Query<&Parent, With<Equipped>>,
) -> Option<Entity> {
    parent_query.iter_ancestors(e_item).next()
}

/// On `(With<InputHandler>, With<T>)`,
/// sets the `Target` component to the user's mouse position.
pub fn set_local_mouse_target<T: Component>(
    camera_query: Query<&PlayerCamera>,
    mut item_query: Query<(&mut Target, &GlobalTransform), (With<InputHandler>, With<T>)>,
    look_info: Res<LookInfo>,
) {
    let Ok(camera) = camera_query.get_single() else {
        return;
    };

    for (mut target, g_transform) in item_query.iter_mut() {
        let target_pos = match camera.alignment {
            CameraAlignment::FortyFive => look_info
                .vertical_target_point(g_transform.translation(), g_transform.up())
                .unwrap_or_default(),
            CameraAlignment::Shooter => look_info.target_point(),
        };
        *target = Target::from_pair(g_transform.translation(), target_pos);
    }
}

/// Enables user input for this item.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct InputHandler;

/// On `(With<InputHandler>, With<T>)`,
/// - If `mouse_button` is pressed, inserts `C`.
/// - If `mouse_button` is not pressed, removes `C`.
pub fn insert_on_mouse_button<C: Component + Default>(
    commands: &mut Commands,
    entities: impl Iterator<Item = Entity>,
    mouse_buttons: &Input<MouseButton>,
    mouse_button: MouseButton,
) {
    if mouse_buttons.pressed(mouse_button) {
        for entity in entities {
            commands.entity(entity).insert(C::default());
        }
    } else {
        for entity in entities {
            commands.entity(entity).remove::<C>();
        }
    }
}

/// On `(With<InputHandler>, With<T>)`,
/// - If LMB is pressed, inserts `C`.
/// - If LMB is not pressed, removes `C`.
pub fn insert_on_lmb<T: Component, C: Component + Default>(
    mut commands: Commands,
    query: Query<Entity, (With<T>, With<InputHandler>)>,
    mouse_buttons: Res<Input<MouseButton>>,
) {
    insert_on_mouse_button::<C>(
        &mut commands,
        query.iter(),
        &mouse_buttons,
        MouseButton::Left,
    );
}

/// On `(With<InputHandler>, With<T>)`,
/// - If RMB is pressed, inserts `C`.
/// - If RMB is not pressed, removes `C`.
pub fn insert_on_rmb<T: Component, C: Component + Default>(
    mut commands: Commands,
    query: Query<Entity, (With<T>, With<InputHandler>)>,
    mouse_buttons: Res<Input<MouseButton>>,
) {
    insert_on_mouse_button::<C>(
        &mut commands,
        query.iter(),
        &mouse_buttons,
        MouseButton::Right,
    );
}

/// On `(With<InputHandler>, With<T>)`,
/// - If `HandAlignment`MB is pressed, inserts `C`.
/// - If `HandAlignment`MB is not pressed, removes `C`.
///
/// `HandAlignment::Double` detects both buttons. To configure separate buttons, use
/// `insert_on_lmb` or `insert_on_rmb`.
pub fn insert_on_hmb<T: Component, C: Component + Default>(
    mut commands: Commands,
    query: Query<(Entity, &SlotAlignment), (With<T>, With<InputHandler>)>,
    mouse_buttons: Res<Input<MouseButton>>,
) {
    insert_on_mouse_button::<C>(
        &mut commands,
        query
            .iter()
            /*me when I use `filter_map`:
            ⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡿⠿⠿⠿⠿⢿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿
            ⣿⣿⣿⣿⣿⣿⣿⣿⠟⠋⠁⠀⠀⠀⠀⠀⠀⠀⠀⠉⠻⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿
            ⣿⣿⣿⣿⣿⣿⣿⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢺⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿
            ⣿⣿⣿⣿⣿⣿⣿⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠆⠜⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿
            ⣿⣿⣿⣿⠿⠿⠛⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠉⠻⣿⣿⣿⣿⣿
            ⣿⣿⡏⠁⠀⠀⠀⠀⠀⣀⣠⣤⣤⣶⣶⣶⣶⣶⣦⣤⡄⠀⠀⠀⠀⢀⣴⣿⣿⣿⣿⣿
            ⣿⣿⣷⣄⠀⠀⠀⢠⣾⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⢿⡧⠇⢀⣤⣶⣿⣿⣿⣿⣿⣿⣿
            ⣿⣿⣿⣿⣿⣿⣾⣮⣭⣿⡻⣽⣒⠀⣤⣜⣭⠐⢐⣒⠢⢰⢸⣿⣿⣿⣿⣿⣿⣿⣿⣿
            ⣿⣿⣿⣿⣿⣿⣿⣏⣿⣿⣿⣿⣿⣿⡟⣾⣿⠂⢈⢿⣷⣞⣸⣿⣿⣿⣿⣿⣿⣿⣿⣿
            ⣿⣿⣿⣿⣿⣿⣿⣿⣽⣿⣿⣷⣶⣾⡿⠿⣿⠗⠈⢻⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿
            ⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡿⠻⠋⠉⠑⠀⠀⢘⢻⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿
            ⣿⣿⣿⣿⣿⣿⣿⡿⠟⢹⣿⣿⡇⢀⣶⣶⠴⠶⠀⠀⢽⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿
            ⣿⣿⣿⣿⣿⣿⡿⠀⠀⢸⣿⣿⠀⠀⠣⠀⠀⠀⠀⠀⡟⢿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿
            ⣿⣿⣿⡿⠟⠋⠀⠀⠀⠀⠹⣿⣧⣀⠀⠀⠀⠀⡀⣴⠁⢘⡙⢿⣿⣿⣿⣿⣿⣿⣿⣿
            ⠉⠉⠁⠀⠀⠀⠀⠀⠀⠀⠀⠈⠙⢿⠗⠂⠄⠀⣴⡟⠀⠀⡃⠀⠉⠉⠟⡿⣿⣿⣿⣿
            ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢷⠾⠛⠂⢹⠀⠀⠀⢡⠀⠀⠀⠀⠀⠙⠛⠿⢿*/
            .filter_map(|(e, h)| {
                matches!(h, SlotAlignment::Left | SlotAlignment::Double).then_some(e)
            }),
        &mouse_buttons,
        MouseButton::Left,
    );
    insert_on_mouse_button::<C>(
        &mut commands,
        query.iter().filter_map(|(e, h)| {
            matches!(h, SlotAlignment::Right | SlotAlignment::Double).then_some(e)
        }),
        &mouse_buttons,
        MouseButton::Right,
    );
}

#[derive(Debug)]
pub enum DamageContactError {
    EventMismatch(DamageEvent),
    ItemQueryMismatch(QueryEntityError),
    NoContactPair(Entity, Entity),
    NoContact(Entity, Entity),
}

/// Helper function for finding a collision point.
pub fn try_find_deepest_contact_point<T: Component>(
    damage_event: &DamageEvent,
    rapier_context: &RapierContext,
    item_query: &Query<&GlobalTransform, With<T>>,
) -> Result<Vec3, DamageContactError> {
    let &DamageEvent::Contact {
        e_damage, e_hit, ..
    } = damage_event
    else {
        return Err(DamageContactError::EventMismatch(damage_event.clone()));
    };
    let g_item_transform = item_query
        .get(e_damage)
        .map_err(|e| DamageContactError::ItemQueryMismatch(e))?;
    let contact_pair = rapier_context
        .contact_pair(e_hit, e_damage)
        .ok_or(DamageContactError::NoContactPair(e_damage, e_hit))?;
    let contact = contact_pair
        .find_deepest_contact()
        .ok_or(DamageContactError::NoContact(e_damage, e_hit))?;
    let contact_point = g_item_transform.transform_point(contact.1.local_p1());
    Ok(contact_point)
}
