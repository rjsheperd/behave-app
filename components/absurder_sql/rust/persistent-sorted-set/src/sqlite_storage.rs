#[cfg(not(target_arch = "wasm32"))]
use std::num::NonZeroUsize;
#[cfg(not(target_arch = "wasm32"))]
use std::rc::Rc;

#[cfg(not(target_arch = "wasm32"))]
use lru::LruCache;

#[cfg(not(target_arch = "wasm32"))]
use crate::key::Key;
#[cfg(not(target_arch = "wasm32"))]
use crate::node::Node;
#[cfg(not(target_arch = "wasm32"))]
use crate::settings::Settings;
#[cfg(not(target_arch = "wasm32"))]
use crate::storage::IStorage;

#[cfg(not(target_arch = "wasm32"))]
use rusqlite::{params, Connection};

/// SQLite-backed storage for the persistent sorted set.
/// Uses an LRU cache to avoid repeated deserialization.
#[cfg(not(target_arch = "wasm32"))]
pub struct SQLiteStorage {
    conn: Connection,
    cache: LruCache<i64, Rc<Node>>,
    next_addr: i64,
    settings: Settings,
}

#[cfg(not(target_arch = "wasm32"))]
impl SQLiteStorage {
    pub fn new(conn: Connection, settings: Settings) -> Self {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS pss_nodes (
                addr INTEGER PRIMARY KEY,
                level INTEGER NOT NULL,
                keys BLOB NOT NULL,
                addrs BLOB
            )",
        )
        .expect("failed to create pss_nodes table");

        let next_addr: i64 = conn
            .query_row("SELECT COALESCE(MAX(addr), 0) + 1 FROM pss_nodes", [], |row| {
                row.get(0)
            })
            .unwrap_or(1);

        Self {
            conn,
            cache: LruCache::new(NonZeroUsize::new(1024).unwrap()),
            next_addr,
            settings,
        }
    }

    /// Serialize keys to a compact binary blob: packed i64 little-endian.
    fn serialize_keys(keys: &[Key]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(keys.len() * 8);
        for k in keys {
            buf.extend_from_slice(&k.to_le_bytes());
        }
        buf
    }

    fn deserialize_keys(blob: &[u8]) -> Vec<Key> {
        blob.chunks_exact(8)
            .map(|chunk| i64::from_le_bytes(chunk.try_into().unwrap()))
            .collect()
    }

    fn serialize_addrs(addrs: &[i64]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(addrs.len() * 8);
        for &a in addrs {
            buf.extend_from_slice(&a.to_le_bytes());
        }
        buf
    }

    fn deserialize_addrs(blob: &[u8]) -> Vec<i64> {
        blob.chunks_exact(8)
            .map(|chunk| i64::from_le_bytes(chunk.try_into().unwrap()))
            .collect()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl IStorage for SQLiteStorage {
    fn store(&mut self, node: &Node) -> i64 {
        let addr = self.next_addr;
        self.next_addr += 1;

        let level = node.level() as i64;
        let keys_blob = Self::serialize_keys(node.keys());

        let addrs_blob: Option<Vec<u8>> = match node {
            Node::Branch(b) => {
                let addrs: Vec<i64> = (0..b.len())
                    .map(|i| {
                        b.address(i)
                            .expect("branch child must be stored before parent")
                    })
                    .collect();
                Some(Self::serialize_addrs(&addrs))
            }
            Node::Leaf(_) => None,
        };

        self.conn
            .execute(
                "INSERT INTO pss_nodes (addr, level, keys, addrs) VALUES (?1, ?2, ?3, ?4)",
                params![addr, level, keys_blob, addrs_blob],
            )
            .expect("failed to store node");

        self.cache.put(addr, Rc::new(node.clone()));
        addr
    }

    fn restore(&self, address: i64) -> Rc<Node> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT level, keys, addrs FROM pss_nodes WHERE addr = ?1")
            .expect("failed to prepare restore query");

        let (level, keys_blob, addrs_blob): (i64, Vec<u8>, Option<Vec<u8>>) = stmt
            .query_row(params![address], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .expect("address not found in SQLiteStorage");

        let keys = Self::deserialize_keys(&keys_blob);
        let addresses = addrs_blob.map(|b| Self::deserialize_addrs(&b));

        let node = Node::restore(level as u32, keys, addresses, &self.settings);
        Rc::new(node)
    }

    fn accessed(&self, _address: i64) {
        // LRU touch would go here with RefCell-wrapped cache
    }

    fn list_addresses(&self) -> Vec<i64> {
        let mut stmt = self
            .conn
            .prepare("SELECT addr FROM pss_nodes")
            .expect("failed to prepare list query");

        stmt.query_map([], |row| row.get(0))
            .expect("failed to list addresses")
            .filter_map(|r| r.ok())
            .collect()
    }

    fn delete(&mut self, addresses: &[i64]) {
        if addresses.is_empty() {
            return;
        }
        let placeholders: Vec<String> = addresses.iter().map(|_| "?".to_string()).collect();
        let sql = format!(
            "DELETE FROM pss_nodes WHERE addr IN ({})",
            placeholders.join(",")
        );
        let params: Vec<Box<dyn rusqlite::types::ToSql>> = addresses
            .iter()
            .map(|a| Box::new(*a) as Box<dyn rusqlite::types::ToSql>)
            .collect();
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        self.conn
            .execute(&sql, param_refs.as_slice())
            .expect("failed to delete addresses");

        for addr in addresses {
            self.cache.pop(addr);
        }
    }
}
