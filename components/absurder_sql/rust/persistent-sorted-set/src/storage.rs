use std::cell::RefCell;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::rc::Rc;

use lru::LruCache;

use crate::key::Key;
use crate::node::Node;
use crate::settings::Settings;

/// Storage backend for persisting and restoring tree nodes.
/// Nodes are stored bottom-up (children before parents).
/// Addresses are i64 (SQLite rowids or auto-incremented IDs).
pub trait IStorage {
    /// Store a node, return its address.
    fn store(&mut self, node: &Node) -> i64;

    /// Restore a node by address.
    fn restore(&self, address: i64) -> Rc<Node>;

    /// Hint that an address was accessed (for LRU tracking).
    fn accessed(&self, _address: i64) {}

    /// List all stored addresses (for garbage collection).
    fn list_addresses(&self) -> Vec<i64>;

    /// Delete addresses no longer in use.
    fn delete(&mut self, addresses: &[i64]);
}

/// Serialized node representation
struct StoredNode {
    level: u32,
    keys: Vec<Key>,
    addresses: Option<Vec<i64>>,
}

/// In-memory storage for testing (no SQLite dependency)
pub struct MemoryStorage {
    nodes: HashMap<i64, StoredNode>,
    next_addr: i64,
    settings: Settings,
}

impl MemoryStorage {
    pub fn new(settings: Settings) -> Self {
        Self {
            nodes: HashMap::new(),
            next_addr: 1,
            settings,
        }
    }
}

impl IStorage for MemoryStorage {
    fn store(&mut self, node: &Node) -> i64 {
        let addr = self.next_addr;
        self.next_addr += 1;

        let stored = match node {
            Node::Leaf(leaf) => StoredNode {
                level: 0,
                keys: leaf.keys.clone(),
                addresses: None,
            },
            Node::Branch(branch) => {
                let addrs: Vec<i64> = (0..branch.len())
                    .map(|i| branch.address(i).expect("branch child must be stored before parent"))
                    .collect();
                StoredNode {
                    level: branch.level,
                    keys: branch.keys.clone(),
                    addresses: Some(addrs),
                }
            }
        };

        self.nodes.insert(addr, stored);
        addr
    }

    fn restore(&self, address: i64) -> Rc<Node> {
        let stored = self.nodes.get(&address).expect("address not found in MemoryStorage");
        let node = Node::restore(
            stored.level,
            stored.keys.clone(),
            stored.addresses.clone(),
            &self.settings,
        );
        Rc::new(node)
    }

    fn list_addresses(&self) -> Vec<i64> {
        self.nodes.keys().copied().collect()
    }

    fn delete(&mut self, addresses: &[i64]) {
        for addr in addresses {
            self.nodes.remove(addr);
        }
    }
}

/// Wrapper that provides interior mutability for storage behind shared references.
/// This is needed because tree traversal takes &self but storage operations need &mut self.
/// Includes an LRU cache that is the single source of strong `Rc<Node>` references
/// for stored (persisted) nodes. Branch children hold `Weak<Node>` references;
/// when the LRU evicts an entry, the node can be freed and re-loaded on demand.
pub struct StorageCell {
    inner: RefCell<Box<dyn IStorage>>,
    cache: RefCell<LruCache<i64, Rc<Node>>>,
}

impl StorageCell {
    pub fn new(storage: Box<dyn IStorage>) -> Self {
        Self::with_cache_size(storage, 1024)
    }

    pub fn with_cache_size(storage: Box<dyn IStorage>, cache_size: usize) -> Self {
        let cap = NonZeroUsize::new(cache_size)
            .unwrap_or(NonZeroUsize::new(1024).unwrap());
        Self {
            inner: RefCell::new(storage),
            cache: RefCell::new(LruCache::new(cap)),
        }
    }

    pub fn store(&self, node: &Node) -> i64 {
        let addr = self.inner.borrow_mut().store(node);
        // Cache a clean copy with addresses but no children references.
        // This ensures child access goes through restore_cached (LRU-managed)
        // rather than holding Strong refs to the original subtree.
        self.cache.borrow_mut().put(addr, Rc::new(Node::storage_copy(node)));
        addr
    }

    /// Restore a node, checking the LRU cache first.
    /// On cache miss, delegates to the inner storage and caches the result.
    pub fn restore_cached(&self, address: i64) -> Rc<Node> {
        {
            let mut cache = self.cache.borrow_mut();
            if let Some(node) = cache.get(&address) {
                return Rc::clone(node);
            }
        }
        let node = self.inner.borrow().restore(address);
        self.cache.borrow_mut().put(address, Rc::clone(&node));
        node
    }

    /// Restore without LRU cache (direct from storage).
    pub fn restore(&self, address: i64) -> Rc<Node> {
        self.restore_cached(address)
    }

    /// Insert a node into the LRU cache under the given address.
    /// Used after storing a child to keep it alive in cache while
    /// the Branch downgrades its reference to Weak.
    pub fn cache_node(&self, address: i64, node: Rc<Node>) {
        self.cache.borrow_mut().put(address, node);
    }

    /// Bump an address in the LRU (mark as recently used).
    pub fn accessed(&self, address: i64) {
        {
            let mut cache = self.cache.borrow_mut();
            cache.get(&address); // promotes to most-recently-used
        }
        self.inner.borrow().accessed(address);
    }

    pub fn list_addresses(&self) -> Vec<i64> {
        self.inner.borrow().list_addresses()
    }

    pub fn delete(&self, addresses: &[i64]) {
        {
            let mut cache = self.cache.borrow_mut();
            for addr in addresses {
                cache.pop(addr);
            }
        }
        self.inner.borrow_mut().delete(addresses)
    }
}
