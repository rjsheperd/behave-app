//! Self-sufficient persistence lifecycle for WasmDataScript.
//!
//! Owns one `absurder_sql::Database` per db name in a thread-local registry.
//! Opening a database here does everything the CLJS `sql/connect!` used to:
//! name normalization, IndexedDB-VFS registration, IndexedDB -> memory block
//! restore, and pooled sqlite3 open. While a database is registered, its
//! pooled connection is guaranteed to exist, so `LegacyStorage` /
//! `UnifiedSQLiteStorage` lookups always hit the IndexedDB-backed connection
//! instead of silently falling back to an in-memory VFS.
//!
//! All entry points return `Result<_, String>`; the wasm layer converts
//! errors to JS exceptions (fail loud, never panic).

#![cfg(target_arch = "wasm32")]

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use absurder_sql::{Database, DatabaseConfig};

thread_local! {
    static DATABASES: RefCell<HashMap<String, Rc<RefCell<Database>>>> =
        RefCell::new(HashMap::new());
}

/// Normalize to the `.db`-suffixed name AbsurderSQL uses internally.
fn normalize(db_name: &str) -> String {
    if db_name.ends_with(".db") {
        db_name.to_string()
    } else {
        format!("{}.db", db_name)
    }
}

fn get(db_name: &str) -> Option<Rc<RefCell<Database>>> {
    let key = normalize(db_name);
    DATABASES.with(|dbs| dbs.borrow().get(&key).cloned())
}

/// Whether `open` has been called (and `close` has not) for this name.
pub fn is_open(db_name: &str) -> bool {
    get(db_name).is_some()
}

/// Open (or reuse) the AbsurderSQL database for `db_name`.
///
/// Mirrors `Database.newDatabase` (the CLJS `sql/connect!` path): same
/// config, plus `allow_non_leader_writes(true)` for single-tab apps.
/// Idempotent — reopening an already-open name is a no-op.
pub async fn ensure_open(db_name: &str) -> Result<(), String> {
    let key = normalize(db_name);
    if is_open(&key) {
        return Ok(());
    }

    let config = DatabaseConfig {
        name: key.clone(),
        version: Some(1),
        cache_size: Some(10_000),
        page_size: Some(4096),
        auto_vacuum: Some(true),
        journal_mode: Some("WAL".to_string()),
        max_export_size_bytes: Some(2 * 1024 * 1024 * 1024),
    };

    let mut db = Database::new(config)
        .await
        .map_err(|e| format!("Failed to open database {}: {}", key, e))?;
    db.allow_non_leader_writes(true)
        .await
        .map_err(|e| format!("Failed to set allowNonLeaderWrites for {}: {:?}", key, e))?;

    DATABASES.with(|dbs| {
        dbs.borrow_mut().insert(key, Rc::new(RefCell::new(db)));
    });
    Ok(())
}

/// Borrow the database mutably for one async operation.
///
/// WASM is single-threaded, but holding a `RefCell` borrow across an await
/// point would panic on reentrancy — so overlapping operations on the same
/// database fail loudly instead.
async fn with_db_mut<F, Fut, T>(db_name: &str, op_name: &str, f: F) -> Result<T, String>
where
    F: FnOnce(Rc<RefCell<Database>>) -> Fut,
    Fut: std::future::Future<Output = Result<T, String>>,
{
    let db = get(db_name).ok_or_else(|| {
        format!(
            "Database {} is not open — call WasmDataScript.open() (or importDb) first",
            normalize(db_name)
        )
    })?;
    // Surface concurrent use as an error up front (best effort; the actual
    // borrow happens inside `f`).
    if db.try_borrow_mut().is_err() {
        return Err(format!(
            "{}: another operation is in progress for {}",
            op_name,
            normalize(db_name)
        ));
    }
    f(db).await
}

/// Flush the write-back VFS cache to IndexedDB (WAL checkpoint + block persist).
pub async fn sync(db_name: &str) -> Result<(), String> {
    with_db_mut(db_name, "sync", |db| async move {
        db.borrow_mut()
            .sync_internal()
            .await
            .map_err(|e| format!("sync failed: {}", e))
    })
    .await
}

/// Sync, then export the database as SQLite file bytes.
pub async fn export(db_name: &str) -> Result<js_sys::Uint8Array, String> {
    with_db_mut(db_name, "export", |db| async move {
        db.borrow_mut()
            .sync_internal()
            .await
            .map_err(|e| format!("sync before export failed: {}", e))?;
        db.borrow()
            .export_to_file()
            .await
            .map_err(|e| format!("export failed: {:?}", e))
    })
    .await
}

/// Import SQLite file bytes into the database (replaces its contents).
pub async fn import(db_name: &str, bytes: js_sys::Uint8Array) -> Result<(), String> {
    with_db_mut(db_name, "import", |db| async move {
        db.borrow_mut()
            .import_from_file(bytes)
            .await
            .map_err(|e| format!("import failed: {:?}", e))
    })
    .await
}

/// Sync and close the database, releasing its pooled connection reference.
pub async fn close(db_name: &str) -> Result<(), String> {
    // Best-effort flush before closing; a missing db is not an error here.
    if is_open(db_name) {
        sync(db_name).await?;
    }
    let key = normalize(db_name);
    DATABASES.with(|dbs| {
        dbs.borrow_mut().remove(&key);
    });
    Ok(())
}
