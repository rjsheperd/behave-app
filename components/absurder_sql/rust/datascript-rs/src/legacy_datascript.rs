//! Legacy DataScript storage: reads/writes PSS nodes in the EDN-based
//! `datascript` table format used by production `.bp7` files.
//!
//! Pure EDN serde functions live in `persistent_sorted_set::legacy_edn`.
//! This module provides the wasm32-only `LegacyStorage` IStorage implementation.

// Re-export the pure EDN serde types for convenience.
pub use persistent_sorted_set::legacy_edn::LegacyMetadata;

// ---------------------------------------------------------------------------
// LegacyStorage (wasm32 only) — IStorage impl using EDN in `datascript` table
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
pub mod wasm {
    use std::ffi::CString;
    use std::num::NonZeroUsize;
    use std::ptr;
    use std::rc::Rc;

    use lru::LruCache;

    use sqlite_wasm_rs::{
        sqlite3, sqlite3_stmt,
        sqlite3_prepare_v2, sqlite3_step, sqlite3_finalize,
        sqlite3_bind_int64, sqlite3_bind_text,
        sqlite3_column_int64, sqlite3_column_text,
        sqlite3_errmsg,
        SQLITE_OK, SQLITE_ROW, SQLITE_DONE,
        SQLITE_TRANSIENT,
    };

    use absurder_sql::connection_pool::{self, ConnectionState};

    use persistent_sorted_set::legacy_edn::{
        LegacyMetadata, metadata_from_edn, metadata_to_edn, node_from_edn, node_to_edn,
        parse_edn,
    };
    use persistent_sorted_set::node::Node;
    use persistent_sorted_set::schema::{ReverseSchema, Schema, build_rschema};
    use persistent_sorted_set::settings::Settings;
    use persistent_sorted_set::storage::IStorage;

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
            unsafe {
                sqlite3_finalize(self.stmt);
            }
        }
    }

    /// Get the pooled AbsurderSQL connection for `db_name`, or error.
    ///
    /// Deliberately NO fallback open: a raw `sqlite3_open_v2` with a null VFS
    /// would silently write to an in-memory database (the Phase-6 "worksheets
    /// don't persist" bug). The connection must be opened first via
    /// `persistence::ensure_open` (`WasmDataScript.open`) or `sql/connect!`.
    pub fn require_connection(db_name: &str) -> Result<Rc<ConnectionState>, String> {
        let pool_key = db_name.trim_end_matches(".db");
        connection_pool::get_or_create_connection(pool_key, || {
            Err(format!(
                "No open AbsurderSQL connection for {} — call WasmDataScript.open() first",
                db_name
            ))
        })
    }

    /// LegacyStorage reads/writes PSS nodes as EDN text in the `datascript` table.
    pub struct LegacyStorage {
        conn: Rc<ConnectionState>,
        db_name: String,
        schema: Schema,
        rschema: ReverseSchema,
        settings: Settings,
        max_addr: i64,
        cache: LruCache<i64, Rc<Node>>,
    }

    impl LegacyStorage {
        pub fn new(db_name: &str, settings: Settings) -> Result<Self, String> {
            let conn = require_connection(db_name)?;

            let (schema, rschema, max_addr) =
                if let Some(meta) = Self::read_metadata_raw(conn.db.get()) {
                    let rs = build_rschema(&meta.schema);
                    (meta.schema, rs, meta.max_addr)
                } else {
                    // Fresh database — no metadata yet
                    let s = Schema::default();
                    let rs = build_rschema(&s);
                    (s, rs, 1) // addr 0 = metadata, addr 1 = tail (reserved)
                };

            let cache_size = settings.cache_size();
            Ok(LegacyStorage {
                conn,
                db_name: db_name.to_string(),
                schema,
                rschema,
                settings,
                max_addr,
                cache: LruCache::new(
                    NonZeroUsize::new(cache_size)
                        .unwrap_or(NonZeroUsize::new(1024).unwrap()),
                ),
            })
        }

        fn read_metadata_raw(db: *mut sqlite3) -> Option<LegacyMetadata> {
            let content = unsafe {
                // Check if datascript table exists first
                let ps = PreparedStmt::new(
                    db,
                    "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='datascript'",
                );
                let ret = sqlite3_step(ps.stmt);
                if ret != SQLITE_ROW || sqlite3_column_int64(ps.stmt, 0) == 0 {
                    return None;
                }

                let ps = PreparedStmt::new(
                    db,
                    "SELECT content FROM datascript WHERE addr = 0",
                );
                let ret = sqlite3_step(ps.stmt);
                if ret != SQLITE_ROW {
                    return None;
                }
                let ptr = sqlite3_column_text(ps.stmt, 0);
                std::ffi::CStr::from_ptr(ptr as *const i8)
                    .to_string_lossy()
                    .into_owned()
            };
            let edn = parse_edn(&content);
            Some(metadata_from_edn(&edn))
        }

        pub fn read_metadata(&self) -> Option<LegacyMetadata> {
            Self::read_metadata_raw(self.conn.db.get())
        }

        pub fn write_metadata(&mut self, meta: &LegacyMetadata) -> Result<(), String> {
            let content = metadata_to_edn(meta);
            let db = self.conn.db.get();
            unsafe {
                let ps = PreparedStmt::new(
                    db,
                    "INSERT OR REPLACE INTO datascript (addr, content) VALUES (0, ?1)",
                );
                let c_content = CString::new(content.as_str()).unwrap();
                sqlite3_bind_text(
                    ps.stmt,
                    1,
                    c_content.as_ptr(),
                    content.len() as i32,
                    SQLITE_TRANSIENT(),
                );
                let ret = sqlite3_step(ps.stmt);
                if ret != SQLITE_DONE {
                    let msg = std::ffi::CStr::from_ptr(sqlite3_errmsg(db))
                        .to_string_lossy()
                        .into_owned();
                    return Err(format!("failed to write metadata ({}): {}", ret, msg));
                }
            }
            self.max_addr = meta.max_addr;
            Ok(())
        }

        pub fn schema(&self) -> &Schema {
            &self.schema
        }

        pub fn rschema(&self) -> &ReverseSchema {
            &self.rschema
        }

        pub fn max_addr(&self) -> i64 {
            self.max_addr
        }

        /// Create a LegacyStorage with an explicit max_addr (used when storing
        /// multiple indexes sequentially to avoid address collisions).
        pub fn new_with_max_addr(
            db_name: &str,
            settings: Settings,
            max_addr: i64,
        ) -> Result<Self, String> {
            let mut s = Self::new(db_name, settings)?;
            s.max_addr = max_addr;
            Ok(s)
        }

        /// Query the actual maximum address from the datascript table.
        pub fn query_max_addr(db_name: &str) -> Result<i64, String> {
            let pool_key = db_name.trim_end_matches(".db");
            let conn = require_connection(db_name)?;
            let db = conn.db.get();
            let result = unsafe {
                let ps = PreparedStmt::new(db, "SELECT COALESCE(MAX(addr), 1) FROM datascript");
                let ret = sqlite3_step(ps.stmt);
                if ret == SQLITE_ROW {
                    sqlite3_column_int64(ps.stmt, 0)
                } else {
                    1
                }
            };
            connection_pool::release_connection(pool_key);
            Ok(result)
        }

        /// Check if the `datascript` table exists in the database.
        pub fn has_legacy_data(db_name: &str) -> bool {
            let pool_key = db_name.trim_end_matches(".db");
            if !connection_pool::connection_exists(pool_key) {
                return false;
            }
            let conn = match connection_pool::get_or_create_connection(pool_key, || {
                Err("no existing connection".to_string())
            }) {
                Ok(c) => c,
                Err(_) => return false,
            };
            let db = conn.db.get();
            let has = unsafe {
                let ps = PreparedStmt::new(
                    db,
                    "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='datascript'",
                );
                let ret = sqlite3_step(ps.stmt);
                if ret != SQLITE_ROW {
                    false
                } else {
                    sqlite3_column_int64(ps.stmt, 0) > 0
                }
            };
            connection_pool::release_connection(pool_key);
            has
        }
    }

    impl Drop for LegacyStorage {
        fn drop(&mut self) {
            connection_pool::release_connection(
                self.db_name.trim_end_matches(".db"),
            );
        }
    }

    impl IStorage for LegacyStorage {
        fn store(&mut self, node: &Node) -> i64 {
            self.max_addr += 1;
            let addr = self.max_addr;

            let level = node.level() as u32;
            let keys = node.keys();

            let addresses: Option<Vec<i64>> = match node {
                Node::Branch(b) => Some(
                    (0..b.len())
                        .map(|i| {
                            b.address(i)
                                .expect("branch child must be stored before parent")
                        })
                        .collect(),
                ),
                Node::Leaf(_) => None,
            };

            let content = node_to_edn(level, keys, addresses.as_deref());

            let db = self.conn.db.get();
            unsafe {
                let ps = PreparedStmt::new(
                    db,
                    "INSERT OR REPLACE INTO datascript (addr, content) VALUES (?1, ?2)",
                );
                sqlite3_bind_int64(ps.stmt, 1, addr);
                let c_content = CString::new(content.as_str()).unwrap();
                sqlite3_bind_text(
                    ps.stmt,
                    2,
                    c_content.as_ptr(),
                    content.len() as i32,
                    SQLITE_TRANSIENT(),
                );
                let ret = sqlite3_step(ps.stmt);
                assert_eq!(ret, SQLITE_DONE, "failed to store legacy node: {}", ret);
            }

            self.cache.put(addr, Rc::new(node.clone()));
            addr
        }

        fn restore(&self, address: i64) -> Rc<Node> {
            if let Some(node) = unsafe {
                // LruCache::get takes &mut self but we only have &self from IStorage.
                // Safe because WASM is single-threaded.
                let cache = &self.cache as *const LruCache<i64, Rc<Node>>
                    as *mut LruCache<i64, Rc<Node>>;
                (*cache).get(&address).cloned()
            } {
                return node;
            }

            let db = self.conn.db.get();
            let content = unsafe {
                let ps = PreparedStmt::new(
                    db,
                    "SELECT content FROM datascript WHERE addr = ?1",
                );
                sqlite3_bind_int64(ps.stmt, 1, address);
                let ret = sqlite3_step(ps.stmt);
                if ret != SQLITE_ROW {
                    web_sys::console::warn_1(
                        &format!(
                            "LegacyStorage: address {} not found (ret={}), returning empty leaf",
                            address, ret
                        )
                        .into(),
                    );
                    return Rc::new(Node::restore(0, vec![], None, &self.settings));
                }
                let ptr = sqlite3_column_text(ps.stmt, 0);
                std::ffi::CStr::from_ptr(ptr as *const i8)
                    .to_string_lossy()
                    .into_owned()
            };

            let edn = parse_edn(&content);
            let (level, keys, addresses) = node_from_edn(&edn, &self.rschema);

            let node = Node::restore(level, keys, addresses, &self.settings);
            let rc = Rc::new(node);

            unsafe {
                let cache = &self.cache as *const LruCache<i64, Rc<Node>>
                    as *mut LruCache<i64, Rc<Node>>;
                (*cache).put(address, rc.clone());
            }

            rc
        }

        fn accessed(&self, _address: i64) {}

        fn list_addresses(&self) -> Vec<i64> {
            let db = self.conn.db.get();
            let mut result = Vec::new();
            unsafe {
                let ps =
                    PreparedStmt::new(db, "SELECT addr FROM datascript WHERE addr >= 2");
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
                let placeholders: String =
                    chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                let sql = format!(
                    "DELETE FROM datascript WHERE addr IN ({})",
                    placeholders
                );
                unsafe {
                    let ps = PreparedStmt::new(db, &sql);
                    for (i, &addr) in chunk.iter().enumerate() {
                        sqlite3_bind_int64(ps.stmt, (i + 1) as i32, addr);
                    }
                    let ret = sqlite3_step(ps.stmt);
                    assert!(
                        ret == SQLITE_DONE || ret == SQLITE_OK,
                        "legacy delete failed: {}",
                        ret
                    );
                }
            }
            for addr in addresses {
                self.cache.pop(addr);
            }
        }
    }
}
