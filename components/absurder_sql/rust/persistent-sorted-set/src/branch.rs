//! Branch node: interior level of the B+ tree.
//! Holds sorted keys, child node pointers, and optional storage addresses for
//! lazy loading. Stored children use `Weak<Node>` references whose lifetimes
//! are managed by the `StorageCell` LRU cache; unstored children use strong
//! `Rc<Node>` references that cannot be evicted.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use crate::key::Key;
use crate::node::{Comparator, Node};
use crate::results::{AddResult, RemoveResult};
use crate::settings::Settings;
use crate::storage::StorageCell;

/// Reference to a child node. Stored children use `Weak` (lifetime managed by
/// the StorageCell LRU cache); unstored children use `Strong`.
pub(crate) enum ChildRef {
    /// Not yet stored — strong reference keeps node alive.
    Strong(Rc<Node>),
    /// Stored (has address) — weak reference, lifetime managed by StorageCell LRU.
    Cached(Weak<Node>),
}

impl ChildRef {
    fn upgrade(&self) -> Option<Rc<Node>> {
        match self {
            ChildRef::Strong(rc) => Some(Rc::clone(rc)),
            ChildRef::Cached(weak) => weak.upgrade(),
        }
    }
}

impl Clone for ChildRef {
    fn clone(&self) -> Self {
        match self {
            ChildRef::Strong(rc) => ChildRef::Strong(Rc::clone(rc)),
            ChildRef::Cached(weak) => ChildRef::Cached(weak.clone()),
        }
    }
}

impl std::fmt::Debug for ChildRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChildRef::Strong(rc) => write!(f, "Strong({:?})", Rc::as_ptr(rc)),
            ChildRef::Cached(weak) => write!(f, "Cached(alive={})", weak.upgrade().is_some()),
        }
    }
}

#[derive(Debug)]
pub struct Branch {
    pub(crate) level: u32,
    pub(crate) keys: Vec<Key>,
    pub(crate) addresses: Option<Vec<Option<i64>>>,
    pub(crate) children: RefCell<Vec<Option<ChildRef>>>,
    pub(crate) settings: Settings,
}

impl Clone for Branch {
    fn clone(&self) -> Self {
        Self {
            level: self.level,
            keys: self.keys.clone(),
            addresses: self.addresses.clone(),
            children: RefCell::new(self.children.borrow().clone()),
            settings: self.settings.clone(),
        }
    }
}

impl Branch {
    pub fn new(
        level: u32,
        keys: Vec<Key>,
        addresses: Option<Vec<Option<i64>>>,
        children: Option<Vec<Option<Rc<Node>>>>,
        settings: Settings,
    ) -> Self {
        let child_refs: Vec<Option<ChildRef>> = match children {
            Some(ch) => ch
                .into_iter()
                .map(|opt| opt.map(ChildRef::Strong))
                .collect(),
            None => vec![None; keys.len()],
        };
        Self {
            level,
            keys,
            addresses,
            children: RefCell::new(child_refs),
            settings,
        }
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn address(&self, idx: usize) -> Option<i64> {
        self.addresses
            .as_ref()
            .and_then(|addrs| addrs.get(idx).copied().flatten())
    }

    /// Get child at index, lazily loading from storage if needed.
    /// Uses interior mutability to cache weak references back into the children vec.
    pub fn child(&self, storage: Option<&StorageCell>, idx: usize) -> Rc<Node> {
        let mut children = self.children.borrow_mut();

        // Try existing reference
        if let Some(child_ref) = children[idx].as_ref() {
            if let Some(rc) = child_ref.upgrade() {
                // Node still alive — bump LRU if stored
                if let (Some(s), Some(addr)) = (storage, self.address(idx)) {
                    s.accessed(addr);
                }
                return rc;
            }
        }

        // Need to load from storage
        let addr = self
            .address(idx)
            .expect("branch child has no address and no cached node");
        let storage = storage.expect("storage required for lazy child load");
        let rc = storage.restore_cached(addr);

        // Cache weak ref back into children vec
        children[idx] = Some(ChildRef::Cached(Rc::downgrade(&rc)));

        rc
    }

    pub fn contains(
        &self,
        storage: Option<&StorageCell>,
        key: &Key,
        cmp: &Comparator,
    ) -> bool {
        match Node::search(&self.keys, key, cmp) {
            Ok(_) => true,
            Err(ins) => {
                if ins == self.keys.len() {
                    false
                } else {
                    self.child(storage, ins).contains(storage, key, cmp)
                }
            }
        }
    }

    pub fn count(&self, storage: Option<&StorageCell>) -> usize {
        let mut total = 0;
        for i in 0..self.keys.len() {
            total += self.child(storage, i).count(storage);
        }
        total
    }

    pub fn add(
        &self,
        storage: Option<&StorageCell>,
        key: &Key,
        cmp: &Comparator,
        settings: &Settings,
    ) -> AddResult {
        match Node::search(&self.keys, key, cmp) {
            Ok(_) => AddResult::Unchanged,
            Err(ins_raw) => {
                let ins = if ins_raw == self.keys.len() {
                    self.keys.len() - 1
                } else {
                    ins_raw
                };

                let child_result =
                    self.child(storage, ins).add(storage, key, cmp, settings);

                match child_result {
                    AddResult::Unchanged => AddResult::Unchanged,
                    AddResult::EarlyExit => AddResult::EarlyExit,

                    AddResult::One(node) => {
                        let new_key = node.max_key().clone();
                        let mut new_keys = self.keys.clone();
                        new_keys[ins] = new_key;

                        let new_addrs: Option<Vec<Option<i64>>> =
                            self.addresses.as_ref().map(|a| {
                                let mut v = a.clone();
                                v[ins] = None; // dirty
                                v
                            });

                        let children = self.children.borrow();
                        let mut new_children: Vec<Option<ChildRef>> = children.clone();
                        new_children[ins] = Some(ChildRef::Strong(node));

                        AddResult::One(Rc::new(Node::Branch(Branch {
                            level: self.level,
                            keys: new_keys,
                            addresses: new_addrs,
                            children: RefCell::new(new_children),
                            settings: settings.clone(),
                        })))
                    }

                    AddResult::Split(left_child, right_child) => {
                        let lk = left_child.max_key().clone();
                        let rk = right_child.max_key().clone();

                        if self.keys.len() < settings.branching_factor() {
                            let mut new_keys =
                                Vec::with_capacity(self.keys.len() + 1);
                            new_keys.extend_from_slice(&self.keys[..ins]);
                            new_keys.push(lk);
                            new_keys.push(rk);
                            new_keys.extend_from_slice(&self.keys[ins + 1..]);

                            let new_addrs = self.addresses.as_ref().map(|a| {
                                let mut v = Vec::with_capacity(a.len() + 1);
                                v.extend_from_slice(&a[..ins]);
                                v.push(None);
                                v.push(None);
                                v.extend_from_slice(&a[ins + 1..]);
                                v
                            });

                            let new_children: Vec<Option<ChildRef>> = {
                                let old = self.children.borrow();
                                let mut v = Vec::with_capacity(old.len() + 1);
                                v.extend_from_slice(&old[..ins]);
                                v.push(Some(ChildRef::Strong(left_child)));
                                v.push(Some(ChildRef::Strong(right_child)));
                                v.extend_from_slice(&old[ins + 1..]);
                                v
                            };

                            AddResult::One(Rc::new(Node::Branch(Branch {
                                level: self.level,
                                keys: new_keys,
                                addresses: new_addrs,
                                children: RefCell::new(new_children),
                                settings: settings.clone(),
                            })))
                        } else {
                            // Branch must split too
                            let total = self.keys.len() + 1;
                            let mut half1 = (total + 1) / 2;
                            if ins + 1 == half1 {
                                half1 += 1;
                            }

                            let mut exp_keys = Vec::with_capacity(total);
                            exp_keys.extend_from_slice(&self.keys[..ins]);
                            exp_keys.push(lk);
                            exp_keys.push(rk);
                            exp_keys.extend_from_slice(&self.keys[ins + 1..]);

                            let exp_addrs: Option<Vec<Option<i64>>> =
                                self.addresses.as_ref().map(|a| {
                                    let mut v = Vec::with_capacity(total);
                                    v.extend_from_slice(&a[..ins]);
                                    v.push(None);
                                    v.push(None);
                                    v.extend_from_slice(&a[ins + 1..]);
                                    v
                                });

                            let exp_children: Vec<Option<ChildRef>> = {
                                let old = self.children.borrow();
                                let mut v = Vec::with_capacity(total);
                                v.extend_from_slice(&old[..ins]);
                                v.push(Some(ChildRef::Strong(left_child)));
                                v.push(Some(ChildRef::Strong(right_child)));
                                v.extend_from_slice(&old[ins + 1..]);
                                v
                            };

                            let keys1 = exp_keys[..half1].to_vec();
                            let keys2 = exp_keys[half1..].to_vec();

                            let addrs1 = exp_addrs.as_ref().map(|a| a[..half1].to_vec());
                            let addrs2 = exp_addrs.as_ref().map(|a| a[half1..].to_vec());

                            let children1 = exp_children[..half1].to_vec();
                            let children2 = exp_children[half1..].to_vec();

                            AddResult::Split(
                                Rc::new(Node::Branch(Branch {
                                    level: self.level,
                                    keys: keys1,
                                    addresses: addrs1,
                                    children: RefCell::new(children1),
                                    settings: settings.clone(),
                                })),
                                Rc::new(Node::Branch(Branch {
                                    level: self.level,
                                    keys: keys2,
                                    addresses: addrs2,
                                    children: RefCell::new(children2),
                                    settings: settings.clone(),
                                })),
                            )
                        }
                    }
                }
            }
        }
    }

    pub fn remove(
        &self,
        storage: Option<&StorageCell>,
        key: &Key,
        left: Option<&Branch>,
        right: Option<&Branch>,
        cmp: &Comparator,
        settings: &Settings,
    ) -> RemoveResult {
        let idx = match Node::search(&self.keys, key, cmp) {
            Ok(i) => i,
            Err(i) => i,
        };

        if idx == self.keys.len() {
            return RemoveResult::Unchanged;
        }

        let left_child: Option<Rc<Node>> = if idx > 0 {
            Some(self.child(storage, idx - 1))
        } else {
            None
        };
        let right_child: Option<Rc<Node>> = if idx < self.keys.len() - 1 {
            Some(self.child(storage, idx + 1))
        } else {
            None
        };

        let child_result = self.child(storage, idx).remove(
            storage,
            key,
            left_child.as_ref().map(|c| c.as_ref()),
            right_child.as_ref().map(|c| c.as_ref()),
            cmp,
            settings,
        );

        match child_result {
            RemoveResult::Unchanged => RemoveResult::Unchanged,
            RemoveResult::EarlyExit => RemoveResult::EarlyExit,

            RemoveResult::Rebalanced {
                left: res_left,
                center: res_center,
                right: res_right,
            } => {
                // Calculate new length
                let new_len: i64 = self.keys.len() as i64 - 1
                    - if left_child.is_some() { 1 } else { 0 }
                    - if right_child.is_some() { 1 } else { 0 }
                    + if res_left.is_some() { 1 } else { 0 }
                    + 1 // center always present
                    + if res_right.is_some() { 1 } else { 0 };
                let new_len = new_len as usize;

                let left_changed = match (&left_child, &res_left) {
                    (Some(lc), Some(rl)) => !Rc::ptr_eq(lc, rl) || lc.len() != rl.len(),
                    (None, None) => false,
                    _ => true,
                };
                let right_changed = match (&right_child, &res_right) {
                    (Some(rc), Some(rr)) => !Rc::ptr_eq(rc, rr) || rc.len() != rr.len(),
                    (None, None) => false,
                    _ => true,
                };

                // Build the replacement children array for this branch
                let build_branch = |_self_left: Option<&Branch>,
                                     _self_right: Option<&Branch>|
                 -> Rc<Node> {
                    let start = if idx > 0 { idx - 1 } else { 0 };
                    let end = std::cmp::min(idx + 2, self.keys.len());

                    let mut new_keys = Vec::with_capacity(new_len);
                    let mut new_addrs: Vec<Option<i64>> = Vec::with_capacity(new_len);
                    let mut new_children_vec: Vec<Option<ChildRef>> =
                        Vec::with_capacity(new_len);

                    // prefix
                    new_keys.extend_from_slice(&self.keys[..start]);
                    if let Some(addrs) = &self.addresses {
                        new_addrs.extend_from_slice(&addrs[..start]);
                    } else {
                        new_addrs.extend(std::iter::repeat(None).take(start));
                    }
                    {
                        let ch = self.children.borrow();
                        new_children_vec.extend_from_slice(&ch[..start]);
                    }

                    // result left
                    if let Some(rl) = &res_left {
                        new_keys.push(rl.max_key().clone());
                        new_addrs.push(if left_changed {
                            None
                        } else {
                            self.address(idx - 1)
                        });
                        new_children_vec.push(Some(ChildRef::Strong(Rc::clone(rl))));
                    }

                    // center
                    new_keys.push(res_center.max_key().clone());
                    new_addrs.push(None); // always dirty
                    new_children_vec.push(Some(ChildRef::Strong(Rc::clone(&res_center))));

                    // result right
                    if let Some(rr) = &res_right {
                        new_keys.push(rr.max_key().clone());
                        new_addrs.push(if right_changed {
                            None
                        } else {
                            self.address(idx + 1)
                        });
                        new_children_vec.push(Some(ChildRef::Strong(Rc::clone(rr))));
                    }

                    // suffix
                    new_keys.extend_from_slice(&self.keys[end..]);
                    if let Some(addrs) = &self.addresses {
                        new_addrs.extend_from_slice(&addrs[end..]);
                    } else {
                        new_addrs.extend(
                            std::iter::repeat(None).take(self.keys.len() - end),
                        );
                    }
                    {
                        let ch = self.children.borrow();
                        new_children_vec.extend_from_slice(&ch[end..]);
                    }

                    Rc::new(Node::Branch(Branch {
                        level: self.level,
                        keys: new_keys,
                        addresses: Some(new_addrs),
                        children: RefCell::new(new_children_vec),
                        settings: settings.clone(),
                    }))
                };

                // No rebalance at this level needed
                if new_len >= settings.min_branching_factor()
                    || (left.is_none() && right.is_none())
                {
                    let new_center = build_branch(left, right);
                    return RemoveResult::Rebalanced {
                        left: left.map(|l| Rc::new(Node::Branch(l.clone()))),
                        center: new_center,
                        right: right.map(|r| Rc::new(Node::Branch(r.clone()))),
                    };
                }

                // Build a temporary center branch, then do sibling operations
                let center_branch = build_branch(None, None);
                let center_b = match center_branch.as_ref() {
                    Node::Branch(b) => b,
                    _ => unreachable!(),
                };

                let left_len = left.map_or(0, |l| l.keys.len());
                let right_len = right.map_or(0, |r| r.keys.len());

                // Can join with left
                if let Some(l) = left {
                    if left_len + new_len <= settings.branching_factor() {
                        let joined = join_branches(l, center_b, settings);
                        return RemoveResult::Rebalanced {
                            left: None,
                            center: Rc::new(Node::Branch(joined)),
                            right: right.map(|r| Rc::new(Node::Branch(r.clone()))),
                        };
                    }
                }

                // Can join with right
                if let Some(r) = right {
                    if new_len + right_len <= settings.branching_factor() {
                        let joined = join_branches(center_b, r, settings);
                        return RemoveResult::Rebalanced {
                            left: left.map(|l| Rc::new(Node::Branch(l.clone()))),
                            center: Rc::new(Node::Branch(joined)),
                            right: None,
                        };
                    }
                }

                // Borrow from left
                if let Some(l) = left {
                    if right.is_none() || left_len >= right_len {
                        let total_len = left_len + new_len;
                        let new_left_len = total_len / 2;

                        let new_left = slice_branch(l, 0, new_left_len, settings);
                        let mut borrowed = Branch {
                            level: self.level,
                            keys: Vec::new(),
                            addresses: Some(Vec::new()),
                            children: RefCell::new(Vec::new()),
                            settings: settings.clone(),
                        };

                        append_branch_range(&mut borrowed, l, new_left_len, left_len);
                        append_branch_range(&mut borrowed, center_b, 0, new_len);

                        return RemoveResult::Rebalanced {
                            left: Some(Rc::new(Node::Branch(new_left))),
                            center: Rc::new(Node::Branch(borrowed)),
                            right: right.map(|r| Rc::new(Node::Branch(r.clone()))),
                        };
                    }
                }

                // Borrow from right
                if let Some(r) = right {
                    let total_len = new_len + right_len;
                    let new_center_len = total_len / 2;
                    let right_head = right_len - (total_len - new_center_len);

                    let mut new_center = Branch {
                        level: self.level,
                        keys: Vec::new(),
                        addresses: Some(Vec::new()),
                        children: RefCell::new(Vec::new()),
                        settings: settings.clone(),
                    };
                    append_branch_range(&mut new_center, center_b, 0, new_len);
                    append_branch_range(&mut new_center, r, 0, right_head);

                    let new_right = slice_branch(r, right_head, right_len, settings);

                    return RemoveResult::Rebalanced {
                        left: left.map(|l| Rc::new(Node::Branch(l.clone()))),
                        center: Rc::new(Node::Branch(new_center)),
                        right: Some(Rc::new(Node::Branch(new_right))),
                    };
                }

                unreachable!("branch remove: no rebalance path matched")
            }
        }
    }

    /// Store bottom-up: store all unstored children first, then store self.
    pub fn store(&self, storage: &StorageCell) -> i64 {
        let mut addrs: Vec<Option<i64>> = self
            .addresses
            .clone()
            .unwrap_or_else(|| vec![None; self.keys.len()]);

        for i in 0..self.keys.len() {
            if addrs[i].is_none() {
                let children = self.children.borrow();
                if let Some(child_ref) = children[i].as_ref() {
                    if let Some(rc) = child_ref.upgrade() {
                        drop(children);
                        let child_addr = rc.store(storage);
                        addrs[i] = Some(child_addr);
                    }
                }
            }
        }

        // Build a temporary node with all addresses filled for serialization
        let children_snapshot = self.children.borrow().clone();
        let stored_branch = Node::Branch(Branch {
            level: self.level,
            keys: self.keys.clone(),
            addresses: Some(addrs),
            children: RefCell::new(children_snapshot),
            settings: self.settings.clone(),
        });
        storage.store(&stored_branch)
    }

    pub fn walk_addresses(
        &self,
        storage: Option<&StorageCell>,
        on_address: &mut dyn FnMut(i64) -> bool,
    ) {
        for i in 0..self.keys.len() {
            if let Some(addr) = self.address(i) {
                if !on_address(addr) {
                    continue;
                }
            }
            if self.level > 1 {
                self.child(storage, i)
                    .walk_addresses(storage, on_address);
            }
        }
    }
}

/// Join two branches into one.
fn join_branches(left: &Branch, right: &Branch, settings: &Settings) -> Branch {
    let total = left.keys.len() + right.keys.len();
    let mut b = Branch {
        level: left.level,
        keys: Vec::with_capacity(total),
        addresses: Some(Vec::with_capacity(total)),
        children: RefCell::new(Vec::with_capacity(total)),
        settings: settings.clone(),
    };
    append_branch_range(&mut b, left, 0, left.keys.len());
    append_branch_range(&mut b, right, 0, right.keys.len());
    b
}

/// Extract a subrange of a branch as a new branch.
fn slice_branch(src: &Branch, from: usize, to: usize, settings: &Settings) -> Branch {
    let children = src.children.borrow();
    Branch {
        level: src.level,
        keys: src.keys[from..to].to_vec(),
        addresses: src
            .addresses
            .as_ref()
            .map(|a| a[from..to].to_vec()),
        children: RefCell::new(children[from..to].to_vec()),
        settings: settings.clone(),
    }
}

/// Append a range of slots from src into dst.
fn append_branch_range(dst: &mut Branch, src: &Branch, from: usize, to: usize) {
    dst.keys.extend_from_slice(&src.keys[from..to]);
    if let Some(dst_addrs) = &mut dst.addresses {
        if let Some(src_addrs) = &src.addresses {
            dst_addrs.extend_from_slice(&src_addrs[from..to]);
        } else {
            dst_addrs.extend(std::iter::repeat(None).take(to - from));
        }
    }
    {
        let src_ch = src.children.borrow();
        let mut dst_ch = dst.children.borrow_mut();
        dst_ch.extend_from_slice(&src_ch[from..to]);
    }
}
