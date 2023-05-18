//! This module manages rewind logic for the game.
//! I expect to make compression optimizations to this in the future(?)
//!
//! https://youtu.be/8dinUbg2h70

use std::{collections::vec_deque::VecDeque, marker::PhantomData};

use bevy::{prelude::*, utils::HashMap};

pub const FIXED_TIMESTEP_SECS: f32 = 1.0 / 24.0;

/// Dependency for `RewindComponentPlugin`.
#[derive(Default)]
pub struct RewindPlugin;

impl Plugin for RewindPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Frame>()
            .add_systems((update_frame_index, update_rewind_frames).in_base_set(CoreSet::First));
    }
}

/// Adding this plugin allows the component `T` to be modified by `Rewind`.
#[derive(Default)]
pub struct RewindComponentPlugin<T: Component + Clone> {
    phantom_data: PhantomData<T>,
}

impl<T: Component + Clone> Plugin for RewindComponentPlugin<T> {
    fn build(&self, app: &mut App) {
        app.init_resource::<EntityHistories<T>>()
            .insert_resource(FixedTime::new_from_secs(FIXED_TIMESTEP_SECS))
            .add_systems(
                (
                    add_new_histories::<T>,
                    retire_frame::<T>,
                    save_frame::<T>,
                    initialize_rewinds::<T>,
                    rewind::<T>.after(update_rewind_frames),
                    apply_system_buffers,
                    clear_unused_histories::<T>,
                )
                    .chain()
                    .before(update_frame_index)
                    .in_base_set(CoreSet::First), //.in_schedule(CoreSchedule::FixedUpdate),
            );
    }
}

/// Causes the entity to step back in time a total of `Rewind.frames` frames, at `fps` frames per frame.
/// This only applies to components that have been registered with a `TimeStepPlugin<T>`.
///
/// Rewinding components occurs in `CoreSchedule::First` and frames are recorded at a fixed timestep.
/// Thus rewinding may occur at a lower fps than other updates. An fps lower than the timestep will lead
/// to rewinds going for longer than anticipated, which I may or may not fix at some point.
///
/// Mutating components during this time will cause undefined behavior,
/// since modifications may be overwritten at random times.
///
/// Be careful using this too often or for extended periods of time.
/// At the moment entities only store about 10 seconds worth of history,
/// and history isn't recorded during rewinding.
/// If this buffer of 10 seconds without rest is depleted then it will fall back to `Rewind.OutOfHistory`.
#[derive(Component, Debug)]
pub struct Rewind {
    pub frames: u32,
    pub fps: u32,
    pub out_of_history: OutOfHistory,
}

impl Default for Rewind {
    fn default() -> Self {
        Self {
            frames: 0,
            fps: 1,
            out_of_history: OutOfHistory::default(),
        }
    }
}

/// Decides what to do with the entity if `Rewind` reaches the beginning of its history.
#[derive(Debug, Default)]
pub enum OutOfHistory {
    /// Terminates rewinding but keeps the `Rewind` component for the remaining frames.
    Pause,
    /// Terminates rewinding and removes the `Rewind` component.
    #[default]
    Resume,
    /// Despawns the entity.
    Despawn,
}

/// How many frames of the game have ran.
#[derive(Resource, Default, Debug)]
pub struct Frame(pub usize);

pub fn update_frame_index(mut frame_index: ResMut<Frame>) {
    frame_index.0 += 1;
}

/// Stores `History`s for all `Entity`s with component `T`.
///
/// Histories are automatically added and removed from this resource.
/// An entity may be removed on one of two conditions:
/// - The entity goes "cold;" the entity has not contained this component for a certain number of frames.
/// - The entity was despawned from the world.
#[derive(Resource, Debug)]
pub struct EntityHistories<T: Component>(HashMap<Entity, History<T>>);

impl<T: Component> Default for EntityHistories<T> {
    fn default() -> Self {
        Self(HashMap::default())
    }
}

/// Stores the `Timestamp`s for component `T` over the last `History::MAX_STORAGE_FRAMES` frames.
#[derive(Debug)]
pub struct History<T: Component> {
    /// `Timestamp`s.
    pub frames: VecDeque<Timestamp>,
    /// `Component`s. Each value of a `Timestamp::Existent` refers to a state of the component from this deque,
    /// where the component at earlier timestamps is located at the front of the deque.
    pub components: VecDeque<T>,
    /// How frames are being stored in `History.frames`. The history will begin forgetting frames
    /// after reaching `History::MAX_STORAGE_FRAMES`, and the storage state will change permanently after this point.
    pub storage_state: HistoryStorageState,
    /// The first frame for which the component had this state.
    /// This prevents the component from being created or removed multiple times
    /// with the same data when rewinding.
    pub rendered_frame: usize,
}

/// Describes how frames are being stored in a `History`. For debugging.
#[derive(Debug, Eq, PartialEq)]
pub enum HistoryStorageState {
    /// It has never been full; frames are being appended.
    Growing,
    /// It has been full and leaked a frame. This doesn't mean it's full right now.
    Leaking,
}

impl<T: Component> History<T> {
    /// The maximum number of frames to store before forgetting values.
    const MAX_STORAGE_FRAMES: usize = 600;
}

impl<T: Component> Default for History<T> {
    fn default() -> Self {
        Self {
            frames: VecDeque::with_capacity(Self::MAX_STORAGE_FRAMES),
            components: VecDeque::new(),
            storage_state: HistoryStorageState::Growing,
            rendered_frame: 0,
        }
    }
}

/// Describes the status of a component at a timestamp.
///
/// Timestamps of different values represent different component states.
/// i.e. if one frame has `Timestamp::Existent(1)` and the next `Timestamp::Existent(2)`
/// then the component must have been mutated in between.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Timestamp {
    /// Entity had this component at this timestamp.
    Existent(usize),
    /// Entity didn't have this component at this timestamp.
    Nonexistent(usize),
}

/// Forgets the oldest frame in a `History` if it's out of memory space.
pub fn retire_frame<T: Component>(mut timestamps: ResMut<EntityHistories<T>>) {
    for mut history in timestamps
        .0
        .values_mut()
        .filter(|h| h.frames.len() == History::<T>::MAX_STORAGE_FRAMES)
    {
        let old_frame = history.frames.pop_front().unwrap();
        let new_frame = history.frames.front_mut().unwrap();

        // if the component state referred to by the deleted frame is overwritten,
        // remove it from the queue forever
        if let Timestamp::Existent(old_frame_time) = old_frame {
            let (Timestamp::Existent(new_frame_time) | Timestamp::Nonexistent(new_frame_time)) =
                new_frame;
            // different frame time => different component state
            if old_frame_time != *new_frame_time {
                history.components.pop_front();
            }
        }

        history.storage_state = HistoryStorageState::Leaking;
    }
}

/// Adds histories for `Entity`s containing `T`.
pub fn add_new_histories<T: Component>(
    mut histories: ResMut<EntityHistories<T>>,
    query: Query<Entity, With<T>>,
) {
    for entity in query.iter() {
        if histories.0.get(&entity).is_none() {
            histories.0.insert(entity, History::default());
        }
    }
}

/// Takes a snapshot of the state of `T` for all entities.
pub fn save_frame<T: Component + Clone>(
    mut histories: ResMut<EntityHistories<T>>,
    frame_time: Res<Frame>,
    t_query: Query<Ref<T>>,
    rewound_query: Query<(), With<Rewind>>,
) {
    for (entity, history) in histories.0.iter_mut() {
        // entities being rewound aren't included in history
        if rewound_query.get(*entity).is_ok() {
            continue;
        }

        let timestamp = match t_query.get(*entity) {
            // component exists
            Ok(t) => {
                match t.is_changed() {
                    // exists and modified; change the component
                    true => {
                        history.components.push_back(t.clone());
                        Timestamp::Existent(frame_time.0)
                    }
                    false => {
                        // use whatever frame index is recorded in the previous frame
                        match *history
                            .frames
                            .back()
                            .expect("Component appears unmodified but frame history is empty.")
                        {
                            Timestamp::Existent(f) => Timestamp::Existent(f),
                            Timestamp::Nonexistent(..) => {
                                panic!(
                                    "Component appears unmodified but nonexistent in previous frame.",
                                );
                            }
                        }
                    }
                }
            }
            // does not exist
            Err(..) => match *history
                .frames
                .back()
                .expect("Component appears nonexistent but frame history is empty.")
            {
                Timestamp::Existent(..) => Timestamp::Nonexistent(frame_time.0),
                Timestamp::Nonexistent(f) => Timestamp::Nonexistent(f),
            },
        };

        let (Timestamp::Existent(f) | Timestamp::Nonexistent(f)) = timestamp;
        history.rendered_frame = f;

        history.frames.push_back(timestamp);
    }
}

/// Delete all `History`s with no existing component and no components in storage.
///
/// These histories are safe to remove because there is no useful data left.
/// All values have been either rewound or forgotten.
/// A new history is guaranteed to be stored with its initial value
/// so it won't be deleted.
pub fn clear_unused_histories<T: Component>(
    mut histories: ResMut<EntityHistories<T>>,
    entity_query: Query<Entity>,
    t_query: Query<(), With<T>>,
) {
    histories.0.retain(|entity, history| {
        entity_query.get(*entity).is_ok()
            && (!history.components.is_empty() || t_query.get(*entity).is_ok())
    });
}

/// Removes the latest component from the component storage buffer, if `Timestamp::Existent`.
///
/// This is because the latest component in storage is already on the entity
/// and rewinding would cause the same one to be copied.
pub fn initialize_rewinds<T: Component>(
    query: Query<Entity, Added<Rewind>>,
    mut histories: ResMut<EntityHistories<T>>,
) {
    for entity in query.iter() {
        let history = histories.0.get_mut(&entity).unwrap();
        if let Some(Timestamp::Existent(..)) = history.frames.back() {
            history.components.pop_back();
        }
    }
}

/// Sets `fps` to the number of frames to render in this stage, which may change if `fps` > `frames`.
///
/// Sets `frames` to the number of frames left after this tick is finished.
pub fn update_rewind_frames(mut query: Query<&mut Rewind>) {
    for mut rewind in query.iter_mut() {
        rewind.fps = rewind.fps.min(rewind.frames);
        rewind.frames -= rewind.fps;
    }
}

/// Rewinds components.
pub fn rewind<T: Component + Clone>(
    mut commands: Commands,
    query: Query<(Entity, &Rewind)>,
    t_query: Query<&T, With<Rewind>>,
    mut histories: ResMut<EntityHistories<T>>,
) {
    for (entity, rewind) in query.iter() {
        let history = histories.0.get_mut(&entity).unwrap();

        // process each frame per `fps`
        // could probably skip the intermediate frames but eh
        for _ in 0..rewind.fps {
            match history.frames.pop_back() {
                // go back to the previous frame
                Some(frame) => match frame {
                    // component was deleted in the next frame
                    // recreate it
                    Timestamp::Existent(frame) if frame != history.rendered_frame => {
                        commands.get_or_spawn(entity).insert(
                            history
                                .components
                                .pop_back()
                                .expect("No past components in frame queue."),
                        );
                        history.rendered_frame = frame;
                    }
                    // component was created in the next frame
                    // delete it
                    Timestamp::Nonexistent(frame) if frame != history.rendered_frame => {
                        commands.get_or_spawn(entity).remove::<T>();
                        history.rendered_frame = frame;
                    }
                    _ => (),
                },
                // no `Timestamp`s are stored in the frame buffer
                // this could be either normal (entity was recently created and now deleted)
                // or it could mean that the frame queue ran out due to too much use of `Rewind`
                // we can verify this by seeing if it has ever leaked a frame
                None => {
                    // if it leaked and data was lost, it's a problem
                    if history.storage_state == HistoryStorageState::Leaking {
                        println!("`History` ran out of frames!");
                    }

                    match rewind.out_of_history {
                        OutOfHistory::Pause => (),
                        OutOfHistory::Resume => {
                            commands.get_or_spawn(entity).remove::<Rewind>();
                        }
                        OutOfHistory::Despawn => {
                            commands.get_or_spawn(entity).despawn();
                        }
                    }
                }
            }
        }

        // finished
        if rewind.frames == 0 {
            commands.get_or_spawn(entity).remove::<Rewind>();
            // return the existing component to component storage
            // this could be the same one removed during rewind initalization,
            // or a new one that it rewound to
            if let Ok(t) = t_query.get(entity) {
                history.components.push_back(t.clone());
            }
        }
    }
}

// god, this was painful
#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Component, Default, Clone, Debug)]
    struct MockComponent(u32);

    fn mutate_component(mut cmpt_query: Query<&mut MockComponent>) {
        for mut cmpt in cmpt_query.iter_mut() {
            cmpt.0 += 1;
        }
    }

    #[test]
    fn history() {
        let mut app = App::new();
        app.add_plugin(RewindPlugin);
        app.add_plugin(RewindComponentPlugin::<MockComponent>::default());
        app.insert_resource(FixedTime::new_from_secs(0.0));

        let e = app.world.spawn(MockComponent::default()).id();

        app.update();

        // should add a history for `e` with one component and timestamp
        let histories = app.world.resource::<EntityHistories<MockComponent>>();
        let history = histories.0.get(&e).unwrap();
        assert_eq!(*history.frames.back().unwrap(), Timestamp::Existent(0));
        assert_eq!(history.components.len(), 1);

        app.update();

        // should refer to previous timestamp
        let histories = app.world.resource::<EntityHistories<MockComponent>>();
        let history = histories.0.get(&e).unwrap();
        assert_eq!(*history.frames.back().unwrap(), Timestamp::Existent(0));

        app.add_system(
            mutate_component
                .before(add_new_histories::<MockComponent>)
                .in_base_set(CoreSet::First),
        );

        app.update();

        // should have an additional modified component with new timestamp
        let histories = app.world.resource::<EntityHistories<MockComponent>>();
        let history = histories.0.get(&e).unwrap();
        assert_eq!(*history.frames.back().unwrap(), Timestamp::Existent(2));
        assert_eq!(history.components.len(), 2);

        app.world.entity_mut(e).remove::<MockComponent>();

        app.update();

        // should show that the component has been removed with new timestamp
        let histories = app.world.resource::<EntityHistories<MockComponent>>();
        let history = histories.0.get(&e).unwrap();
        assert_eq!(*history.frames.back().unwrap(), Timestamp::Nonexistent(3));

        app.update();

        // should refer to previous timestamp
        let histories = app.world.resource::<EntityHistories<MockComponent>>();
        let history = histories.0.get(&e).unwrap();
        assert_eq!(*history.frames.back().unwrap(), Timestamp::Nonexistent(3));
    }

    #[test]
    fn history_forget() {
        let mut app = App::new();
        app.add_plugin(RewindPlugin);
        app.add_plugin(RewindComponentPlugin::<MockComponent>::default());
        app.insert_resource(FixedTime::new_from_secs(0.0));
        app.add_system(
            mutate_component
                .before(add_new_histories::<MockComponent>)
                .in_base_set(CoreSet::First),
        );

        let e = app.world.spawn(MockComponent::default()).id();

        app.update();
        app.update();

        app.world.entity_mut(e).remove::<MockComponent>();

        for _ in 0..(History::<MockComponent>::MAX_STORAGE_FRAMES - 2) {
            app.update();
        }

        // history should be at max capacity but functioning with two different components stored
        let histories = app.world.resource::<EntityHistories<MockComponent>>();
        let history = histories.0.get(&e).unwrap();
        assert_eq!(history.storage_state, HistoryStorageState::Growing);
        assert_eq!(
            history.frames.len(),
            History::<MockComponent>::MAX_STORAGE_FRAMES
        );
        assert_eq!(history.components.len(), 2);

        app.update();

        // should have forgotten one entry
        let histories = app.world.resource::<EntityHistories<MockComponent>>();
        let history = histories.0.get(&e).unwrap();
        assert_eq!(history.storage_state, HistoryStorageState::Leaking);
        assert_eq!(
            history.frames.len(),
            History::<MockComponent>::MAX_STORAGE_FRAMES
        );
        assert_eq!(history.components.len(), 1);
    }

    #[test]
    fn history_cleanup() {
        let mut app = App::new();
        app.add_plugin(RewindPlugin);
        app.add_plugin(RewindComponentPlugin::<MockComponent>::default());
        app.insert_resource(FixedTime::new_from_secs(0.0));

        let e = app.world.spawn(MockComponent::default()).id();

        app.update();

        app.world.entity_mut(e).remove::<MockComponent>();

        for _ in 0..(History::<MockComponent>::MAX_STORAGE_FRAMES - 1) {
            app.update();
        }

        // history should drop when all `Existent` frames are forgotten
        let histories = app.world.resource::<EntityHistories<MockComponent>>();
        let cold_history = histories.0.get(&e);
        assert!(cold_history.is_some());

        app.update();

        let histories = app.world.resource::<EntityHistories<MockComponent>>();
        let cold_history = histories.0.get(&e);
        assert!(cold_history.is_none());

        let e = app.world.spawn(MockComponent::default()).id();

        app.update();

        // history should drop when entity despawns
        let histories = app.world.resource::<EntityHistories<MockComponent>>();
        let despawned_history = histories.0.get(&e);
        assert!(despawned_history.is_some());

        app.world.despawn(e);

        app.update();

        let histories = app.world.resource::<EntityHistories<MockComponent>>();
        let despawned_history = histories.0.get(&e);
        assert!(despawned_history.is_none());
    }

    #[test]
    fn rewind() {
        let mut app = App::new();
        app.add_plugin(RewindPlugin);
        app.add_plugin(RewindComponentPlugin::<MockComponent>::default());
        app.insert_resource(FixedTime::new_from_secs(0.0));

        let e = app.world.spawn(MockComponent::default()).id();
        app.update();
        app.world.entity_mut(e).remove::<MockComponent>();
        app.update();
        app.world.entity_mut(e).insert(MockComponent::default());
        app.update();
        app.update();
        app.add_system(
            mutate_component
                .before(add_new_histories::<MockComponent>)
                .in_base_set(CoreSet::First),
        );
        app.update();
        // expected final state:
        // `components.len() == 3`
        // `frames == [E(0), NE(1), E(2), E(2), E(4)]`

        let histories = app.world.resource::<EntityHistories<MockComponent>>();
        let history = histories.0.get(&e).unwrap();
        assert_eq!(history.components.len(), 3);
        assert_eq!(app.world.get::<MockComponent>(e).unwrap().0, 1);

        let prev_rendered_frame = history.rendered_frame;
        app.world.entity_mut(e).insert(Rewind {
            frames: 5,
            ..Default::default()
        });
        app.update();

        let histories = app.world.resource::<EntityHistories<MockComponent>>();
        let history = histories.0.get(&e).unwrap();
        assert_eq!(history.components.len(), 2);

        app.update();

        let histories = app.world.resource::<EntityHistories<MockComponent>>();
        let history = histories.0.get(&e).unwrap();
        assert_eq!(history.components.len(), 1);
        assert_eq!(app.world.get::<MockComponent>(e).unwrap().0, 0);
        assert_ne!(prev_rendered_frame, history.rendered_frame);

        let prev_rendered_frame = history.rendered_frame;
        app.update();

        let histories = app.world.resource::<EntityHistories<MockComponent>>();
        let history = histories.0.get(&e).unwrap();
        assert_eq!(history.components.len(), 1);
        assert_eq!(prev_rendered_frame, history.rendered_frame);

        app.update();

        assert!(app.world.get::<MockComponent>(e).is_none());

        app.update();

        assert!(app.world.get::<MockComponent>(e).is_some());
        assert!(app.world.get::<Rewind>(e).is_none());
    }

    #[test]
    fn out_of_history() {
        let mut app = App::new();
        app.add_plugin(RewindPlugin);
        app.add_plugin(RewindComponentPlugin::<MockComponent>::default());
        app.insert_resource(FixedTime::new_from_secs(0.0));

        let e = app.world.spawn(MockComponent::default()).id();

        app.world.entity_mut(e).insert(Rewind {
            frames: 1,
            fps: 1,
            out_of_history: OutOfHistory::Resume,
        });
        app.update();

        assert!(app.world.get::<Rewind>(e).is_none());

        app.world.entity_mut(e).insert(Rewind {
            frames: 2,
            fps: 1,
            out_of_history: OutOfHistory::Pause,
        });
        app.update();

        assert!(app.world.get::<Rewind>(e).is_some());

        app.update();

        app.world.entity_mut(e).insert(Rewind {
            frames: 5,
            fps: 1,
            out_of_history: OutOfHistory::Despawn,
        });

        app.update();

        assert!(app.world.get_entity(e).is_none());
    }
}
