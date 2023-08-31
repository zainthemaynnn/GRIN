//! Behavior tree (no bevy stuff here).

/// `Verdict`, without `Running`. For tree traversal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutVerdict {
    Success,
    Failure,
}

/// Used when moving between behavior tree nodes. It helps avoid recursion.
enum NodeTraversal<A: Clone> {
    /// Traverse down the tree.
    Down { node: usize },
    /// Traverse up the tree, with this status.
    Up { node: usize, verdict: OutVerdict },
    /// Stop traversing the tree at a leaf node.
    Task { action: A },
    /// Finish traversing the tree at the root node.
    Finish { verdict: OutVerdict },
}

/// Behavior tree node.
// not the most equally sized enum, but it's alright.
pub enum Node<A> {
    Root(usize),
    Composite(Composite),
    Decorator(Decorator),
    Leaf(Leaf<A>),
}

impl<A> Default for Node<A> {
    fn default() -> Self {
        Self::Root(usize::MAX)
    }
}

impl<A: Clone> Node<A> {
    /// Traverse down the behavior tree. Returns on a leaf node.
    fn down(&self) -> NodeTraversal<A> {
        match self {
            Node::Root(node) => NodeTraversal::Down { node: *node },
            Node::Composite(composite) => {
                // if we ever run down on a composite, it should start fresh from the first child
                let node = composite.children[0];
                NodeTraversal::Down { node }
            }
            Node::Decorator(decorator) => NodeTraversal::Down {
                node: decorator.child,
            },
            Node::Leaf(Leaf { action, .. }) => NodeTraversal::Task {
                action: action.clone(),
            },
        }
    }

    /// Traverse up the behavior tree. Returns on a leaf node or the root node if the tree is finished.
    fn up(&self, source: usize, verdict: OutVerdict) -> NodeTraversal<A> {
        match self {
            Node::Root(..) => NodeTraversal::Finish { verdict },
            Node::Composite(composite) => {
                // they use the same algorithm
                let (result, inv_result) = match composite.kind {
                    CompositeNode::Sequence => (OutVerdict::Success, OutVerdict::Failure),
                    CompositeNode::Selector => (OutVerdict::Failure, OutVerdict::Success),
                };

                if verdict == result {
                    // this operates on the principle that children always have
                    // ascending node id's
                    if let Some(&node) = composite.children.iter().find(|n| **n > source) {
                        NodeTraversal::Down { node }
                    } else {
                        NodeTraversal::Up {
                            node: composite.parent,
                            verdict: result,
                        }
                    }
                } else {
                    NodeTraversal::Up {
                        node: composite.parent,
                        verdict: inv_result,
                    }
                }
            }
            Node::Decorator(decorator) => match decorator.kind {
                DecoratorNode::Invert => NodeTraversal::Up {
                    node: decorator.parent,
                    verdict: match verdict {
                        OutVerdict::Success => OutVerdict::Failure,
                        OutVerdict::Failure => OutVerdict::Success,
                    },
                },
            },
            Node::Leaf(leaf) => NodeTraversal::Up {
                node: leaf.parent,
                verdict,
            },
        }
    }
}

pub struct Composite {
    pub kind: CompositeNode,
    pub parent: usize,
    pub children: Vec<usize>,
    pub child_index: usize,
}

impl Composite {
    pub fn new(kind: CompositeNode) -> Self {
        Self {
            kind,
            parent: usize::MAX,
            children: Vec::default(),
            child_index: 0,
        }
    }
}

pub enum CompositeNode {
    /// Iterates every child for `Verdict::Success`, but stops immediately at `Verdict::Failure`.
    Sequence,
    /// Iterates every child for `Verdict::Failure`, but stops immediately at `Verdict::Success`.
    Selector,
}

pub struct Decorator {
    pub kind: DecoratorNode,
    pub parent: usize,
    pub child: usize,
}

impl Decorator {
    pub fn new(kind: DecoratorNode) -> Self {
        Self {
            kind,
            parent: usize::MAX,
            child: usize::MAX,
        }
    }
}

pub enum DecoratorNode {
    /// Turns `Verdict::Success` into `Verdict::Failure` and vice versa.
    Invert,
}

pub struct Leaf<A> {
    pub action: A,
    pub parent: usize,
}

impl<A> Leaf<A> {
    pub fn new(action: A) -> Self {
        Self {
            action,
            parent: usize::MAX,
        }
    }
}

/// Behavior tree.
///
/// This tree is guaranteed to run from any node with any status at any time. There's no internal state.
/// It will only return the appropriate task or root node that it reaches first.
/// This property allows the tree to be shared by multiple agents.
/// It's up to the user to continue running the tree from a task node after it reaches one.
pub struct BehaviorTree<A> {
    pub graph: Vec<Node<A>>,
}

impl<A> Default for BehaviorTree<A> {
    fn default() -> Self {
        Self { graph: Vec::new() }
    }
}

#[derive(Debug)]
pub enum GraphBuildError {
    RootChild,
    LeafParent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BehaviorOutput<A> {
    /// Reached a task.
    Task { node: usize, action: A },
    /// Reached the top of the tree.
    Complete { verdict: OutVerdict },
}

impl<A: Clone> BehaviorTree<A> {
    /// Run a node traversal.
    fn traverse_at(&self, mut traversal: NodeTraversal<A>) -> BehaviorOutput<A> {
        let mut visiting_node = usize::MAX;
        loop {
            traversal = match traversal {
                NodeTraversal::Down { node } => {
                    visiting_node = node;
                    self.graph[node].down()
                }
                NodeTraversal::Up { node, verdict } => {
                    let source = visiting_node;
                    visiting_node = node;
                    self.graph[node].up(source, verdict)
                }
                // I dunno why they get formatted like this, but OK
                NodeTraversal::Task { action } => {
                    break BehaviorOutput::Task {
                        node: visiting_node,
                        action,
                    }
                }
                NodeTraversal::Finish { verdict } => break BehaviorOutput::Complete { verdict },
            }
        }
    }

    /// Run from the top of the tree.
    pub fn run_root(&self) -> BehaviorOutput<A> {
        self.traverse_at(NodeTraversal::Down { node: 0 })
    }

    /// Run from a particular node that outputted a particular verdict.
    pub fn run_leaf(&self, input_node: usize, verdict: OutVerdict) -> BehaviorOutput<A> {
        match verdict {
            OutVerdict::Success => self.traverse_at(NodeTraversal::Up {
                node: input_node,
                verdict: OutVerdict::Success,
            }),
            OutVerdict::Failure => self.traverse_at(NodeTraversal::Up {
                node: input_node,
                verdict: OutVerdict::Failure,
            }),
        }
    }

    /// Add a node. Returns the node's id.
    pub fn push_node(&mut self, node: Node<A>) -> usize {
        self.graph.push(node);
        self.graph.len() - 1
    }

    /// Create a directed edge from `from` to `to`.
    pub fn add_node_edge(&mut self, from: usize, to: usize) -> Result<(), GraphBuildError> {
        match &mut self.graph[from] {
            Node::Root(ref mut child) => {
                *child = to;
            }
            Node::Composite(ref mut composite) => {
                composite.children.push(to);
            }
            Node::Decorator(ref mut decorator) => {
                decorator.child = to;
            }
            Node::Leaf(..) => Err(GraphBuildError::LeafParent)?,
        }

        match &mut self.graph[to] {
            Node::Composite(ref mut composite) => {
                composite.parent = from;
            }
            Node::Decorator(ref mut decorator) => {
                decorator.parent = from;
            }
            Node::Leaf(ref mut leaf) => {
                leaf.parent = from;
            }
            Node::Root(..) => Err(GraphBuildError::RootChild)?,
        }

        Ok(())
    }
}

/// Behavior tree macro. Returns a `BehaviorTree`. It's a lot easier to set up and read
/// since nodes and node edges are created automatically. Also, it's impossible to make
/// a malformed hierarchy at compile time.
///
/// Usage:
/// ```
/// enum Action {
///     A,
///     B,
///     C,   
/// }
///
/// let behavior = bt! {
///     Composite(CompositeNode::Selector) {
///         Leaf(Action::A),
///         Decorator(DecoratorNode::Invert) {
///             Leaf(Action::B),
///         },
///         Leaf(Action::C),
///     }
/// };
/// ```
#[macro_export(local_inner_macros)]
macro_rules! bt {
    ( $($block:tt)* ) => {
        bt_internal!(@root $($block)*)
    }
}

/// Behavior tree macro. Needs to be annotated with `@root` to work.
/// The public version `bt!` doesn't need to be annotated.
//
// wow. learning how to make a non-trivial macro was a journey. a whole day, in fact...
#[macro_export(local_inner_macros)]
macro_rules! bt_internal {
    ( @root $($block:tt)* ) => {
        {
            let mut tree = crate::ai::bt::tree::BehaviorTree::default();
            let root = tree.push_node(crate::ai::bt::tree::Node::default());
            bt_internal!(@child tree, root, $($block)*);
            tree
        }
    };

    // I would like not having to require wrapping things with `Composite()` or `Decorator()`,
    // but it needs to be done in order to match their subtypes as an expression.
    // it's not a terrible loss though, just a nitpick. and way better than no macro at all...

    // composite
    ( @child @multi $tree:expr, $parent:expr, Composite($kind:expr$(,)?) { $($block:tt)+ }, $($tail:tt)+ ) => {
        bt_internal!(@composite $tree, $parent, $kind, $($block)+);
        bt_internal!(@child @multi $tree, $parent, $($tail)*);
    };

    // composite at end
    ( @child $(@multi)? $tree:expr, $parent:expr, Composite($kind:expr$(,)?) { $($block:tt)+ } $(,)? ) => {
        bt_internal!(@composite $tree, $parent, $kind, $($block)*);
    };

    // decorator
    ( @child @multi $tree:expr, $parent:expr, Decorator($kind:expr$(,)?) { $($block:tt)+ }, $($tail:tt)+ ) => {
        bt_internal!(@decorator $tree, $parent, $kind, $($block)+);
        bt_internal!(@child @multi $tree, $parent, $($tail)+);
    };

    // decorator at end
    ( @child $(@multi)? $tree:expr, $parent:expr, Decorator($kind:expr$(,)?) { $($block:tt)+ } $(,)? ) => {
        bt_internal!(@decorator $tree, $parent, $kind, $($block)+);
    };

    // leaf
    ( @child @multi $tree:expr, $parent:expr, Leaf($task:expr$(,)?), $($tail:tt)+ ) => {
        bt_internal!(@leaf $tree, $parent, $task);
        bt_internal!(@child @multi $tree, $parent, $($tail)+);
    };

    // leaf at end
    ( @child $(@multi)? $tree:expr, $parent:expr, Leaf($action:expr$(,)?) $(,)? ) => {
        bt_internal!(@leaf $tree, $parent, $action);
    };

    // create composite node
    ( @composite $tree:expr, $parent:expr, $kind:expr, $($block:tt)+ ) => {
        let composite = $tree.push_node(
            crate::ai::bt::tree::Node::Composite(
                crate::ai::bt::tree::Composite::new($kind),
            ),
        );
        // adding @multi uses tt munching to allow multiple children
        // don't want it on the single-child nodes so that they error
        bt_internal!(@child @multi $tree, composite, $($block)+);
        $tree.add_node_edge($parent, composite).unwrap();
    };

    // create decorator node
    ( @decorator $tree:expr, $parent:expr, $kind:expr, $($block:tt)+ ) => {
        let decorator = $tree.push_node(
            crate::ai::bt::tree::Node::Decorator(
                crate::ai::bt::tree::Decorator::new($kind)
            ),
        );
        bt_internal!(@child $tree, decorator, $($block)+);
        $tree.add_node_edge($parent, decorator).unwrap();
    };

    // create leaf node
    ( @leaf $tree:expr, $parent:expr, $task:expr ) => {
        let leaf = $tree.push_node(
            crate::ai::bt::tree::Node::Leaf(
                crate::ai::bt::tree::Leaf::new($task)
            ),
        );
        $tree.add_node_edge($parent, leaf).unwrap();
    };
}

#[cfg(test)]
mod tests {
    use std::array;

    use super::*;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum MockTask {
        A,
        B,
    }

    // the tests below use my assumptions of what the node id's should be.
    // not exactly bulletproof, but I don't feel like writing the AI trees by hand :/
    #[inline]
    fn nodes<const N: usize>() -> [usize; N] {
        array::from_fn(|i| i)
    }

    #[test]
    fn sequence() {
        // you can still use intellisense on the enums... that's so cool
        let mut bt = bt! {
            Composite(CompositeNode::Sequence) {
                Leaf(MockTask::A),
                Leaf(MockTask::B),
            },
        };

        let [_root, _composite, task_a, task_b] = nodes::<4>();

        assert_eq!(
            bt.run_root(),
            BehaviorOutput::Task {
                node: task_a,
                action: MockTask::A,
            },
            "First task not selected.",
        );

        assert_eq!(
            bt.run_leaf(task_a, OutVerdict::Success),
            BehaviorOutput::Task {
                node: task_b,
                action: MockTask::B,
            },
            "Second task not selected.",
        );

        assert_eq!(
            bt.run_leaf(task_b, OutVerdict::Success),
            BehaviorOutput::Complete {
                verdict: OutVerdict::Success
            },
            "Sequence didn't succeed fully.",
        );

        assert_eq!(
            bt.run_root(),
            BehaviorOutput::Task {
                node: task_a,
                action: MockTask::A,
            },
            "First task not selected on next iteration.",
        );

        assert_eq!(
            bt.run_leaf(task_a, OutVerdict::Failure),
            BehaviorOutput::Complete {
                verdict: OutVerdict::Failure
            },
            "Sequence didn't fail.",
        );
    }

    #[test]
    fn selector() {
        let mut bt = bt! {
            Composite(CompositeNode::Selector) {
                Leaf(MockTask::A),
                Leaf(MockTask::B),
            },
        };

        let [_root, _composite, task_a, task_b] = nodes::<4>();

        assert_eq!(
            bt.run_root(),
            BehaviorOutput::Task {
                node: task_a,
                action: MockTask::A,
            },
            "First task not selected.",
        );

        assert_eq!(
            bt.run_leaf(task_a, OutVerdict::Failure),
            BehaviorOutput::Task {
                node: task_b,
                action: MockTask::B,
            },
            "Second task not selected.",
        );

        assert_eq!(
            bt.run_leaf(task_b, OutVerdict::Failure),
            BehaviorOutput::Complete {
                verdict: OutVerdict::Failure
            },
            "Selector didn't fail fully.",
        );

        assert_eq!(
            bt.run_root(),
            BehaviorOutput::Task {
                node: task_a,
                action: MockTask::A,
            },
            "First task not selected on next iteration.",
        );

        assert_eq!(
            bt.run_leaf(task_a, OutVerdict::Success),
            BehaviorOutput::Complete {
                verdict: OutVerdict::Success
            },
            "Selector didn't succeed.",
        );
    }

    #[test]
    fn invert() {
        let mut bt = bt! {
            Decorator(DecoratorNode::Invert) {
                Leaf(MockTask::A),
            },
        };
        let [_root, _decorator, task_a] = nodes::<3>();

        assert_eq!(
            bt.run_root(),
            BehaviorOutput::Task {
                node: task_a,
                action: MockTask::A,
            },
            "Child task not selected.",
        );

        assert_eq!(
            bt.run_leaf(task_a, OutVerdict::Success),
            BehaviorOutput::Complete {
                verdict: OutVerdict::Failure
            },
            "Success wasn't inverted to failure.",
        );

        assert_eq!(
            bt.run_root(),
            BehaviorOutput::Task {
                node: task_a,
                action: MockTask::A,
            },
            "Child task not selected on next iteration.",
        );

        assert_eq!(
            bt.run_leaf(task_a, OutVerdict::Failure),
            BehaviorOutput::Complete {
                verdict: OutVerdict::Success
            },
            "Failure wasn't inverted to success.",
        );
    }
}
