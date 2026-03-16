//! Persistent Sorted Set — a B+ tree with structural sharing.
//!
//! Compiles to native (i64 keys, rusqlite storage) for testing,
//! and to WASM (JsValue keys, JS callback storage) for use from ClojureScript.

pub mod key;
pub mod results;
pub mod settings;
pub mod node;
pub mod leaf;
pub mod branch;
pub mod seq;
pub mod chunk;
pub mod storage;
pub mod sqlite_storage;
pub mod set;
pub mod datom;
pub mod datom_serde;
pub mod comparator;
pub mod schema;
pub mod db;
pub mod legacy_edn;
pub mod relation;
pub mod query_parser;
pub mod pull_parser;
pub mod pull;
pub mod transact;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

#[cfg(target_arch = "wasm32")]
pub mod js_storage;

// wasm_sqlite_storage is now in datascript-rs as unified_storage


// Re-export main types
pub use key::Key;
pub use set::PersistentSortedSet;
pub use settings::Settings;
pub use storage::{IStorage, MemoryStorage, StorageCell};
pub use node::Node;
pub use seq::Seq;
pub use chunk::Chunk;

#[cfg(not(target_arch = "wasm32"))]
pub use sqlite_storage::SQLiteStorage;
