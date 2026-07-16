//! VFS Sync module extracted from block_storage.rs
//! This module contains the ACTUAL VFS sync and global storage management logic

#[allow(unused_imports)]
use super::metadata::BlockMetadataPersist;
#[allow(unused_imports)]
use crate::types::DatabaseError;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

// Global storage for WASM to maintain data across instances
#[cfg(target_arch = "wasm32")]
thread_local! {
    pub static GLOBAL_STORAGE: RefCell<HashMap<String, HashMap<u64, Vec<u8>>>> = RefCell::new(HashMap::new());
    static GLOBAL_ALLOCATION_MAP: RefCell<HashMap<String, HashSet<u64>>> = RefCell::new(HashMap::new());
}

// Global storage mirrors for native builds
#[cfg(not(target_arch = "wasm32"))]
thread_local! {
    static GLOBAL_STORAGE_TEST: RefCell<HashMap<String, HashMap<u64, Vec<u8>>>> = RefCell::new(HashMap::new());
    static GLOBAL_ALLOCATION_MAP_TEST: RefCell<HashMap<String, HashSet<u64>>> = RefCell::new(HashMap::new());
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static GLOBAL_METADATA: RefCell<HashMap<String, HashMap<u64, BlockMetadataPersist>>> = RefCell::new(HashMap::new());
}
// Per-DB commit marker for WASM builds to simulate atomic commit semantics
#[cfg(target_arch = "wasm32")]
thread_local! {
    pub static GLOBAL_COMMIT_MARKER: RefCell<HashMap<String, u64>> = RefCell::new(HashMap::new());
}

// BEHAVE FIX (incremental sync): per-DB set of block ids written through the
// VFS since the last successful IndexedDB persist. `sync_internal` persists
// only these instead of re-writing every block of the database on every sync.
// Blocks restored FROM IndexedDB are deliberately not marked dirty.
#[cfg(target_arch = "wasm32")]
thread_local! {
    static GLOBAL_DIRTY_BLOCKS: RefCell<HashMap<String, HashSet<u64>>> = RefCell::new(HashMap::new());
}

#[cfg(not(target_arch = "wasm32"))]
thread_local! {
    static GLOBAL_DIRTY_BLOCKS_TEST: RefCell<HashMap<String, HashSet<u64>>> = RefCell::new(HashMap::new());
}

#[cfg(target_arch = "wasm32")]
fn with_dirty_blocks<F, R>(f: F) -> R
where
    F: FnOnce(&RefCell<HashMap<String, HashSet<u64>>>) -> R,
{
    GLOBAL_DIRTY_BLOCKS.with(f)
}

#[cfg(not(target_arch = "wasm32"))]
fn with_dirty_blocks<F, R>(f: F) -> R
where
    F: FnOnce(&RefCell<HashMap<String, HashSet<u64>>>) -> R,
{
    GLOBAL_DIRTY_BLOCKS_TEST.with(f)
}

/// Mark a block dirty (written since the last persist).
pub fn mark_block_dirty(db_name: &str, block_id: u64) {
    with_dirty_blocks(|d| {
        d.borrow_mut()
            .entry(db_name.to_string())
            .or_insert_with(HashSet::new)
            .insert(block_id);
    });
}

/// Take (and clear) the dirty block set for `db_name`.
/// On persist failure, call [`restore_dirty_blocks`] to put them back.
pub fn take_dirty_blocks(db_name: &str) -> HashSet<u64> {
    with_dirty_blocks(|d| d.borrow_mut().remove(db_name).unwrap_or_default())
}

/// Re-mark blocks dirty (used when a persist attempt fails).
pub fn restore_dirty_blocks(db_name: &str, blocks: HashSet<u64>) {
    with_dirty_blocks(|d| {
        d.borrow_mut()
            .entry(db_name.to_string())
            .or_insert_with(HashSet::new)
            .extend(blocks);
    });
}

/// Access to global storage for BlockStorage (internal use)
#[cfg(target_arch = "wasm32")]
pub fn with_global_storage<F, R>(f: F) -> R
where
    F: FnOnce(&RefCell<HashMap<String, HashMap<u64, Vec<u8>>>>) -> R,
{
    GLOBAL_STORAGE.with(f)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn with_global_storage<F, R>(f: F) -> R
where
    F: FnOnce(&RefCell<HashMap<String, HashMap<u64, Vec<u8>>>>) -> R,
{
    GLOBAL_STORAGE_TEST.with(f)
}

/// Access to global metadata for BlockStorage (internal use)
#[cfg(target_arch = "wasm32")]
pub fn with_global_metadata<F, R>(f: F) -> R
where
    F: FnOnce(&RefCell<HashMap<String, HashMap<u64, BlockMetadataPersist>>>) -> R,
{
    GLOBAL_METADATA.with(f)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn with_global_metadata<F, R>(f: F) -> R
where
    F: FnOnce(&parking_lot::Mutex<HashMap<String, HashMap<u64, BlockMetadataPersist>>>) -> R,
{
    // For native tests, use the shared GLOBAL_METADATA_TEST from block_storage
    use super::block_storage::GLOBAL_METADATA_TEST;
    GLOBAL_METADATA_TEST.with(f)
}

/// Access to global commit marker for BlockStorage (internal use)
#[cfg(target_arch = "wasm32")]
pub fn with_global_commit_marker<F, R>(f: F) -> R
where
    F: FnOnce(&RefCell<HashMap<String, u64>>) -> R,
{
    GLOBAL_COMMIT_MARKER.with(f)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn with_global_commit_marker<F, R>(f: F) -> R
where
    F: FnOnce(&RefCell<HashMap<String, u64>>) -> R,
{
    // For native tests, we need a test-only commit marker storage
    thread_local! {
        static GLOBAL_COMMIT_MARKER_TEST: RefCell<HashMap<String, u64>> = RefCell::new(HashMap::new());
    }
    GLOBAL_COMMIT_MARKER_TEST.with(f)
}

/// Access to allocation map (internal use)
#[cfg(target_arch = "wasm32")]
pub fn with_global_allocation_map<F, R>(f: F) -> R
where
    F: FnOnce(&RefCell<HashMap<String, HashSet<u64>>>) -> R,
{
    GLOBAL_ALLOCATION_MAP.with(f)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn with_global_allocation_map<F, R>(f: F) -> R
where
    F: FnOnce(&RefCell<HashMap<String, HashSet<u64>>>) -> R,
{
    GLOBAL_ALLOCATION_MAP_TEST.with(f)
}
