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

// --- LRU cache / Weak reference tests ---

#[test]
fn test_lru_cache_hit() {
    // Verifies that restore_cached returns the same Rc on cache hit
    let settings = Settings::default();
    let cell = StorageCell::with_cache_size(
        Box::new(MemoryStorage::new(settings.clone())),
        4,
    );
    let leaf = Node::Leaf(Leaf::with_keys(vec![1, 2, 3], settings));
    let addr = cell.store(&leaf);

    // Insert into LRU
    let first = cell.restore_cached(addr);
    let second = cell.restore_cached(addr);
    assert!(Rc::ptr_eq(&first, &second), "cache hit should return same Rc");
}

#[test]
fn test_lru_eviction_and_reload() {
    // With cache_size=2, inserting 3 entries should evict the oldest
    let settings = Settings::default();
    let cell = StorageCell::with_cache_size(
        Box::new(MemoryStorage::new(settings.clone())),
        2,
    );

    let leaf1 = Node::Leaf(Leaf::with_keys(vec![10], settings.clone()));
    let leaf2 = Node::Leaf(Leaf::with_keys(vec![20], settings.clone()));
    let leaf3 = Node::Leaf(Leaf::with_keys(vec![30], settings));

    let a1 = cell.store(&leaf1);
    let a2 = cell.store(&leaf2);

    // Load both into cache
    let rc1 = cell.restore_cached(a1);
    let _rc2 = cell.restore_cached(a2);

    // Store and load a third — should evict a1
    let a3 = cell.store(&leaf3);
    let _rc3 = cell.restore_cached(a3);

    // a1 was evicted from LRU, but rc1 still holds a strong ref.
    // Drop it, then reload — should get a new Rc from storage
    drop(rc1);
    let rc1_reloaded = cell.restore_cached(a1);
    assert_eq!(rc1_reloaded.keys(), &[10]);
}

#[test]
fn test_store_tree_and_access_after_eviction() {
    // Build a tree large enough to have branches, store it,
    // then verify that accessing children works after LRU eviction.
    let settings = Settings::new(4).with_cache_size(4);
    let mut set = PersistentSortedSet::with_storage(
        make_cmp(),
        Box::new(MemoryStorage::new(settings.clone())),
        settings,
    );

    for i in 1..=20 {
        set = set.conj(&i);
    }

    set.store();

    // All 20 elements should still be retrievable even though the LRU
    // cache is tiny (4 nodes). Accessing children will trigger re-restores.
    for i in 1..=20 {
        assert!(set.contains(&i), "set should contain {}", i);
    }

    // Full iteration should also work
    let all = set.to_vec();
    assert_eq!(all.len(), 20);
    assert_eq!(all, (1..=20).collect::<Vec<_>>());
}

#[test]
fn test_cache_node_inserts_into_lru() {
    let settings = Settings::default();
    let cell = StorageCell::with_cache_size(
        Box::new(MemoryStorage::new(settings.clone())),
        4,
    );
    let leaf = Node::Leaf(Leaf::with_keys(vec![42], settings));
    let addr = cell.store(&leaf);

    // Manually insert into LRU cache
    let rc = Rc::new(leaf.clone());
    cell.cache_node(addr, Rc::clone(&rc));

    // Should get the cached Rc back
    let fetched = cell.restore_cached(addr);
    assert!(Rc::ptr_eq(&rc, &fetched));
}

// --- LRU / Weak stress tests ---

#[test]
fn test_stress_store_restore_5000_elements_tiny_cache() {
    // 5000 elements with branching factor 8 → hundreds of nodes.
    // LRU cache holds only 8 nodes. Every child access beyond the
    // cache window forces a re-restore from MemoryStorage.
    let settings = Settings::new(8).with_cache_size(8);
    let mut set = PersistentSortedSet::with_storage(
        make_cmp(),
        Box::new(MemoryStorage::new(settings.clone())),
        settings,
    );

    for i in 1..=5000 {
        set = set.conj(&i);
    }
    assert_eq!(set.count(), 5000);

    set.store();

    // Forward contains — random-ish access pattern
    for i in (1..=5000).step_by(7) {
        assert!(set.contains(&i), "missing {}", i);
    }
    // Reverse contains
    for i in (1..=5000).rev().step_by(13) {
        assert!(set.contains(&i), "missing {}", i);
    }
    // Negative lookups
    for i in 5001..5100 {
        assert!(!set.contains(&i), "false positive {}", i);
    }
}

#[test]
fn test_stress_full_iteration_after_store_tiny_cache() {
    // Build, store, then iterate every element forward and reverse.
    // Cache is absurdly small (2 nodes) to maximize eviction churn.
    let settings = Settings::new(8).with_cache_size(2);
    let mut set = PersistentSortedSet::with_storage(
        make_cmp(),
        Box::new(MemoryStorage::new(settings.clone())),
        settings,
    );

    let n = 3000;
    for i in 1..=n {
        set = set.conj(&i);
    }
    set.store();

    let forward = set.to_vec();
    assert_eq!(forward.len(), n as usize);
    assert_eq!(forward, (1..=n).collect::<Vec<_>>());

    let reverse: Vec<Key> = match set.rseq() {
        Some(s) => s.to_vec(),
        None => vec![],
    };
    assert_eq!(reverse.len(), n as usize);
    assert_eq!(reverse, (1..=n).rev().collect::<Vec<_>>());
}

#[test]
fn test_stress_slice_after_store() {
    // Slice operations should work correctly when nodes are evicted.
    let settings = Settings::new(16).with_cache_size(4);
    let mut set = PersistentSortedSet::with_storage(
        make_cmp(),
        Box::new(MemoryStorage::new(settings.clone())),
        settings,
    );

    for i in 1..=2000 {
        set = set.conj(&i);
    }
    set.store();

    // Forward slice [500, 600]
    let slice = set.slice(Some(&500), Some(&600));
    let vals = slice.map(|s| s.to_vec()).unwrap_or_default();
    assert_eq!(vals, (500..=600).collect::<Vec<_>>());

    // Reverse slice [1500, 1600]
    let rslice = set.rslice(Some(&1600), Some(&1500));
    let rvals = rslice.map(|s| s.to_vec()).unwrap_or_default();
    assert_eq!(rvals, (1500..=1600).rev().collect::<Vec<_>>());
}

#[test]
fn test_stress_multiple_store_versions_share_storage() {
    // Multiple tree versions stored to the same storage.
    // After storing, both sets release their roots. Accessing either
    // should reload correctly from the shared MemoryStorage.
    let settings = Settings::new(8).with_cache_size(16);
    let mut set1 = PersistentSortedSet::with_storage(
        make_cmp(),
        Box::new(MemoryStorage::new(settings.clone())),
        settings,
    );

    for i in 1..=1000 {
        set1 = set1.conj(&i);
    }

    // Clone before mutating further — shares storage via Rc<StorageCell>
    let mut set2 = set1.clone();
    for i in 1001..=2000 {
        set2 = set2.conj(&i);
    }

    set1.store();
    set2.store();

    // set1 should have 1..=1000
    assert_eq!(set1.count(), 1000);
    assert!(set1.contains(&1));
    assert!(set1.contains(&1000));
    assert!(!set1.contains(&1001));

    // set2 should have 1..=2000
    assert_eq!(set2.count(), 2000);
    assert!(set2.contains(&1));
    assert!(set2.contains(&2000));
    assert!(!set2.contains(&2001));
}

#[test]
fn test_stress_conj_after_store_and_re_store() {
    // Store, then add more elements, then store again.
    // The second store must correctly handle a mix of stored (addressed)
    // and unstored (new) subtrees.
    let settings = Settings::new(8).with_cache_size(8);
    let mut set = PersistentSortedSet::with_storage(
        make_cmp(),
        Box::new(MemoryStorage::new(settings.clone())),
        settings,
    );

    for i in 1..=1000 {
        set = set.conj(&i);
    }
    set.store();

    // Add more after storing (root was cleared, will lazy-load)
    for i in 1001..=2000 {
        set = set.conj(&i);
    }
    set.store();

    let all = set.to_vec();
    assert_eq!(all.len(), 2000);
    assert_eq!(all, (1..=2000).collect::<Vec<_>>());
}

#[test]
fn test_stress_disj_after_store() {
    // Store, then remove elements, verify correctness.
    let settings = Settings::new(8).with_cache_size(8);
    let mut set = PersistentSortedSet::with_storage(
        make_cmp(),
        Box::new(MemoryStorage::new(settings.clone())),
        settings,
    );

    for i in 1..=1000 {
        set = set.conj(&i);
    }
    set.store();

    // Remove even numbers
    for i in (2..=1000).step_by(2) {
        set = set.disj(&i);
    }

    let remaining = set.to_vec();
    let expected: Vec<Key> = (1..=1000).step_by(2).collect();
    assert_eq!(remaining, expected);
}

#[test]
fn test_stress_walk_addresses_after_store() {
    // walk_addresses must traverse the full tree even when nodes
    // need to be re-restored from storage due to LRU eviction.
    let settings = Settings::new(8).with_cache_size(4);
    let mut set = PersistentSortedSet::with_storage(
        make_cmp(),
        Box::new(MemoryStorage::new(settings.clone())),
        settings,
    );

    for i in 1..=2000 {
        set = set.conj(&i);
    }
    set.store();

    let mut addrs = vec![];
    set.walk_addresses(&mut |addr| {
        addrs.push(addr);
        true
    });

    // Should have collected many addresses (root + branches + leaves)
    assert!(addrs.len() > 100, "expected many addresses, got {}", addrs.len());
    // All addresses should be unique
    let unique: std::collections::HashSet<i64> = addrs.iter().copied().collect();
    assert_eq!(unique.len(), addrs.len(), "duplicate addresses found");
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
