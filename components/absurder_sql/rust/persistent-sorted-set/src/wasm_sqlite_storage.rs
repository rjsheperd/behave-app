//! WASM SQLiteStorage — direct SQLite access via sqlite-wasm-rs FFI.
//!
//! Eliminates the JsStorage bridge for B+ tree node persistence.
//! Uses raw sqlite3_* C FFI functions (sqlite-wasm-rs is NOT rusqlite-compatible).
//! Only compiled for wasm32 targets.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::CString;
use std::num::NonZeroUsize;
use std::ptr;
use std::rc::Rc;

use lru::LruCache;

use sqlite_wasm_rs::{
    sqlite3, sqlite3_stmt,
    sqlite3_open_v2, sqlite3_close,
    sqlite3_prepare_v2, sqlite3_step, sqlite3_finalize,
    sqlite3_bind_int64, sqlite3_bind_blob, sqlite3_bind_null,
    sqlite3_column_int64, sqlite3_column_blob, sqlite3_column_bytes, sqlite3_column_type,
    sqlite3_exec, sqlite3_errmsg,
    SQLITE_OK, SQLITE_ROW, SQLITE_DONE,
    SQLITE_OPEN_READWRITE, SQLITE_OPEN_CREATE,
    SQLITE_NULL, SQLITE_TRANSIENT,
};

use crate::datom_serde;
use crate::key::Key;
use crate::node::Node;
use crate::settings::Settings;
use crate::storage::IStorage;

// ---------------------------------------------------------------------------
// Connection pool
// ---------------------------------------------------------------------------

thread_local! {
    static CONNECTIONS: RefCell<HashMap<String, *mut sqlite3>> = RefCell::new(HashMap::new());
}

/// Open or reuse a SQLite connection for the given db path.
fn open_or_get(db_path: &str) -> *mut sqlite3 {
    CONNECTIONS.with(|conns| {
        let mut map = conns.borrow_mut();
        if let Some(&db) = map.get(db_path) {
            return db;
        }
        let c_path = CString::new(db_path).expect("db_path contains null byte");
        let mut db: *mut sqlite3 = ptr::null_mut();
        let ret = unsafe {
            sqlite3_open_v2(
                c_path.as_ptr(),
                &mut db,
                SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE,
                ptr::null(),
            )
        };
        assert_eq!(ret, SQLITE_OK, "failed to open SQLite db: {}", db_path);
        map.insert(db_path.to_string(), db);
        db
    })
}

/// Close all pooled connections. Call during teardown.
pub fn close_all() {
    CONNECTIONS.with(|conns| {
        let mut map = conns.borrow_mut();
        for (_, db) in map.drain() {
            unsafe { sqlite3_close(db); }
        }
    });
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

unsafe fn exec_sql(db: *mut sqlite3, sql: &str) {
    let c_sql = CString::new(sql).unwrap();
    let ret = sqlite3_exec(db, c_sql.as_ptr(), None, ptr::null_mut(), ptr::null_mut());
    if ret != SQLITE_OK {
        let msg = std::ffi::CStr::from_ptr(sqlite3_errmsg(db))
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
        let ret = sqlite3_prepare_v2(
            db,
            c_sql.as_ptr(),
            sql.len() as i32,
            &mut stmt,
            ptr::null_mut(),
        );
        if ret != SQLITE_OK {
            let msg = std::ffi::CStr::from_ptr(sqlite3_errmsg(db))
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
// WasmSQLiteStorage
// ---------------------------------------------------------------------------

pub struct WasmSQLiteStorage {
    db: *mut sqlite3,
    cache: LruCache<i64, Rc<Node>>,
    next_addr: i64,
    settings: Settings,
}

impl WasmSQLiteStorage {
    pub fn new(db_path: &str, settings: Settings) -> Self {
        let db = open_or_get(db_path);

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

        let next_addr = unsafe {
            let ps = PreparedStmt::new(db, "SELECT COALESCE(MAX(addr), 0) + 1 FROM pss_nodes");
            let ret = sqlite3_step(ps.stmt);
            if ret == SQLITE_ROW {
                sqlite3_column_int64(ps.stmt, 0)
            } else {
                1
            }
        };

        let cache_size = settings.cache_size();
        WasmSQLiteStorage {
            db,
            cache: LruCache::new(NonZeroUsize::new(cache_size).unwrap_or(NonZeroUsize::new(1024).unwrap())),
            next_addr,
            settings,
        }
    }
}

impl IStorage for WasmSQLiteStorage {
    fn store(&mut self, node: &Node) -> i64 {
        let addr = self.next_addr;
        self.next_addr += 1;

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

        unsafe {
            let ps = PreparedStmt::new(
                self.db,
                "INSERT INTO pss_nodes (addr, level, keys, addrs) VALUES (?1, ?2, ?3, ?4)",
            );
            sqlite3_bind_int64(ps.stmt, 1, addr);
            sqlite3_bind_int64(ps.stmt, 2, level);
            sqlite3_bind_blob(
                ps.stmt,
                3,
                keys_blob.as_ptr() as *const _,
                keys_blob.len() as i32,
                SQLITE_TRANSIENT(),
            );
            match &addrs_blob {
                Some(blob) => {
                    sqlite3_bind_blob(
                        ps.stmt,
                        4,
                        blob.as_ptr() as *const _,
                        blob.len() as i32,
                        SQLITE_TRANSIENT(),
                    );
                }
                None => {
                    sqlite3_bind_null(ps.stmt, 4);
                }
            }
            let ret = sqlite3_step(ps.stmt);
            assert_eq!(ret, SQLITE_DONE, "failed to store node at addr {}", addr);
        }

        self.cache.put(addr, Rc::new(node.clone()));
        addr
    }

    fn restore(&self, address: i64) -> Rc<Node> {
        // Check cache first (need a const_cast because LruCache::get takes &mut)
        // We can't use the cache here without interior mutability, but the caller
        // (StorageCell) wraps us with its own LRU cache, so this is acceptable.

        let (level, keys, addresses) = unsafe {
            let ps = PreparedStmt::new(
                self.db,
                "SELECT level, keys, addrs FROM pss_nodes WHERE addr = ?1",
            );
            sqlite3_bind_int64(ps.stmt, 1, address);

            let ret = sqlite3_step(ps.stmt);
            assert_eq!(ret, SQLITE_ROW, "address {} not found in WasmSQLiteStorage", address);

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
        let mut result = Vec::new();
        unsafe {
            let ps = PreparedStmt::new(self.db, "SELECT addr FROM pss_nodes");
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
        // Delete in batches to avoid overly long SQL
        for chunk in addresses.chunks(100) {
            let placeholders: String = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!("DELETE FROM pss_nodes WHERE addr IN ({})", placeholders);

            unsafe {
                let ps = PreparedStmt::new(self.db, &sql);
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
