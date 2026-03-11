use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

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
pub struct StorageCell {
    inner: RefCell<Box<dyn IStorage>>,
}

impl StorageCell {
    pub fn new(storage: Box<dyn IStorage>) -> Self {
        Self {
            inner: RefCell::new(storage),
        }
    }

    pub fn store(&self, node: &Node) -> i64 {
        self.inner.borrow_mut().store(node)
    }

    pub fn restore(&self, address: i64) -> Rc<Node> {
        self.inner.borrow().restore(address)
    }

    pub fn accessed(&self, address: i64) {
        self.inner.borrow().accessed(address)
    }

    pub fn list_addresses(&self) -> Vec<i64> {
        self.inner.borrow().list_addresses()
    }

    pub fn delete(&self, addresses: &[i64]) {
        self.inner.borrow_mut().delete(addresses)
    }
}
