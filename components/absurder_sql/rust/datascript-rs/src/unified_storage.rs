//! UnifiedSQLiteStorage — PSS node storage using AbsurderSQL's connection pool.
//!
//! Instead of opening its own SQLite connection (like WasmSQLiteStorage),
//! this implementation gets `*mut sqlite3` from AbsurderSQL's connection pool.
//! This means PSS nodes are stored in the same database that AbsurderSQL manages,
//! and automatically persist to IndexedDB via AbsurderSQL's VFS.

use std::ffi::CString;
use std::num::NonZeroUsize;
use std::ptr;
use std::rc::Rc;

use lru::LruCache;

use sqlite_wasm_rs::{
    sqlite3, sqlite3_stmt,
    sqlite3_open_v2,
    sqlite3_prepare_v2, sqlite3_step, sqlite3_finalize,
    sqlite3_bind_int64, sqlite3_bind_blob, sqlite3_bind_null,
    sqlite3_column_int64, sqlite3_column_blob, sqlite3_column_bytes, sqlite3_column_type,
    sqlite3_exec, sqlite3_errmsg, sqlite3_last_insert_rowid,
    SQLITE_OK, SQLITE_ROW, SQLITE_DONE,
    SQLITE_OPEN_READWRITE, SQLITE_OPEN_CREATE,
    SQLITE_NULL, SQLITE_TRANSIENT,
};

use absurder_sql::connection_pool::{self, ConnectionState};

use persistent_sorted_set::datom_serde;
use persistent_sorted_set::key::Key;
use persistent_sorted_set::node::Node;
use persistent_sorted_set::settings::Settings;
use persistent_sorted_set::storage::IStorage;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

unsafe fn exec_sql(db: *mut sqlite3, sql: &str) {
    let c_sql = CString::new(sql).unwrap();
    let ret = unsafe { sqlite3_exec(db, c_sql.as_ptr(), None, ptr::null_mut(), ptr::null_mut()) };
    if ret != SQLITE_OK {
        let msg = unsafe { std::ffi::CStr::from_ptr(sqlite3_errmsg(db)) }
            .to_string_lossy()
            .into_owned();
        panic!("sqlite3_exec failed ({}): {}", ret, msg);
    }
}

struct PreparedStmt {
    stmt: *mut sqlite3_stmt,
}

impl PreparedStmt {
    unsafe fn new(db: *mut sqlite3, sql: &str) -> Self {
        let c_sql = CString::new(sql).unwrap();
        let mut stmt: *mut sqlite3_stmt = ptr::null_mut();
        let ret = unsafe {
            sqlite3_prepare_v2(
                db,
                c_sql.as_ptr(),
                sql.len() as i32,
                &mut stmt,
                ptr::null_mut(),
            )
        };
        if ret != SQLITE_OK {
            let msg = unsafe { std::ffi::CStr::from_ptr(sqlite3_errmsg(db)) }
                .to_string_lossy()
                .into_owned();
            panic!("sqlite3_prepare_v2 failed ({}): {}", ret, msg);
        }
        PreparedStmt { stmt }
    }
}

impl Drop for PreparedStmt {
    fn drop(&mut self) {
        unsafe { sqlite3_finalize(self.stmt); }
    }
}

// ---------------------------------------------------------------------------
// UnifiedSQLiteStorage
// ---------------------------------------------------------------------------

pub struct UnifiedSQLiteStorage {
    conn: Rc<ConnectionState>,
    db_name: String,
    cache: LruCache<i64, Rc<Node>>,
    settings: Settings,
}

impl UnifiedSQLiteStorage {
    pub fn new(db_name: &str, settings: Settings) -> Self {
        let db_name_owned = db_name.to_string();
        // AbsurderSQL strips ".db" for the pool key — match that convention
        let pool_key = db_name.trim_end_matches(".db");

        // Get or create a connection via AbsurderSQL's pool.
        // If AbsurderSQL has already opened this database (with IndexedDB VFS),
        // we reuse that connection. Otherwise, fall back to a basic open.
        let conn = connection_pool::get_or_create_connection(pool_key, {
            let name = db_name_owned.clone();
            move || {
                let c_path = CString::new(name.as_str())
                    .map_err(|e| e.to_string())?;
                let mut db: *mut sqlite3 = ptr::null_mut();
                let ret = unsafe {
                    sqlite3_open_v2(
                        c_path.as_ptr(),
                        &mut db,
                        SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE,
                        ptr::null(),
                    )
                };
                if ret != SQLITE_OK {
                    return Err(format!("Failed to open SQLite: {}", db_name));
                }
                Ok(db)
            }
        }).expect("Failed to get or create SQLite connection");

        let db = conn.db.get();

        // Create the pss_nodes table if it doesn't exist
        unsafe {
            exec_sql(db,
                "CREATE TABLE IF NOT EXISTS pss_nodes (
                    addr INTEGER PRIMARY KEY,
                    level INTEGER NOT NULL,
                    keys BLOB NOT NULL,
                    addrs BLOB
                )",
            );
        }

        let cache_size = settings.cache_size();
        UnifiedSQLiteStorage {
            conn,
            db_name: db_name_owned,
            cache: LruCache::new(
                NonZeroUsize::new(cache_size).unwrap_or(NonZeroUsize::new(1024).unwrap()),
            ),
            settings,
        }
    }
}

impl UnifiedSQLiteStorage {
    /// Store metadata at a specific address (INSERT OR REPLACE).
    /// Used for the metadata row at addr=0.
    pub fn store_metadata(&mut self, addr: i64, data: &[u8]) {
        let db = self.conn.db.get();
        unsafe {
            let ps = PreparedStmt::new(
                db,
                "INSERT OR REPLACE INTO pss_nodes (addr, level, keys, addrs) VALUES (?1, -1, ?2, NULL)",
            );
            sqlite3_bind_int64(ps.stmt, 1, addr);
            sqlite3_bind_blob(
                ps.stmt,
                2,
                data.as_ptr() as *const _,
                data.len() as i32,
                SQLITE_TRANSIENT(),
            );
            let ret = sqlite3_step(ps.stmt);
            assert_eq!(ret, SQLITE_DONE, "failed to store metadata: {}", ret);
        }
    }

    /// Restore metadata from a specific address.
    /// Returns `None` if no row exists at the given address.
    pub fn restore_metadata(&self, addr: i64) -> Option<Vec<u8>> {
        let db = self.conn.db.get();
        unsafe {
            let ps = PreparedStmt::new(
                db,
                "SELECT keys FROM pss_nodes WHERE addr = ?1",
            );
            sqlite3_bind_int64(ps.stmt, 1, addr);
            let ret = sqlite3_step(ps.stmt);
            if ret != SQLITE_ROW {
                return None;
            }

            let ptr = sqlite3_column_blob(ps.stmt, 0) as *const u8;
            let len = sqlite3_column_bytes(ps.stmt, 0) as usize;
            Some(std::slice::from_raw_parts(ptr, len).to_vec())
        }
    }
}

impl Drop for UnifiedSQLiteStorage {
    fn drop(&mut self) {
        connection_pool::release_connection(&self.db_name);
    }
}

/// Close all PSS-held connection references.
pub fn close_all() {
    // UnifiedSQLiteStorage uses AbsurderSQL's connection pool,
    // so closing is handled by releasing references.
    // This is a no-op; connections close when all references are released.
}

impl IStorage for UnifiedSQLiteStorage {
    fn store(&mut self, node: &Node) -> i64 {
        let level = node.level() as i64;
        let keys_blob = datom_serde::serialize_keys(node.keys());

        let addrs_blob: Option<Vec<u8>> = match node {
            Node::Branch(b) => {
                let addrs: Vec<i64> = (0..b.len())
                    .map(|i| {
                        b.address(i)
                            .expect("branch child must be stored before parent")
                    })
                    .collect();
                Some(datom_serde::serialize_addrs(&addrs))
            }
            Node::Leaf(_) => None,
        };

        let db = self.conn.db.get();
        let addr = unsafe {
            let ps = PreparedStmt::new(
                db,
                "INSERT INTO pss_nodes (level, keys, addrs) VALUES (?1, ?2, ?3)",
            );
            sqlite3_bind_int64(ps.stmt, 1, level);
            sqlite3_bind_blob(
                ps.stmt,
                2,
                keys_blob.as_ptr() as *const _,
                keys_blob.len() as i32,
                SQLITE_TRANSIENT(),
            );
            match &addrs_blob {
                Some(blob) => {
                    sqlite3_bind_blob(
                        ps.stmt,
                        3,
                        blob.as_ptr() as *const _,
                        blob.len() as i32,
                        SQLITE_TRANSIENT(),
                    );
                }
                None => {
                    sqlite3_bind_null(ps.stmt, 3);
                }
            }
            let ret = sqlite3_step(ps.stmt);
            assert_eq!(ret, SQLITE_DONE, "failed to store node: {}", ret);
            sqlite3_last_insert_rowid(db)
        };

        self.cache.put(addr, Rc::new(node.clone()));
        addr
    }

    fn restore(&self, address: i64) -> Rc<Node> {
        let db = self.conn.db.get();

        let (level, keys, addresses) = unsafe {
            let ps = PreparedStmt::new(
                db,
                "SELECT level, keys, addrs FROM pss_nodes WHERE addr = ?1",
            );
            sqlite3_bind_int64(ps.stmt, 1, address);

            let ret = sqlite3_step(ps.stmt);
            if ret != SQLITE_ROW {
                // Address not found — return an empty leaf node rather than panicking.
                // This can happen during legacy restore when pss_nodes is incomplete.
                web_sys::console::warn_1(
                    &format!("UnifiedSQLiteStorage: address {} not found (ret={}), returning empty leaf", address, ret).into(),
                );
                return Rc::new(Node::restore(0, vec![], None, &self.settings));
            }

            let level = sqlite3_column_int64(ps.stmt, 0) as u32;

            let keys_ptr = sqlite3_column_blob(ps.stmt, 1) as *const u8;
            let keys_len = sqlite3_column_bytes(ps.stmt, 1) as usize;
            let keys_blob = std::slice::from_raw_parts(keys_ptr, keys_len).to_vec();
            let keys: Vec<Key> = datom_serde::deserialize_keys(&keys_blob);

            let addresses: Option<Vec<i64>> = if sqlite3_column_type(ps.stmt, 2) == SQLITE_NULL {
                None
            } else {
                let addrs_ptr = sqlite3_column_blob(ps.stmt, 2) as *const u8;
                let addrs_len = sqlite3_column_bytes(ps.stmt, 2) as usize;
                let addrs_blob = std::slice::from_raw_parts(addrs_ptr, addrs_len).to_vec();
                Some(datom_serde::deserialize_addrs(&addrs_blob))
            };

            (level, keys, addresses)
        };

        let node = Node::restore(level, keys, addresses, &self.settings);
        Rc::new(node)
    }

    fn accessed(&self, _address: i64) {
        // StorageCell handles LRU for us
    }

    fn list_addresses(&self) -> Vec<i64> {
        let db = self.conn.db.get();
        let mut result = Vec::new();
        unsafe {
            let ps = PreparedStmt::new(db, "SELECT addr FROM pss_nodes");
            loop {
                let ret = sqlite3_step(ps.stmt);
                if ret == SQLITE_ROW {
                    result.push(sqlite3_column_int64(ps.stmt, 0));
                } else {
                    break;
                }
            }
        }
        result
    }

    fn delete(&mut self, addresses: &[i64]) {
        if addresses.is_empty() {
            return;
        }
        let db = self.conn.db.get();
        for chunk in addresses.chunks(100) {
            let placeholders: String = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!("DELETE FROM pss_nodes WHERE addr IN ({})", placeholders);

            unsafe {
                let ps = PreparedStmt::new(db, &sql);
                for (i, &addr) in chunk.iter().enumerate() {
                    sqlite3_bind_int64(ps.stmt, (i + 1) as i32, addr);
                }
                let ret = sqlite3_step(ps.stmt);
                assert!(ret == SQLITE_DONE || ret == SQLITE_OK, "delete failed: {}", ret);
            }
        }

        for addr in addresses {
            self.cache.pop(addr);
        }
    }
}
