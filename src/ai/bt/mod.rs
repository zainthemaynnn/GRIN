pub mod tree;

use std::{marker::PhantomData, mem};

use bevy::ecs::schedule::ScheduleLabel;
use bevy::prelude::*;

use self::tree::{BehaviorOutput, BehaviorTree, OutVerdict};

pub trait Action: Component + std::fmt::Debug + Clone {}

impl<T: Component + std::fmt::Debug + Clone> Action for T {}

/// Node status. I called it `Verdict` instead of `Status` because status ambiguously
/// has like 200 different meanings... not cause I'm quirky or anything.
#[derive(Component, Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Verdict {
    #[default]
    Success,
    Failure,
    Running,
}

impl From<OutVerdict> for Verdict {
    fn from(value: OutVerdict) -> Self {
        match value {
            OutVerdict::Success => Verdict::Success,
            OutVerdict::Failure => Verdict::Failure,
        }
    }
}

#[derive(Debug)]
pub struct VerdictCastError;

impl TryFrom<Verdict> for OutVerdict {
    type Error = VerdictCastError;

    fn try_from(value: Verdict) -> Result<Self, Self::Error> {
        Ok(match value {
            Verdict::Success => OutVerdict::Success,
            Verdict::Failure => OutVerdict::Failure,
            Verdict::Running => Err(VerdictCastError)?,
        })
    }
}

#[derive(SystemSet, Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub enum BehaviorSet {
    /// Respond and write output of selected behavior tree nodes.
    Act,
    /// Calculate the next behavior tree node.
    Think,
}

// I have no idea how this hits performance. I am pretty sure it just means that
// every i'th iteration for every type of behavior tree will run in parallel,
// but distinctly numbered iterations will not.
pub fn ai_schedule_runner(world: &mut World) {
    world.run_schedule(PreBehaviorIteration);

    // run until all `ActiveTree`s are gone
    while {
        let (this_run, last_run) = (world.change_tick(), world.last_change_tick());
        !world
            .query::<&ActiveTree>()
            .is_empty(world, last_run, this_run)
    } {
        world.run_schedule(BehaviorIteration);
    }
}

/// Runs once per frame before `BehaviorIteration`.
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PreBehaviorIteration;

/// Runs multiple times per frame until all trees have either finished or returned
/// `Verdict::Running`.
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct BehaviorIteration;

/// Runs the `PreBehaviorIteration` and `BehaviorIteration` schedules.
pub struct MasterBehaviorPlugin;

impl Plugin for MasterBehaviorPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            BehaviorIteration,
            // think before you act? not here.
            (BehaviorSet::Act, BehaviorSet::Think).chain(),
        )
        .add_systems(Update, ai_schedule_runner)
        .add_systems(PreBehaviorIteration, init_behavior_update);
    }
}

/// Implements behavior tree iteration for the corresponding action `A`. Make sure
/// that the `AiModel<A>` resource exists, otherwise the app will panic.
pub struct BehaviorPlugin<A: Action> {
    pub phantom_data: PhantomData<A>,
}

impl<A: Action> Default for BehaviorPlugin<A> {
    fn default() -> Self {
        Self {
            phantom_data: PhantomData::default(),
        }
    }
}

impl<A: Action> Plugin for BehaviorPlugin<A> {
    fn build(&self, app: &mut App) {
        app.add_systems(
            BehaviorIteration,
            behavior_update::<A>.in_set(BehaviorSet::Think),
        );
    }
}

/// Contains a behavior tree corresponding to action `A`. This AI model can be shared
/// between any NPC's with a `BrainMarker<A>`, as long as the generic matches.
#[derive(Resource)]
pub struct AiModel<A: Action> {
    pub bt: BehaviorTree<A>,
}

/// The generic `A` dictates the type of AI model followed by `Brain`.
///
/// It's a separate component so that `Brain` doesn't need a generic. Otherwise you
/// pretty much always want both at once. See `BrainBundle`.
#[derive(Component)]
pub struct BrainMarker<A: Action> {
    pub phantom_data: PhantomData<A>,
}

impl<A: Action> Default for BrainMarker<A> {
    fn default() -> Self {
        Self {
            phantom_data: PhantomData::default(),
        }
    }
}

/// Facilitates interaction between an agent and its `AiModel<A>`. You can select the
/// behavior tree by inserting this with a `BrainMarker<A>`. Also see `BrainBundle`.
#[derive(Component, Debug, Default)]
pub struct Brain {
    visiting_node: usize,
    verdict: Verdict,
    changed: bool,
}

impl Brain {
    /// Currently selected node status.
    pub fn verdict(&self) -> Verdict {
        self.verdict
    }

    /// Write the output for the currently selected behavior tree node. This needs to be done
    /// immediately after the node is selected, otherwise the tree will be disabled.
    pub fn write_verdict(&mut self, verdict: Verdict) {
        self.changed = true;
        self.verdict = verdict;
    }

    /// Whether `verdict` has been modified since the last `pop_changed`.
    fn pop_changed(&mut self) -> bool {
        mem::replace(&mut self.changed, false)
    }
}

#[derive(Bundle)]
pub struct BrainBundle<A: Action> {
    pub brain: Brain,
    pub marker: BrainMarker<A>,
}

impl<A: Action> Default for BrainBundle<A> {
    fn default() -> Self {
        Self {
            brain: Brain::default(),
            marker: BrainMarker::default(),
        }
    }
}

/// Trees that are still iterating this frame.
#[derive(Component)]
pub struct ActiveTree;

/// Reactives all behavior trees for the current frame.
pub fn init_behavior_update(mut commands: Commands, mut agent_query: Query<Entity, With<Brain>>) {
    for e_agent in agent_query.iter_mut() {
        commands.entity(e_agent).insert(ActiveTree);
    }
}

/// Updates all behavior trees until the next task/root node.
pub fn behavior_update<A: Action>(
    mut commands: Commands,
    ai: Res<AiModel<A>>,
    mut agent_query: Query<
        (Entity, &mut Brain, Option<&mut A>),
        (With<ActiveTree>, With<BrainMarker<A>>),
    >,
) {
    for (e_agent, mut brain, action) in agent_query.iter_mut() {
        if action.is_some() && !brain.pop_changed() {
            warn!("{:?} was not handled. Make sure to handle this with `Brain::write_status`. Terminating tree...", action);
            commands
                .entity(e_agent)
                .remove::<(ActiveTree, BrainBundle<A>)>();
            continue;
        }

        if let Some(mut action) = action {
            match brain.verdict() {
                Verdict::Success | Verdict::Failure => match ai
                    .bt
                    .run_leaf(brain.visiting_node, brain.verdict().try_into().unwrap())
                {
                    // don't deactivate the tree for further iteration steps
                    BehaviorOutput::Task {
                        node,
                        action: new_action,
                    } => {
                        debug!("Running action {:?}.", new_action);
                        brain.visiting_node = node;
                        *action = new_action;
                    }
                    // deactivate, remove the action, and set `visiting_node` to root
                    BehaviorOutput::Complete { verdict } => {
                        brain.visiting_node = 0;
                        debug!("Tree finished with {:?}.", verdict);
                        commands.entity(e_agent).remove::<(ActiveTree, A)>();
                    }
                },
                // temporarily deactivate but keep `visiting_node`, so that it starts
                // at the same node next frame
                Verdict::Running => {
                    debug!("Deferring action {:?}.", *action);
                    commands.entity(e_agent).remove::<ActiveTree>();
                }
            }
        } else {
            // there's no action so the tree needs to be restarted. call `run_root`.
            let BehaviorOutput::Task { node, action } = ai.bt.run_root() else {
                error!("Behavior tree finished without task.");
                continue;
            };

            debug!("Running action {:?}.", action);
            brain.visiting_node = node;
            commands.entity(e_agent).insert(action);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{ai::bt::tree::CompositeNode, bt};

    use super::*;

    #[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
    pub enum MockTask {
        A,
        B,
    }

    fn defer_3_succeed_1_fail_1_system(
        mut agent_query: Query<(&MockTask, &mut Brain)>,
        mut switch: Local<u32>,
    ) {
        for (action, mut brain) in agent_query.iter_mut() {
            brain.write_verdict(match action {
                MockTask::A => {
                    if *switch == 3 {
                        Verdict::Success
                    } else {
                        *switch += 1;
                        Verdict::Running
                    }
                }
                MockTask::B => Verdict::Failure,
            });
        }
    }

    // what else do you want me to call it?
    #[test]
    fn defer_3_succeed_1_fail_1() {
        let mut app = App::default();
        app.add_plugins((MasterBehaviorPlugin, BehaviorPlugin::<MockTask>::default()))
            .insert_resource(AiModel {
                bt: bt! {
                    Composite(CompositeNode::Sequence) {
                        Leaf(MockTask::A),
                        Leaf(MockTask::B),
                    },
                },
            })
            .add_systems(
                BehaviorIteration,
                defer_3_succeed_1_fail_1_system.in_set(BehaviorSet::Act),
            );

        let e_agent = app.world.spawn(BrainBundle::<MockTask>::default()).id();

        for _ in 0..3 {
            app.update();

            let action = app.world.entity(e_agent).get::<MockTask>();
            assert_eq!(action, Some(&MockTask::A));
        }

        app.update();

        let action = app.world.entity(e_agent).get::<MockTask>();
        assert_eq!(action, None);
    }

    #[test]
    fn kill_unhandled_trees() {
        let mut app = App::default();
        app.add_plugins((MasterBehaviorPlugin, BehaviorPlugin::<MockTask>::default()))
            .insert_resource(AiModel {
                bt: bt! {
                    Leaf(MockTask::A),
                },
            });

        let e_agent = app.world.spawn(BrainBundle::<MockTask>::default()).id();

        app.update();

        let brain = app.world.entity(e_agent).get::<Brain>();
        assert!(brain.is_none());
    }
}
