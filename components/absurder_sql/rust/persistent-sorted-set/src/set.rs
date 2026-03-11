//! PersistentSortedSet: the top-level B+ tree API.
//! Provides persistent (immutable) sorted set operations: conj, disj, contains,
//! slice/rslice iteration, and storage persistence. Each mutation returns a new
//! set sharing structure with the original via `Rc<Node>`.

use std::cell::Cell;
use std::rc::Rc;

use crate::branch::Branch;
use crate::key::Key;
use crate::leaf::Leaf;
use crate::node::{Comparator, Node};
use crate::results::{AddResult, RemoveResult};
use crate::seq::{build_seq_ascending, build_seq_descending, Seq};
use crate::settings::Settings;
use crate::storage::StorageCell;

pub struct PersistentSortedSet {
    cmp: Rc<Comparator>,
    storage: Option<Rc<StorageCell>>,
    settings: Settings,
    address: Option<i64>,
    root: Option<Rc<Node>>,
    count: i64, // -1 = unknown (lazy), else cached count
    version: Rc<Cell<u64>>,
}

impl PersistentSortedSet {
    /// Create an empty set with the given comparator.
    pub fn empty(cmp: Rc<Comparator>) -> Self {
        let settings = Settings::default();
        Self {
            cmp,
            storage: None,
            settings: settings.clone(),
            address: None,
            root: Some(Rc::new(Node::Leaf(Leaf::new(settings)))),
            count: 0,
            version: Rc::new(Cell::new(0)),
        }
    }

    /// Create an empty set with comparator and custom settings (no storage).
    pub fn empty_with_settings(cmp: Rc<Comparator>, settings: Settings) -> Self {
        Self {
            cmp,
            storage: None,
            settings: settings.clone(),
            address: None,
            root: Some(Rc::new(Node::Leaf(Leaf::new(settings)))),
            count: 0,
            version: Rc::new(Cell::new(0)),
        }
    }

    /// Create an empty set with comparator and storage.
    pub fn with_storage(
        cmp: Rc<Comparator>,
        storage: Box<dyn crate::storage::IStorage>,
        settings: Settings,
    ) -> Self {
        Self {
            cmp,
            storage: Some(Rc::new(StorageCell::new(storage))),
            settings: settings.clone(),
            address: None,
            root: Some(Rc::new(Node::Leaf(Leaf::new(settings)))),
            count: 0,
            version: Rc::new(Cell::new(0)),
        }
    }

    /// Create from a sorted array of keys.
    pub fn from_sorted(keys: Vec<Key>, cmp: Rc<Comparator>) -> Self {
        let mut set = Self::empty(cmp);
        for key in &keys {
            set = set.conj(key);
        }
        set
    }

    /// Get root node, lazily loading from storage if needed.
    pub fn root(&self) -> Rc<Node> {
        if let Some(root) = &self.root {
            return Rc::clone(root);
        }
        if let (Some(addr), Some(storage)) = (self.address, &self.storage) {
            return storage.restore(addr);
        }
        panic!("PersistentSortedSet has no root and no address")
    }

    /// Check if this set has a storage backend.
    pub fn storage(&self) -> Option<&Rc<StorageCell>> {
        self.storage.as_ref()
    }

    /// Set the storage backend (for sets created without one).
    pub fn set_storage(&mut self, storage: Box<dyn crate::storage::IStorage>) {
        self.storage = Some(Rc::new(StorageCell::new(storage)));
    }

    pub fn contains(&self, key: &Key) -> bool {
        self.root()
            .contains(self.storage.as_ref().map(|s| s.as_ref()), key, &*self.cmp)
    }

    pub fn count(&self) -> usize {
        if self.count < 0 {
            // Lazy count — would need interior mutability to cache.
            // For now, recompute each time when lazy.
            self.root()
                .count(self.storage.as_ref().map(|s| s.as_ref()))
        } else {
            self.count as usize
        }
    }

    pub fn is_empty(&self) -> bool {
        self.count() == 0
    }

    fn alter_count(&self, delta: i64) -> i64 {
        if self.count < 0 {
            self.count
        } else {
            self.count + delta
        }
    }

    fn bump_version(&self) -> u64 {
        let v = self.version.get() + 1;
        self.version.set(v);
        v
    }

    /// Add key to set. Returns a new set (persistent) or mutates self (transient).
    pub fn conj(self, key: &Key) -> Self {
        let storage_ref = self.storage.as_ref().map(|s| s.as_ref());
        let result = self.root().add(storage_ref, key, &*self.cmp, &self.settings);

        match result {
            AddResult::Unchanged => self,

            AddResult::EarlyExit => {
                // Transient path — already mutated in place
                let new_count = self.alter_count(1);
                self.bump_version();
                Self {
                    cmp: self.cmp,
                    storage: self.storage,
                    settings: self.settings,
                    address: None,
                    root: self.root,
                    count: new_count,
                    version: self.version,
                }
            }

            AddResult::One(node) => {
                let new_count = self.alter_count(1);
                let new_version = self.version.get() + 1;
                Self {
                    cmp: self.cmp,
                    storage: self.storage,
                    settings: self.settings.clone(),
                    address: None,
                    root: Some(node),
                    count: new_count,
                    version: Rc::new(Cell::new(new_version)),
                }
            }

            AddResult::Split(left, right) => {
                let lk = left.max_key().clone();
                let rk = right.max_key().clone();
                let level = left.level() + 1;
                let new_root = Rc::new(Node::Branch(Branch::new(
                    level,
                    vec![lk, rk],
                    None,
                    Some(vec![Some(left), Some(right)]),
                    self.settings.clone(),
                )));
                let new_count = self.alter_count(1);
                let new_version = self.version.get() + 1;
                Self {
                    cmp: self.cmp,
                    storage: self.storage,
                    settings: self.settings.clone(),
                    address: None,
                    root: Some(new_root),
                    count: new_count,
                    version: Rc::new(Cell::new(new_version)),
                }
            }
        }
    }

    /// Remove key from set.
    pub fn disj(self, key: &Key) -> Self {
        let root = self.root();
        let result = {
            let storage_ref = self.storage.as_ref().map(|s| s.as_ref());
            root.remove(storage_ref, key, None, None, &*self.cmp, &self.settings)
        };

        match result {
            RemoveResult::Unchanged => self,

            RemoveResult::EarlyExit => {
                let new_count = self.alter_count(-1);
                self.bump_version();
                Self {
                    cmp: self.cmp,
                    storage: self.storage,
                    settings: self.settings,
                    address: None,
                    root: self.root,
                    count: new_count,
                    version: self.version,
                }
            }

            RemoveResult::Rebalanced {
                left: _,
                center,
                right: _,
            } => {
                // Root collapse: if root is Branch with len==1, replace with its child
                let new_root = match center.as_ref() {
                    Node::Branch(b) if b.len() == 1 => {
                        let storage_ref = self.storage.as_ref().map(|s| s.as_ref());
                        b.child(storage_ref, 0)
                    }
                    _ => center,
                };

                let new_count = self.alter_count(-1);
                let new_version = self.version.get() + 1;
                Self {
                    cmp: self.cmp,
                    storage: self.storage,
                    settings: self.settings.clone(),
                    address: None,
                    root: Some(new_root),
                    count: new_count,
                    version: Rc::new(Cell::new(new_version)),
                }
            }
        }
    }

    /// Forward iterator, optionally bounded [from, to] inclusive.
    pub fn slice(&self, from: Option<&Key>, to: Option<&Key>) -> Option<Seq> {
        build_seq_ascending(
            &self.root(),
            from,
            to,
            &self.cmp,
            &self.storage,
            self.version.get(),
            &self.version,
        )
    }

    /// Reverse iterator, optionally bounded [to, from] inclusive.
    pub fn rslice(&self, from: Option<&Key>, to: Option<&Key>) -> Option<Seq> {
        build_seq_descending(
            &self.root(),
            from,
            to,
            &self.cmp,
            &self.storage,
            self.version.get(),
            &self.version,
        )
    }

    pub fn seq(&self) -> Option<Seq> {
        self.slice(None, None)
    }

    pub fn rseq(&self) -> Option<Seq> {
        self.rslice(None, None)
    }

    pub fn to_vec(&self) -> Vec<Key> {
        match self.seq() {
            Some(s) => s.to_vec(),
            None => vec![],
        }
    }

    /// Convert to transient (editable) set.
    pub fn as_transient(self) -> Self {
        assert!(!self.settings.editable(), "Already transient");
        Self {
            cmp: self.cmp,
            storage: self.storage,
            settings: self.settings.editable_settings(),
            address: self.address,
            root: self.root,
            count: self.count,
            version: self.version,
        }
    }

    /// Convert transient set back to persistent.
    pub fn persistent(mut self) -> Self {
        assert!(self.settings.editable(), "Already persistent");
        self.settings.make_persistent();
        self
    }

    /// Store tree bottom-up into storage. Returns root address.
    pub fn store(&mut self) -> i64 {
        if let Some(addr) = self.address {
            return addr;
        }

        let storage = self
            .storage
            .as_ref()
            .expect("No storage backend provided");

        let root = self.root();
        let addr = root.store(storage);
        self.address = Some(addr);
        addr
    }

    /// Walk all addresses used by this tree.
    pub fn walk_addresses(&self, on_address: &mut dyn FnMut(i64) -> bool) {
        if let Some(addr) = self.address {
            if !on_address(addr) {
                return;
            }
        }
        self.root()
            .walk_addresses(self.storage.as_ref().map(|s| s.as_ref()), on_address);
    }

    /// Restore from storage address (lazy — root not loaded until accessed).
    pub fn restore(
        cmp: Rc<Comparator>,
        address: i64,
        storage: Box<dyn crate::storage::IStorage>,
        settings: Settings,
    ) -> Self {
        Self {
            cmp,
            storage: Some(Rc::new(StorageCell::new(storage))),
            settings,
            address: Some(address),
            root: None,
            count: -1,
            version: Rc::new(Cell::new(0)),
        }
    }

    pub fn comparator(&self) -> &Rc<Comparator> {
        &self.cmp
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }
}

impl Clone for PersistentSortedSet {
    fn clone(&self) -> Self {
        Self {
            cmp: Rc::clone(&self.cmp),
            storage: self.storage.clone(),
            settings: self.settings.clone(),
            address: self.address,
            root: self.root.clone(),
            count: self.count,
            version: Rc::clone(&self.version),
        }
    }
}
