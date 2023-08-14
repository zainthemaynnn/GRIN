//! This module manages rewind logic for the game.
//! I expect to make compression optimizations to this in the future(?)
//!
//! https://youtu.be/8dinUbg2h70

use std::{collections::vec_deque::VecDeque, marker::PhantomData};

use bevy::{
    ecs::system::{EntityCommand, EntityCommands},
    prelude::*,
    utils::{HashMap, HashSet},
};

pub const FIXED_TIMESTEP_SECS: f32 = 1.0 / 60.0;

/// Dependency for `RewindComponentPlugin`.
pub struct RewindPlugin {
    // this needs to be toggleable because the stupid unit tests don't work with fixed timesteps...
    pub fixed_timestep: bool,
}

impl Default for RewindPlugin {
    fn default() -> Self {
        Self {
            fixed_timestep: true,
        }
    }
}

impl Plugin for RewindPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Frame>()
            .insert_resource(FixedTime::new_from_secs(FIXED_TIMESTEP_SECS));

        let systems = (update_frame_index, update_rewind_frames, propagate_rewinds);

        if self.fixed_timestep {
            app.add_systems(FixedUpdate, systems);
        } else {
            app.add_systems(First, systems);
        }
    }
}

/// Adding this plugin allows the component `T` to be modified by `Rewind`.
pub struct RewindComponentPlugin<T: Component + Clone> {
    pub fixed_timestep: bool,
    phantom_data: PhantomData<T>,
}

impl<T: Component + Clone> Default for RewindComponentPlugin<T> {
    fn default() -> Self {
        Self {
            fixed_timestep: true,
            phantom_data: PhantomData::default(),
        }
    }
}

impl<T: Component + Clone> Plugin for RewindComponentPlugin<T> {
    fn build(&self, app: &mut App) {
        assert!(app.is_plugin_added::<RewindPlugin>());

        app.init_resource::<EntityHistories<T>>();

        let systems = (
            add_new_histories::<T>,
            retire_frame::<T>,
            save_frame::<T>,
            initialize_rewinds::<T>.after(propagate_rewinds),
            rewind::<T>.after(update_rewind_frames),
            clear_unused_histories::<T>,
        )
            .chain()
            .before(update_frame_index);

        if self.fixed_timestep {
            app.add_systems(FixedUpdate, systems);
        } else {
            app.add_systems(First, systems);
        }
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
/// If this buffer of 10 seconds without rest is depleted then it will fall back to `Rewind.out_of_history`.
#[derive(Component, Debug, Clone)]
pub struct Rewind {
    pub frames: u32,
    pub fps: u32,
}

impl Default for Rewind {
    fn default() -> Self {
        Self { frames: 0, fps: 1 }
    }
}

/// Defines what related entities should be rewound by `Rewind` along with this one.
///
/// Unlike usual `Children`, this uses a `HashSet` instead of a `SmallVec`
/// to support a larger number of relations. This makes it unordered.
///
/// This is useful for rewinding other entities without necessarily associating their transforms
/// using the typical `Parent`, `Children` hierarchy.
/// For example, projectiles from a weapon could be in global space but can also rewind with the gun.
#[derive(Component, Default)]
pub struct TimeChildren(pub HashSet<Entity>);

/// Defines what entity this entity should rewind with.
///
/// This is useful for rewinding other entities without necessarily associating their transforms
/// using the typical `Parent`, `Children` hierarchy.
/// For example, projectiles from a weapon could be in global space but can also rewind with the gun.
#[derive(Component)]
pub struct TimeParent(pub Entity);

pub struct SetTimeParent {
    pub parent: Entity,
}

// NOTE: I don't see why in the world switching or removing time relations would ever happen
// outside of despawning, so I'm not going to implement any of the sort until then
impl EntityCommand for SetTimeParent {
    fn apply(self, entity: Entity, world: &mut World) {
        let mut parent = world.entity_mut(self.parent);
        let mut children = match parent.get_mut::<TimeChildren>() {
            Some(c) => c,
            None => parent
                .insert(TimeChildren::default())
                .get_mut::<TimeChildren>()
                .unwrap(),
        };
        children.0.insert(entity);

        let mut child = world.entity_mut(entity);
        child.insert(TimeParent(self.parent));
    }
}

/// This is a drop-in replacement for regular despawn. It also affects the time hierarchy.
pub struct Despawn;

impl EntityCommand for Despawn {
    fn apply(self, entity: Entity, world: &mut World) {
        if let Some(TimeParent(e_parent)) = world.entity_mut(entity).take::<TimeParent>() {
            let mut parent = world.entity_mut(e_parent);
            parent.get_mut::<TimeChildren>().unwrap().0.remove(&entity);
        }
        world.entity_mut(entity).despawn();
    }
}

pub trait CommandsExt {
    fn set_time_parent(&mut self, parent: Entity);
    fn time_despawn(&mut self);
}

impl<'w, 's, 'a> CommandsExt for EntityCommands<'w, 's, 'a> {
    /// Sets the `TimeParent` for this entity.
    fn set_time_parent(&mut self, parent: Entity) {
        self.add(SetTimeParent { parent });
    }

    /// Despawns the entity and resolves the time hierarchy.
    /// **Prefer this over `despawn`.**
    fn time_despawn(&mut self) {
        self.add(Despawn);
        self.despawn();
    }
}

// can't use `iter_descendants` cause I'm using a bootleg hierarchy
pub fn propagate_rewinds(
    mut commands: Commands,
    query: Query<(Entity, &Rewind), (With<TimeChildren>, Added<Rewind>)>,
    children_query: Query<&TimeChildren>,
) {
    for (entity, rewind) in query.iter() {
        for child in children_query.get(entity).unwrap().0.iter() {
            propagate_rewinds_child(&mut commands, &children_query, rewind, *child);
        }
    }
}

pub fn propagate_rewinds_child(
    commands: &mut Commands,
    children_query: &Query<&TimeChildren>,
    rewind: &Rewind,
    entity: Entity,
) {
    commands.entity(entity).insert(rewind.clone());

    if let Ok(children) = children_query.get(entity) {
        for child in children.0.iter() {
            propagate_rewinds_child(commands, children_query, rewind, *child);
        }
    }
}

/// Decides what to do with the entity if `Rewind` reaches the beginning of its history.
#[derive(Component, Debug, Default, Copy, Clone)]
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
    for history in timestamps
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
                        match history.frames.back().unwrap() {
                            Timestamp::Existent(f) => Timestamp::Existent(*f),
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
            Err(..) => match history.frames.back().unwrap() {
                Timestamp::Existent(..) => Timestamp::Nonexistent(frame_time.0),
                Timestamp::Nonexistent(f) => Timestamp::Nonexistent(*f),
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
    frame_time: Res<Frame>,
    query: Query<(Entity, &Rewind, Option<&OutOfHistory>)>,
    t_query: Query<&T, With<Rewind>>,
    mut histories: ResMut<EntityHistories<T>>,
) {
    for (entity, rewind, out_of_history) in query.iter() {
        let history = histories.0.get_mut(&entity).unwrap();
        // need to track this in a variable instead of query since buffers aren't updated within system
        let mut despawned = false;
        // process each frame per `fps`
        // could probably skip the intermediate frames but eh
        for _ in 0..rewind.fps {
            match history.frames.pop_back() {
                // go back to the previous frame
                Some(frame) => match frame {
                    // component was deleted in the next frame
                    // recreate it
                    Timestamp::Existent(frame) if frame != history.rendered_frame => {
                        commands.entity(entity).insert(
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
                        commands.entity(entity).remove::<T>();
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
                        warn!("`History` ran out of frames!");
                    }

                    match out_of_history.copied().unwrap_or_default() {
                        OutOfHistory::Pause => (),
                        OutOfHistory::Resume => {
                            commands.entity(entity).remove::<Rewind>();
                        }
                        OutOfHistory::Despawn => {
                            commands.entity(entity).despawn();
                            despawned = true;
                        }
                    }
                }
            }
        }

        // finished
        if rewind.frames == 0 {
            if let Some(mut e) = commands.get_entity(entity) {
                e.remove::<Rewind>();
            }
            // return the existing component to component storage
            // this could be the same one removed during rewind initalization,
            // or a new one that it rewound to
            if !despawned {
                if let Ok(t) = t_query.get(entity) {
                    history.components.push_back(t.clone());
                    // make sure at least one frame exists to accompany this component
                    // otherwise we get errors
                    if history.frames.is_empty() {
                        history.frames.push_back(Timestamp::Existent(frame_time.0));
                        history.rendered_frame = frame_time.0;
                    }
                }
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

    fn mock_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            RewindPlugin {
                fixed_timestep: false,
            },
            RewindComponentPlugin::<MockComponent> {
                fixed_timestep: false,
                ..Default::default()
            },
        ));
        app
    }

    fn add_mutations(app: &mut App) {
        app.add_systems(
            First,
            mutate_component.before(add_new_histories::<MockComponent>),
        );
    }

    #[test]
    fn history() {
        let mut app = mock_app();

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

        add_mutations(&mut app);

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
        let mut app = mock_app();
        add_mutations(&mut app);

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
        let mut app = mock_app();

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
        let mut app = mock_app();

        let e = app.world.spawn(MockComponent::default()).id();
        app.update();
        app.world.entity_mut(e).remove::<MockComponent>();
        app.update();
        app.world.entity_mut(e).insert(MockComponent::default());
        app.update();
        app.update();
        add_mutations(&mut app);
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
        let mut app = mock_app();

        let e = app.world.spawn(MockComponent::default()).id();

        app.world
            .entity_mut(e)
            .insert((Rewind { frames: 1, fps: 1 }, OutOfHistory::Resume));
        app.update();

        assert!(app.world.get::<Rewind>(e).is_none());

        app.world
            .entity_mut(e)
            .insert((Rewind { frames: 2, fps: 1 }, OutOfHistory::Pause));
        app.update();

        assert!(app.world.get::<Rewind>(e).is_some());

        app.update();

        app.world
            .entity_mut(e)
            .insert((Rewind { frames: 5, fps: 1 }, OutOfHistory::Despawn));

        app.update();
        app.update();

        assert!(app.world.get_entity(e).is_none());
    }
}
