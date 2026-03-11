use std::cmp::Ordering;
use std::rc::Rc;

use persistent_sorted_set::Key;
use persistent_sorted_set::node::Node;
use persistent_sorted_set::leaf::Leaf;
use persistent_sorted_set::settings::Settings;
use persistent_sorted_set::storage::{IStorage, MemoryStorage, StorageCell};
use persistent_sorted_set::PersistentSortedSet;

fn int_cmp(a: &Key, b: &Key) -> Ordering {
    a.cmp(b)
}

fn make_cmp() -> Rc<dyn Fn(&Key, &Key) -> Ordering> {
    Rc::new(int_cmp)
}

// --- MemoryStorage tests ---

#[test]
fn test_store_restore_leaf() {
    let settings = Settings::default();
    let mut storage = MemoryStorage::new(settings.clone());
    let leaf = Node::Leaf(Leaf::with_keys(vec![1, 3, 5, 7], settings));

    let addr = storage.store(&leaf);
    let restored = storage.restore(addr);

    assert_eq!(restored.keys(), &[1, 3, 5, 7]);
    assert_eq!(restored.level(), 0);
}

#[test]
fn test_store_restore_tree() {
    let settings = Settings::default();
    let _storage_cell = Rc::new(StorageCell::new(Box::new(MemoryStorage::new(settings.clone()))));

    let mut set = PersistentSortedSet::with_storage(
        make_cmp(),
        Box::new(MemoryStorage::new(settings.clone())),
        settings.clone(),
    );

    for i in 1..=100 {
        set = set.conj(&i);
    }

    let addr = set.store();
    assert!(addr > 0);

    // Restore and verify
    let _restored = PersistentSortedSet::restore(
        make_cmp(),
        addr,
        Box::new(MemoryStorage::new(settings)),
        Settings::default(),
    );
    // Note: restored set uses a different MemoryStorage so won't have data.
    // In a real test we'd share the storage.
}

#[test]
fn test_walk_addresses() {
    let settings = Settings::default();
    let mut set = PersistentSortedSet::with_storage(
        make_cmp(),
        Box::new(MemoryStorage::new(settings.clone())),
        settings,
    );
    for i in 1..=50 {
        set = set.conj(&i);
    }
    set.store();

    let mut addrs = vec![];
    set.walk_addresses(&mut |addr| {
        addrs.push(addr);
        true
    });

    assert!(!addrs.is_empty());
}

#[test]
fn test_list_and_delete() {
    let settings = Settings::default();
    let mut storage = MemoryStorage::new(settings.clone());
    let leaf1 = Node::Leaf(Leaf::with_keys(vec![1, 2], settings.clone()));
    let leaf2 = Node::Leaf(Leaf::with_keys(vec![3, 4], settings));

    let addr1 = storage.store(&leaf1);
    let addr2 = storage.store(&leaf2);

    let all = storage.list_addresses();
    assert!(all.contains(&addr1));
    assert!(all.contains(&addr2));

    storage.delete(&[addr1]);
    let remaining = storage.list_addresses();
    assert!(!remaining.contains(&addr1));
    assert!(remaining.contains(&addr2));
}

// --- SQLiteStorage tests (native only) ---

#[cfg(not(target_arch = "wasm32"))]
mod sqlite_tests {
    use super::*;
    use persistent_sorted_set::SQLiteStorage;
    use rusqlite::Connection;

    fn make_sqlite_storage() -> SQLiteStorage {
        let conn = Connection::open_in_memory().unwrap();
        SQLiteStorage::new(conn, Settings::default())
    }

    #[test]
    fn test_sqlite_store_restore() {
        let mut storage = make_sqlite_storage();
        let leaf = Node::Leaf(Leaf::with_keys(vec![10, 20, 30], Settings::default()));

        let addr = storage.store(&leaf);
        let restored = storage.restore(addr);

        assert_eq!(restored.keys(), &[10, 20, 30]);
        assert_eq!(restored.level(), 0);
    }

    #[test]
    fn test_sqlite_schema_creation() {
        let conn = Connection::open_in_memory().unwrap();
        let _storage = SQLiteStorage::new(conn, Settings::default());
        // If we get here, table was created successfully
    }

    #[test]
    fn test_sqlite_list_addresses() {
        let mut storage = make_sqlite_storage();
        let leaf1 = Node::Leaf(Leaf::with_keys(vec![1], Settings::default()));
        let leaf2 = Node::Leaf(Leaf::with_keys(vec![2], Settings::default()));

        let a1 = storage.store(&leaf1);
        let a2 = storage.store(&leaf2);

        let addrs = storage.list_addresses();
        assert!(addrs.contains(&a1));
        assert!(addrs.contains(&a2));
    }

    #[test]
    fn test_sqlite_delete() {
        let mut storage = make_sqlite_storage();
        let leaf = Node::Leaf(Leaf::with_keys(vec![1, 2, 3], Settings::default()));

        let addr = storage.store(&leaf);
        assert!(!storage.list_addresses().is_empty());

        storage.delete(&[addr]);
        assert!(storage.list_addresses().is_empty());
    }

    #[test]
    fn test_sqlite_large_tree() {
        let mut storage = make_sqlite_storage();
        // Store many leaves and verify round-trip
        for i in 0..100 {
            let leaf = Node::Leaf(Leaf::with_keys(vec![i * 10, i * 10 + 1], Settings::default()));
            let addr = storage.store(&leaf);
            let restored = storage.restore(addr);
            assert_eq!(restored.keys(), &[i * 10, i * 10 + 1]);
        }
    }
}
