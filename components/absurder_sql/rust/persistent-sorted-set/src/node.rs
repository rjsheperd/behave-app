//! Node: the unified enum over Leaf and Branch, plus binary search utilities.
//! Also defines the `Comparator` type alias used throughout the crate.

use std::cmp::Ordering;

use crate::branch::Branch;
use crate::key::Key;
use crate::leaf::Leaf;
use crate::results::{AddResult, RemoveResult};
use crate::settings::Settings;
use crate::storage::StorageCell;

/// Comparator: takes two key references, returns Ordering
pub type Comparator = dyn Fn(&Key, &Key) -> Ordering;

/// Unified node type replacing the JS ANode class hierarchy.
pub enum Node {
    Leaf(Leaf),
    Branch(Branch),
}

impl Node {
    pub fn level(&self) -> u32 {
        match self {
            Node::Leaf(_) => 0,
            Node::Branch(b) => b.level,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Node::Leaf(l) => l.keys.len(),
            Node::Branch(b) => b.keys.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn keys(&self) -> &[Key] {
        match self {
            Node::Leaf(l) => &l.keys,
            Node::Branch(b) => &b.keys,
        }
    }

    pub fn min_key(&self) -> &Key {
        &self.keys()[0]
    }

    pub fn max_key(&self) -> &Key {
        &self.keys()[self.len() - 1]
    }

    pub fn contains(
        &self,
        storage: Option<&StorageCell>,
        key: &Key,
        cmp: &Comparator,
    ) -> bool {
        match self {
            Node::Leaf(l) => l.contains(key, cmp),
            Node::Branch(b) => b.contains(storage, key, cmp),
        }
    }

    pub fn count(&self, storage: Option<&StorageCell>) -> usize {
        match self {
            Node::Leaf(l) => l.keys.len(),
            Node::Branch(b) => b.count(storage),
        }
    }

    pub fn add(
        &self,
        storage: Option<&StorageCell>,
        key: &Key,
        cmp: &Comparator,
        settings: &Settings,
    ) -> AddResult {
        match self {
            Node::Leaf(l) => l.add(key, cmp, settings),
            Node::Branch(b) => b.add(storage, key, cmp, settings),
        }
    }

    pub fn remove(
        &self,
        storage: Option<&StorageCell>,
        key: &Key,
        left: Option<&Node>,
        right: Option<&Node>,
        cmp: &Comparator,
        settings: &Settings,
    ) -> RemoveResult {
        match self {
            Node::Leaf(l) => {
                let left_leaf = left.map(|n| match n {
                    Node::Leaf(l) => l,
                    _ => panic!("leaf sibling must be leaf"),
                });
                let right_leaf = right.map(|n| match n {
                    Node::Leaf(l) => l,
                    _ => panic!("leaf sibling must be leaf"),
                });
                l.remove(key, left_leaf, right_leaf, cmp, settings)
            }
            Node::Branch(b) => {
                let left_branch = left.map(|n| match n {
                    Node::Branch(b) => b,
                    _ => panic!("branch sibling must be branch"),
                });
                let right_branch = right.map(|n| match n {
                    Node::Branch(b) => b,
                    _ => panic!("branch sibling must be branch"),
                });
                b.remove(storage, key, left_branch, right_branch, cmp, settings)
            }
        }
    }

    pub fn store(&self, storage: &StorageCell) -> i64 {
        match self {
            Node::Leaf(_) => storage.store(self),
            Node::Branch(b) => b.store(storage),
        }
    }

    pub fn walk_addresses(
        &self,
        storage: Option<&StorageCell>,
        on_address: &mut dyn FnMut(i64) -> bool,
    ) {
        match self {
            Node::Leaf(_) => {} // leaves have no child addresses
            Node::Branch(b) => b.walk_addresses(storage, on_address),
        }
    }

    /// Binary search for key in a sorted slice.
    /// Returns Ok(idx) if found, Err(insertion_point) if not.
    pub fn search(keys: &[Key], key: &Key, cmp: &Comparator) -> Result<usize, usize> {
        if keys.is_empty() {
            return Err(0);
        }
        let mut lo: usize = 0;
        let mut hi: usize = keys.len() - 1;

        while lo <= hi {
            let mid = lo + (hi - lo) / 2;
            match cmp(&keys[mid], key) {
                Ordering::Less => lo = mid + 1,
                Ordering::Greater => {
                    if mid == 0 {
                        return Err(0);
                    }
                    hi = mid - 1;
                }
                Ordering::Equal => return Ok(mid),
            }
        }
        Err(lo)
    }

    /// Find first index where keys[idx] >= key.
    /// Returns Ok(idx) if exact match, Err(idx) for first greater.
    pub fn search_first(keys: &[Key], key: &Key, cmp: &Comparator) -> Result<usize, usize> {
        if keys.is_empty() {
            return Err(0);
        }
        let mut lo: usize = 0;
        let mut hi: usize = keys.len() - 1;
        let mut found: Option<usize> = None;

        while lo <= hi {
            let mid = lo + (hi - lo) / 2;
            match cmp(&keys[mid], key) {
                Ordering::Less => lo = mid + 1,
                Ordering::Greater => {
                    found = Some(mid);
                    if mid == 0 {
                        break;
                    }
                    hi = mid - 1;
                }
                Ordering::Equal => return Ok(mid),
            }
        }
        match found {
            Some(idx) => Err(idx),
            None => Err(lo), // past end
        }
    }

    /// Find last index where keys[idx] <= key.
    /// Returns None if all keys > key.
    pub fn search_last(keys: &[Key], key: &Key, cmp: &Comparator) -> Option<usize> {
        if keys.is_empty() {
            return None;
        }
        let mut lo: usize = 0;
        let mut hi: usize = keys.len() - 1;
        let mut found: Option<usize> = None;

        while lo <= hi {
            let mid = lo + (hi - lo) / 2;
            match cmp(&keys[mid], key) {
                Ordering::Less => {
                    found = Some(mid);
                    lo = mid + 1;
                }
                Ordering::Greater => {
                    if mid == 0 {
                        break;
                    }
                    hi = mid - 1;
                }
                Ordering::Equal => return Some(mid),
            }
        }
        found
    }

    /// Restore a node from storage data.
    pub fn restore(
        level: u32,
        keys: Vec<Key>,
        addresses: Option<Vec<i64>>,
        settings: &Settings,
    ) -> Node {
        if level == 0 {
            Node::Leaf(Leaf { keys })
        } else {
            let len = keys.len();
            let addr_vec: Vec<Option<i64>> = match addresses {
                Some(addrs) => addrs.into_iter().map(Some).collect(),
                None => vec![None; len],
            };
            Node::Branch(Branch {
                level,
                keys,
                addresses: Some(addr_vec),
                children: Some(vec![None; len]),
                settings: settings.clone(),
            })
        }
    }
}

impl Clone for Node {
    fn clone(&self) -> Self {
        match self {
            Node::Leaf(l) => Node::Leaf(l.clone()),
            Node::Branch(b) => Node::Branch(b.clone()),
        }
    }
}

impl std::fmt::Debug for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Node::Leaf(l) => f.debug_struct("Leaf").field("keys", &l.keys).finish(),
            Node::Branch(b) => f
                .debug_struct("Branch")
                .field("level", &b.level)
                .field("keys", &b.keys)
                .finish_non_exhaustive(),
        }
    }
}
