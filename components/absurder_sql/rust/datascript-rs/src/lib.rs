//! datascript-rs: Unified crate combining AbsurderSQL and PersistentSortedSet.
//!
//! Single WASM binary that provides:
//! - AbsurderSQL: IndexedDB VFS + SQL (Database, connection pool)
//! - PSS: B+ tree indexes (WasmPSS, WasmSeq)
//! - UnifiedSQLiteStorage: PSS nodes stored directly in AbsurderSQL's SQLite

use wasm_bindgen::prelude::*;

// Force absurder-sql's wasm-bindgen descriptors into the final binary.
// Its #[wasm_bindgen] exports (Database, etc.) are discovered from custom sections.
extern crate absurder_sql;

/// Install panic hook so Rust panics show as console.error instead of "unreachable".
#[wasm_bindgen(js_name = "setPanicHook")]
pub fn set_panic_hook() {
    console_error_panic_hook::set_once();
}

// Re-export PSS types so their wasm-bindgen exports are linked.
pub use persistent_sorted_set::wasm::{WasmPSS, WasmSeq};

pub mod persistence;
pub mod unified_storage;
pub mod wasm_datascript;
pub mod legacy_datascript;
pub mod query;

// Re-export WasmDataScript so its wasm-bindgen exports are linked.
pub use wasm_datascript::WasmDataScript;

use persistent_sorted_set::comparator::parse_index_type;
use persistent_sorted_set::settings::Settings;
use unified_storage::UnifiedSQLiteStorage;

// ---------------------------------------------------------------------------
// Settings parsing (duplicated from PSS wasm.rs since it's not pub there)
// ---------------------------------------------------------------------------

fn parse_settings(settings: &JsValue) -> Settings {
    let bf = js_sys::Reflect::get(settings, &JsValue::from_str("branchingFactor"))
        .ok()
        .and_then(|v| v.as_f64())
        .map(|n| n as usize)
        .unwrap_or(512);

    let cache_size = js_sys::Reflect::get(settings, &JsValue::from_str("cacheSize"))
        .ok()
        .and_then(|v| v.as_f64())
        .map(|n| n as usize)
        .unwrap_or(0);

    let s = Settings::new(bf);
    if cache_size > 0 { s.with_cache_size(cache_size) } else { s }
}

// ---------------------------------------------------------------------------
// Unified constructors: PSS with AbsurderSQL's IndexedDB-backed SQLite
// ---------------------------------------------------------------------------

/// Create an empty PSS with IndexType comparator and unified SQLite storage.
/// The `db_name` must match an OPEN AbsurderSQL database (throws otherwise).
#[wasm_bindgen(js_name = "withUnifiedSqlite")]
pub fn with_unified_sqlite(
    index_type: String,
    db_name: String,
    settings: JsValue,
) -> Result<WasmPSS, JsValue> {
    let idx = parse_index_type(&index_type);
    let rust_settings = parse_settings(&settings);
    let storage = UnifiedSQLiteStorage::new(&db_name, rust_settings.clone())
        .map_err(|e| JsValue::from_str(&e))?;

    Ok(WasmPSS::new_with_storage(idx, Box::new(storage), rust_settings))
}

/// Restore a PSS from unified SQLite storage by root address.
/// The `db_name` must match an OPEN AbsurderSQL database (throws otherwise).
#[wasm_bindgen(js_name = "restoreFromUnifiedSqlite")]
pub fn restore_from_unified_sqlite(
    index_type: String,
    address: f64,
    db_name: String,
    settings: JsValue,
) -> Result<WasmPSS, JsValue> {
    let idx = parse_index_type(&index_type);
    let rust_settings = parse_settings(&settings);
    let storage = UnifiedSQLiteStorage::new(&db_name, rust_settings.clone())
        .map_err(|e| JsValue::from_str(&e))?;

    Ok(WasmPSS::new_restored(idx, address as i64, Box::new(storage), rust_settings))
}

/// Close all unified SQLite connections managed by PSS. Call during teardown.
#[wasm_bindgen(js_name = "closeUnifiedConnections")]
pub fn close_unified_connections() {
    unified_storage::close_all();
}
