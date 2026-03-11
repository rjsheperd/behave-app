//! Seq: forward and reverse iterators over the B+ tree.
//! Maintains a stack of branch-level bookmarks for efficient in-order traversal.
//! Supports optional bounds (from/to) and seeking within a sequence.

use std::cmp::Ordering;
use std::rc::Rc;

use crate::key::Key;
use crate::node::{Comparator, Node};
use crate::storage::StorageCell;

/// A single frame in the iteration stack (branch-level bookmark)
#[derive(Clone)]
pub(crate) struct SeqFrame {
    node: Rc<Node>, // Branch node at this level
    idx: usize,     // Current child index
    parent: Option<Box<SeqFrame>>,
}

/// Sorted set iterator — forward or reverse, with optional bounds.
#[derive(Clone)]
pub struct Seq {
    pub(crate) storage: Option<Rc<StorageCell>>,
    pub(crate) leaf: Rc<Node>,
    pub(crate) idx: usize,
    stack: Option<Box<SeqFrame>>,
    pub(crate) key_to: Option<Key>,
    pub(crate) cmp: Option<Rc<Comparator>>,
    pub(crate) ascending: bool,
    pub(crate) version: u64,
    pub(crate) set_version: Rc<std::cell::Cell<u64>>,
}

impl Seq {
    pub(crate) fn new(
        storage: Option<Rc<StorageCell>>,
        leaf: Rc<Node>,
        idx: usize,
        stack: Option<Box<SeqFrame>>,
        key_to: Option<Key>,
        cmp: Option<Rc<Comparator>>,
        ascending: bool,
        version: u64,
        set_version: Rc<std::cell::Cell<u64>>,
    ) -> Self {
        Self {
            storage, leaf, idx, stack, key_to, cmp, ascending, version, set_version,
        }
    }

    fn check_version(&self) {
        assert_eq!(
            self.version,
            self.set_version.get(),
            "iterating and mutating a transient set at the same time"
        );
    }

    pub fn first(&self) -> Key {
        self.check_version();
        self.leaf.keys()[self.idx].clone()
    }

    pub fn over(&self) -> bool {
        match (&self.key_to, &self.cmp) {
            (Some(kt), Some(cmp)) => {
                let d = cmp(&self.leaf.keys()[self.idx], kt);
                if self.ascending { d == Ordering::Greater } else { d == Ordering::Less }
            }
            _ => false,
        }
    }

    pub fn advance(&mut self) -> bool {
        self.check_version();
        if self.ascending {
            if self.idx < self.leaf.len() - 1 {
                self.idx += 1;
                return !self.over();
            }
            self.advance_stack_ascending()
        } else {
            if self.idx > 0 {
                self.idx -= 1;
                return !self.over();
            }
            self.advance_stack_descending()
        }
    }

    fn advance_stack_ascending(&mut self) -> bool {
        loop {
            let frame = match self.stack.take() {
                Some(f) => f,
                None => return false,
            };
            let new_idx = frame.idx + 1;
            if new_idx < frame.node.len() {
                let child = get_child(&frame.node, new_idx, self.storage.as_ref());
                self.stack = Some(Box::new(SeqFrame {
                    node: Rc::clone(&frame.node),
                    idx: new_idx,
                    parent: frame.parent,
                }));
                self.descend_left(child);
                return !self.over();
            }
            self.stack = frame.parent;
        }
    }

    fn advance_stack_descending(&mut self) -> bool {
        loop {
            let frame = match self.stack.take() {
                Some(f) => f,
                None => return false,
            };
            if frame.idx > 0 {
                let new_idx = frame.idx - 1;
                let child = get_child(&frame.node, new_idx, self.storage.as_ref());
                self.stack = Some(Box::new(SeqFrame {
                    node: Rc::clone(&frame.node),
                    idx: new_idx,
                    parent: frame.parent,
                }));
                self.descend_right(child);
                return !self.over();
            }
            self.stack = frame.parent;
        }
    }

    fn descend_left(&mut self, mut node: Rc<Node>) {
        loop {
            if node.level() == 0 {
                self.leaf = node;
                self.idx = 0;
                return;
            }
            let child = get_child(&node, 0, self.storage.as_ref());
            self.stack = Some(Box::new(SeqFrame {
                node,
                idx: 0,
                parent: self.stack.take(),
            }));
            node = child;
        }
    }

    fn descend_right(&mut self, mut node: Rc<Node>) {
        loop {
            let last = node.len() - 1;
            if node.level() == 0 {
                self.leaf = node;
                self.idx = last;
                return;
            }
            let child = get_child(&node, last, self.storage.as_ref());
            self.stack = Some(Box::new(SeqFrame {
                node,
                idx: last,
                parent: self.stack.take(),
            }));
            node = child;
        }
    }

    pub fn next(&self) -> Option<Seq> {
        let mut seq = self.clone();
        if seq.advance() { Some(seq) } else { None }
    }

    pub fn seek(&self, to: &Key, cmp_override: Option<&Comparator>) -> Option<Seq> {
        let default_cmp: &Comparator;
        let cmp: &Comparator = match cmp_override {
            Some(c) => c,
            None => {
                let rc = self.cmp.as_ref().expect("seek requires a comparator");
                default_cmp = &**rc;
                default_cmp
            }
        };

        let storage_ref = self.storage.as_ref().map(|s| s.as_ref());

        if self.ascending {
            let mut node: Rc<Node> = Rc::clone(&self.leaf);
            let mut stack = self.stack.clone();

            while cmp(node.max_key(), to) == Ordering::Less {
                match stack.take() {
                    Some(frame) => {
                        node = Rc::clone(&frame.node);
                        stack = frame.parent;
                    }
                    None => return None,
                }
            }

            let mut seq_stack = stack;
            let mut cur = node;
            loop {
                let idx = match Node::search_first(cur.keys(), to, cmp) {
                    Ok(i) | Err(i) => i,
                };
                if idx == cur.len() {
                    return None;
                }

                if cur.level() == 0 {
                    let new_seq = Seq {
                        storage: self.storage.clone(),
                        leaf: cur,
                        idx,
                        stack: seq_stack,
                        key_to: self.key_to.clone(),
                        cmp: self.cmp.clone(),
                        ascending: true,
                        version: self.version,
                        set_version: Rc::clone(&self.set_version),
                    };
                    return if new_seq.over() { None } else { Some(new_seq) };
                }

                let child = get_child_storage(&cur, idx, storage_ref);
                seq_stack = Some(Box::new(SeqFrame {
                    node: cur,
                    idx,
                    parent: seq_stack,
                }));
                cur = child;
            }
        } else {
            let mut node: Rc<Node> = Rc::clone(&self.leaf);
            let mut stack = self.stack.clone();

            while cmp(to, node.min_key()) == Ordering::Less {
                match stack {
                    Some(frame) => {
                        node = Rc::clone(&frame.node);
                        stack = frame.parent;
                    }
                    None => break,
                }
            }

            let mut seq_stack = stack;
            let mut cur = node;
            loop {
                if cur.level() > 0 {
                    let search_result = Node::search_last(cur.keys(), to, cmp);
                    let mut idx = match search_result {
                        Some(i) => i + 1,
                        None => 0,
                    };
                    if idx == cur.len() { idx -= 1; }
                    let child = get_child_storage(&cur, idx, storage_ref);
                    seq_stack = Some(Box::new(SeqFrame {
                        node: cur,
                        idx,
                        parent: seq_stack,
                    }));
                    cur = child;
                } else {
                    let idx_opt = Node::search_last(cur.keys(), to, cmp);
                    match idx_opt {
                        None => {
                            let mut new_seq = Seq {
                                storage: self.storage.clone(),
                                leaf: cur,
                                idx: 0,
                                stack: seq_stack,
                                key_to: self.key_to.clone(),
                                cmp: self.cmp.clone(),
                                ascending: false,
                                version: self.version,
                                set_version: Rc::clone(&self.set_version),
                            };
                            return if new_seq.advance() { Some(new_seq) } else { None };
                        }
                        Some(idx) => {
                            let new_seq = Seq {
                                storage: self.storage.clone(),
                                leaf: cur,
                                idx,
                                stack: seq_stack,
                                key_to: self.key_to.clone(),
                                cmp: self.cmp.clone(),
                                ascending: false,
                                version: self.version,
                                set_version: Rc::clone(&self.set_version),
                            };
                            return if new_seq.over() { None } else { Some(new_seq) };
                        }
                    }
                }
            }
        }
    }

    pub fn to_vec(&self) -> Vec<Key> {
        let mut arr = vec![self.first()];
        let mut seq = self.clone();
        while seq.advance() {
            arr.push(seq.first());
        }
        arr
    }
}

fn get_child(node: &Rc<Node>, idx: usize, storage: Option<&Rc<StorageCell>>) -> Rc<Node> {
    match node.as_ref() {
        Node::Branch(b) => b.child(storage.map(|s| s.as_ref()), idx),
        _ => panic!("get_child on non-branch"),
    }
}

fn get_child_storage(node: &Rc<Node>, idx: usize, storage: Option<&StorageCell>) -> Rc<Node> {
    match node.as_ref() {
        Node::Branch(b) => b.child(storage, idx),
        _ => panic!("get_child on non-branch"),
    }
}

/// Build a Seq by descending from root for forward iteration.
pub(crate) fn build_seq_ascending(
    root: &Rc<Node>,
    from: Option<&Key>,
    to: Option<&Key>,
    cmp: &Rc<Comparator>,
    storage: &Option<Rc<StorageCell>>,
    version: u64,
    set_version: &Rc<std::cell::Cell<u64>>,
) -> Option<Seq> {
    if root.is_empty() {
        return None;
    }

    let storage_ref = storage.as_ref().map(|s| s.as_ref());
    let cmp_ref: &Comparator = &**cmp;
    let mut stack: Option<Box<SeqFrame>> = None;
    let mut node = Rc::clone(root);

    match from {
        None => {
            loop {
                if node.level() == 0 {
                    let seq = Seq::new(
                        storage.clone(), node, 0, stack, to.cloned(),
                        Some(Rc::clone(cmp)), true, version, Rc::clone(set_version),
                    );
                    return if seq.over() { None } else { Some(seq) };
                }
                let child = get_child_storage(&node, 0, storage_ref);
                stack = Some(Box::new(SeqFrame { node, idx: 0, parent: stack }));
                node = child;
            }
        }
        Some(from_key) => {
            loop {
                let idx = match Node::search_first(node.keys(), from_key, cmp_ref) {
                    Ok(i) | Err(i) => i,
                };
                if idx == node.len() {
                    return None;
                }

                if node.level() == 0 {
                    let seq = Seq::new(
                        storage.clone(), node, idx, stack, to.cloned(),
                        Some(Rc::clone(cmp)), true, version, Rc::clone(set_version),
                    );
                    return if seq.over() { None } else { Some(seq) };
                }
                let child = get_child_storage(&node, idx, storage_ref);
                stack = Some(Box::new(SeqFrame { node, idx, parent: stack }));
                node = child;
            }
        }
    }
}

/// Build a Seq for reverse iteration.
pub(crate) fn build_seq_descending(
    root: &Rc<Node>,
    from: Option<&Key>,
    to: Option<&Key>,
    cmp: &Rc<Comparator>,
    storage: &Option<Rc<StorageCell>>,
    version: u64,
    set_version: &Rc<std::cell::Cell<u64>>,
) -> Option<Seq> {
    if root.is_empty() {
        return None;
    }

    let storage_ref = storage.as_ref().map(|s| s.as_ref());
    let cmp_ref: &Comparator = &**cmp;
    let mut stack: Option<Box<SeqFrame>> = None;
    let mut node = Rc::clone(root);

    match from {
        None => {
            loop {
                let last = node.len() - 1;
                if node.level() == 0 {
                    let seq = Seq::new(
                        storage.clone(), node, last, stack, to.cloned(),
                        Some(Rc::clone(cmp)), false, version, Rc::clone(set_version),
                    );
                    return if seq.over() { None } else { Some(seq) };
                }
                let child = get_child_storage(&node, last, storage_ref);
                stack = Some(Box::new(SeqFrame { node, idx: last, parent: stack }));
                node = child;
            }
        }
        Some(from_key) => {
            loop {
                if node.level() > 0 {
                    let search_result = Node::search_last(node.keys(), from_key, cmp_ref);
                    let mut idx = match search_result {
                        Some(i) => i + 1,
                        None => 0,
                    };
                    if idx == node.len() { idx -= 1; }
                    let child = get_child_storage(&node, idx, storage_ref);
                    stack = Some(Box::new(SeqFrame { node, idx, parent: stack }));
                    node = child;
                } else {
                    let idx_opt = Node::search_last(node.keys(), from_key, cmp_ref);
                    match idx_opt {
                        None => {
                            let mut seq = Seq::new(
                                storage.clone(), node, 0, stack, to.cloned(),
                                Some(Rc::clone(cmp)), false, version, Rc::clone(set_version),
                            );
                            return if seq.advance() { Some(seq) } else { None };
                        }
                        Some(idx) => {
                            let seq = Seq::new(
                                storage.clone(), node, idx, stack, to.cloned(),
                                Some(Rc::clone(cmp)), false, version, Rc::clone(set_version),
                            );
                            return if seq.over() { None } else { Some(seq) };
                        }
                    }
                }
            }
        }
    }
}
