//! Result types for add (conj) and remove (disj) operations.
//! These encode the tree-structural outcome: unchanged, mutated in place,
//! replaced with one node, or split into two.

use std::rc::Rc;

use crate::node::Node;

/// Result of an add (conj) operation on a node
pub enum AddResult {
    /// Key already existed, tree unchanged
    Unchanged,
    /// Node was modified in-place (transient/editable path only)
    EarlyExit,
    /// Subtree fits in one node (no split)
    One(Rc<Node>),
    /// Subtree split into two nodes (caller must add separator)
    Split(Rc<Node>, Rc<Node>),
}

/// Result of a remove (disj) operation on a node
pub enum RemoveResult {
    /// Key not found, tree unchanged
    Unchanged,
    /// Node was modified in-place (transient/editable path only)
    EarlyExit,
    /// Rebalanced result after removal.
    /// - left/right may be None if merged with center
    /// - left/right may differ from input if keys were borrowed
    Rebalanced {
        left: Option<Rc<Node>>,
        center: Rc<Node>,
        right: Option<Rc<Node>>,
    },
}
