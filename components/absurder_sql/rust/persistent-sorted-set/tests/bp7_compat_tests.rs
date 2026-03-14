//! Tests that verify backward compatibility with the production `.bp7` format.
//!
//! The `.bp7` file is a SQLite database with a `datascript` table:
//!   addr INTEGER PRIMARY KEY, content TEXT
//! Content is EDN-encoded. Metadata at addr 0 has:
//!   {:schema {...} :eavt <root-addr> :aevt <root-addr> :avet <root-addr>
//!    :max-eid N :max-tx N :max-addr N :branching-factor N}
//! PSS leaf nodes:  {:level 0, :keys [[e :attr val tx] ...]}
//! PSS branch nodes: {:level N, :keys [...], :addresses [addr1 ...]}
//! Tail at addr 1: []

use std::cmp::Ordering;
use std::collections::HashMap;
use std::rc::Rc;
use rusqlite::Connection;
use edn::parser::Parser;
use edn::Value as EdnValue;

use persistent_sorted_set::{Key, PersistentSortedSet, SQLiteStorage};
use persistent_sorted_set::settings::Settings;

/// Path to the production bp7 file (relative to the crate root).
const BP7_PATH: &str = "../../../../projects/behave/resources/public/behave-test.bp7";

fn bp7_conn() -> Connection {
    Connection::open_with_flags(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(BP7_PATH),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .expect("failed to open behave-test.bp7")
}

fn parse_edn(s: &str) -> EdnValue {
    let mut parser = Parser::new(s);
    parser.read().expect("failed to parse EDN").expect("empty EDN")
}

/// Check if an EDN Keyword string matches a given name (ignoring namespace).
/// EDN keywords are stored as strings like ":name" or ":ns/name".
fn kw_matches(kw: &str, name: &str) -> bool {
    // Strip leading ":"
    let stripped = kw.strip_prefix(':').unwrap_or(kw);
    // Match just the local name (after last /)
    if let Some((_ns, local)) = stripped.rsplit_once('/') {
        local == name
    } else {
        stripped == name
    }
}

/// Extract an integer from an EDN map by keyword key.
fn edn_get_int(map: &EdnValue, key: &str) -> i64 {
    if let EdnValue::Map(ref m) = map {
        for (k, v) in m.iter() {
            if let EdnValue::Keyword(ref kw) = k {
                if kw_matches(kw, key) {
                    return match v {
                        EdnValue::Integer(n) => *n,
                        _ => panic!("expected integer for key {}, got {:?}", key, v),
                    };
                }
            }
        }
    }
    panic!("key {} not found in EDN map", key);
}

/// Read all rows from the datascript table into a HashMap.
fn read_all_rows(conn: &Connection) -> HashMap<i64, String> {
    let mut stmt = conn
        .prepare("SELECT addr, content FROM datascript")
        .expect("datascript table must exist");
    let rows = stmt
        .query_map([], |row| {
            let addr: i64 = row.get(0)?;
            let content: String = row.get(1)?;
            Ok((addr, content))
        })
        .expect("query failed");
    rows.filter_map(|r| r.ok()).collect()
}

/// Recursively collect all datom vectors from a PSS tree stored in the datascript table.
fn collect_datoms(rows: &HashMap<i64, String>, addr: i64) -> Vec<Vec<EdnValue>> {
    let content = rows
        .get(&addr)
        .unwrap_or_else(|| panic!("addr {} not found in datascript table", addr));
    let edn = parse_edn(content);

    if let EdnValue::Map(ref m) = edn {
        let level = edn_get_int(&edn, "level");

        // Extract keys
        let keys_val = m.iter()
            .find(|(k, _)| matches!(k, EdnValue::Keyword(kw) if kw_matches(kw, "keys")))
            .map(|(_, v)| v)
            .expect("node must have :keys");

        let keys: Vec<Vec<EdnValue>> = if let EdnValue::Vector(ref arr) = keys_val {
            arr.iter()
                .map(|datom| {
                    if let EdnValue::Vector(ref fields) = datom {
                        fields.clone()
                    } else {
                        panic!("expected datom vector, got {:?}", datom);
                    }
                })
                .collect()
        } else {
            panic!("expected :keys to be a vector");
        };

        if level == 0 {
            keys
        } else {
            // Branch node — recurse into children
            let addrs_val = m.iter()
                .find(|(k, _)| matches!(k, EdnValue::Keyword(kw) if kw_matches(kw, "addresses")))
                .map(|(_, v)| v)
                .expect("branch node must have :addresses");

            let child_addrs: Vec<i64> = if let EdnValue::Vector(ref arr) = addrs_val {
                arr.iter()
                    .map(|v| match v {
                        EdnValue::Integer(n) => *n,
                        _ => panic!("expected integer address"),
                    })
                    .collect()
            } else {
                panic!("expected :addresses to be a vector");
            };

            let mut all_datoms = Vec::new();
            for child_addr in &child_addrs {
                all_datoms.extend(collect_datoms(rows, *child_addr));
            }
            all_datoms
        }
    } else {
        panic!("expected EDN map for node at addr {}, got {:?}", addr, edn);
    }
}

#[test]
fn test_bp7_opens_and_has_datascript_table() {
    let conn = bp7_conn();
    let count: i64 = conn
        .query_row("SELECT count(*) FROM datascript", [], |row| row.get(0))
        .expect("failed to count datascript rows");
    assert!(count > 0, "datascript table should have rows");
    println!("bp7 has {} rows in datascript table", count);
}

#[test]
fn test_bp7_metadata_is_valid() {
    let conn = bp7_conn();
    let content: String = conn
        .query_row(
            "SELECT content FROM datascript WHERE addr = 0",
            [],
            |row| row.get(0),
        )
        .expect("metadata at addr 0 must exist");

    let meta = parse_edn(&content);
    let eavt = edn_get_int(&meta, "eavt");
    let aevt = edn_get_int(&meta, "aevt");
    let avet = edn_get_int(&meta, "avet");
    let max_eid = edn_get_int(&meta, "max-eid");
    let max_tx = edn_get_int(&meta, "max-tx");
    let bf = edn_get_int(&meta, "branching-factor");

    println!("eavt root={}, aevt root={}, avet root={}", eavt, aevt, avet);
    println!("max-eid={}, max-tx={}, branching-factor={}", max_eid, max_tx, bf);

    assert!(eavt > 0);
    assert!(aevt > 0);
    assert!(avet > 0);
    assert!(max_eid > 0);
    assert!(max_tx > 0);
    assert_eq!(bf, 512);
}

#[test]
fn test_bp7_read_eavt_datoms() {
    let conn = bp7_conn();
    let rows = read_all_rows(&conn);

    let meta = parse_edn(rows.get(&0).expect("addr 0 must exist"));
    let eavt_root = edn_get_int(&meta, "eavt");

    let datoms = collect_datoms(&rows, eavt_root);
    println!("EAVT index has {} datoms", datoms.len());
    assert!(!datoms.is_empty(), "EAVT should have datoms");

    // Each datom should be [e a v tx] — 4 elements
    for (i, datom) in datoms.iter().enumerate().take(5) {
        assert_eq!(
            datom.len(),
            4,
            "datom {} should have 4 fields, got {}",
            i,
            datom.len()
        );
        assert!(
            matches!(datom[0], EdnValue::Integer(_)),
            "datom[0] (entity) should be integer"
        );
        assert!(
            matches!(datom[1], EdnValue::Keyword(_)),
            "datom[1] (attribute) should be keyword"
        );
        assert!(
            matches!(datom[3], EdnValue::Integer(_)),
            "datom[3] (tx) should be integer"
        );
        println!("  datom[{}]: {:?}", i, datom);
    }
}

#[test]
fn test_bp7_read_all_three_indexes() {
    let conn = bp7_conn();
    let rows = read_all_rows(&conn);
    let meta = parse_edn(rows.get(&0).expect("addr 0 must exist"));

    let eavt_root = edn_get_int(&meta, "eavt");
    let aevt_root = edn_get_int(&meta, "aevt");
    let avet_root = edn_get_int(&meta, "avet");

    let eavt_datoms = collect_datoms(&rows, eavt_root);
    let aevt_datoms = collect_datoms(&rows, aevt_root);
    let avet_datoms = collect_datoms(&rows, avet_root);

    println!(
        "Datom counts — EAVT: {}, AEVT: {}, AVET: {}",
        eavt_datoms.len(),
        aevt_datoms.len(),
        avet_datoms.len()
    );

    // EAVT and AEVT should have the same number of datoms
    assert_eq!(
        eavt_datoms.len(),
        aevt_datoms.len(),
        "EAVT and AEVT should have the same number of datoms"
    );

    // AVET may have fewer (only indexed attributes)
    assert!(
        avet_datoms.len() <= eavt_datoms.len(),
        "AVET should have <= EAVT datoms"
    );
}

#[test]
fn test_bp7_datom_value_types() {
    let conn = bp7_conn();
    let rows = read_all_rows(&conn);
    let meta = parse_edn(rows.get(&0).expect("addr 0 must exist"));
    let eavt_root = edn_get_int(&meta, "eavt");
    let datoms = collect_datoms(&rows, eavt_root);

    let mut has_integer = false;
    let mut has_string = false;
    let mut has_keyword = false;
    let mut has_bool = false;
    let mut has_float = false;

    for datom in &datoms {
        match &datom[2] {
            EdnValue::Integer(_) => has_integer = true,
            EdnValue::String(_) => has_string = true,
            EdnValue::Keyword(_) => has_keyword = true,
            EdnValue::Boolean(_) => has_bool = true,
            EdnValue::Float(_) => has_float = true,
            _ => {}
        }
    }

    println!(
        "Value types found — int: {}, str: {}, kw: {}, bool: {}, float: {}",
        has_integer, has_string, has_keyword, has_bool, has_float
    );

    assert!(has_integer, "should have integer values");
    assert!(has_keyword, "should have keyword values");
}

// ---------------------------------------------------------------------------
// Transaction tests: open a copy of the bp7, add new PSS data alongside it
// ---------------------------------------------------------------------------

fn make_cmp() -> Rc<dyn Fn(&Key, &Key) -> Ordering> {
    Rc::new(|a: &Key, b: &Key| a.cmp(b))
}

/// Copy the bp7 file to a temp path so we can write to it.
fn bp7_writable_copy() -> (Connection, tempfile::NamedTempFile) {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(BP7_PATH);
    let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
    std::fs::copy(&src, tmp.path()).expect("failed to copy bp7");
    let conn = Connection::open(tmp.path()).expect("failed to open writable copy");
    (conn, tmp)
}

#[test]
fn test_transact_new_pss_into_bp7_copy() {
    let (conn, _tmp) = bp7_writable_copy();

    // Original datascript table should still be intact
    let old_count: i64 = conn
        .query_row("SELECT count(*) FROM datascript", [], |row| row.get(0))
        .unwrap();
    assert_eq!(old_count, 52);

    // Create a new PSS with SQLiteStorage in the same database.
    // This creates a pss_nodes table alongside the existing datascript table.
    let settings = Settings::new(16);
    let storage = SQLiteStorage::new(conn, settings.clone());
    let mut set = PersistentSortedSet::with_storage(make_cmp(), Box::new(storage), settings);

    // "Transact" 100 keys
    for i in 1..=100 {
        set = set.conj(&i);
    }
    assert_eq!(set.count(), 100);

    let root_addr = set.store();
    assert!(root_addr > 0);
    println!("Stored 100 keys, root addr = {}", root_addr);

    // Verify all keys survived the store
    let all = set.to_vec();
    assert_eq!(all, (1..=100).collect::<Vec<Key>>());
}

#[test]
fn test_transact_store_restore_roundtrip() {
    let (conn, tmp) = bp7_writable_copy();

    let settings = Settings::new(16);
    let storage = SQLiteStorage::new(conn, settings.clone());
    let mut set = PersistentSortedSet::with_storage(make_cmp(), Box::new(storage), settings.clone());

    for i in 1..=500 {
        set = set.conj(&i);
    }
    let root_addr = set.store();
    println!("First store: root_addr={}, count={}", root_addr, set.count());

    // Drop the set and reopen from the same SQLite file
    drop(set);
    let conn2 = Connection::open(tmp.path()).unwrap();
    let storage2 = SQLiteStorage::new(conn2, settings.clone());
    let restored = PersistentSortedSet::restore(make_cmp(), root_addr, Box::new(storage2), settings);

    assert_eq!(restored.count(), 500);
    assert!(restored.contains(&1));
    assert!(restored.contains(&250));
    assert!(restored.contains(&500));
    assert!(!restored.contains(&501));

    let all = restored.to_vec();
    assert_eq!(all, (1..=500).collect::<Vec<Key>>());
    println!("Restored 500 keys successfully");
}

#[test]
fn test_transact_multiple_rounds() {
    let (conn, tmp) = bp7_writable_copy();

    let settings = Settings::new(16);
    let storage = SQLiteStorage::new(conn, settings.clone());
    let mut set = PersistentSortedSet::with_storage(make_cmp(), Box::new(storage), settings.clone());

    // Round 1: insert 1..=100
    for i in 1..=100 {
        set = set.conj(&i);
    }
    let addr1 = set.store();
    println!("Round 1: root={}, count={}", addr1, set.count());

    // Round 2: insert 101..=200 (conj after store)
    for i in 101..=200 {
        set = set.conj(&i);
    }
    let addr2 = set.store();
    println!("Round 2: root={}, count={}", addr2, set.count());
    assert_ne!(addr1, addr2, "new root should have different address");

    // Round 3: remove odds, add 201..=250
    for i in (1..=200).step_by(2) {
        set = set.disj(&i);
    }
    for i in 201..=250 {
        set = set.conj(&i);
    }
    let addr3 = set.store();
    println!("Round 3: root={}, count={}", addr3, set.count());

    // Verify by restoring from scratch
    drop(set);
    let conn2 = Connection::open(tmp.path()).unwrap();
    let storage2 = SQLiteStorage::new(conn2, settings.clone());
    let restored = PersistentSortedSet::restore(make_cmp(), addr3, Box::new(storage2), settings);

    // Should have evens 2..=200 plus 201..=250
    let expected: Vec<Key> = (2..=200)
        .step_by(2)
        .chain(201..=250)
        .collect();
    let actual = restored.to_vec();
    assert_eq!(actual, expected);
    println!("Verified {} keys after 3 transaction rounds", actual.len());
}

#[test]
fn test_old_datascript_table_untouched_after_transactions() {
    let (conn, tmp) = bp7_writable_copy();

    // Read original datascript content before any PSS operations
    let original_rows = read_all_rows(&conn);

    let settings = Settings::new(16);
    let storage = SQLiteStorage::new(conn, settings.clone());
    let mut set = PersistentSortedSet::with_storage(make_cmp(), Box::new(storage), settings);

    for i in 1..=1000 {
        set = set.conj(&i);
    }
    set.store();
    drop(set);

    // Reopen and verify datascript table is unchanged
    let conn2 = Connection::open(tmp.path()).unwrap();
    let after_rows = read_all_rows(&conn2);

    assert_eq!(
        original_rows.len(),
        after_rows.len(),
        "datascript table row count should be unchanged"
    );
    for (addr, content) in &original_rows {
        assert_eq!(
            after_rows.get(addr).unwrap(),
            content,
            "datascript row at addr {} should be unchanged",
            addr
        );
    }

    // Verify both tables coexist
    let has_pss_nodes: bool = conn2
        .query_row(
            "SELECT count(*) > 0 FROM sqlite_master WHERE type='table' AND name='pss_nodes'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(has_pss_nodes, "pss_nodes table should exist after transactions");

    let pss_count: i64 = conn2
        .query_row("SELECT count(*) FROM pss_nodes", [], |row| row.get(0))
        .unwrap();
    assert!(pss_count > 0, "pss_nodes should have stored nodes");
    println!(
        "Old datascript table ({} rows) untouched, new pss_nodes has {} rows",
        original_rows.len(),
        pss_count
    );
}

#[test]
fn test_bp7_no_pss_nodes_table() {
    let conn = bp7_conn();
    let has_pss_nodes: bool = conn
        .query_row(
            "SELECT count(*) > 0 FROM sqlite_master WHERE type='table' AND name='pss_nodes'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(
        !has_pss_nodes,
        "production bp7 should NOT have pss_nodes table — it uses the old EDN format"
    );
}

// ---------------------------------------------------------------------------
// Tests using the legacy_edn module (parsing bp7 data into Datom types)
// ---------------------------------------------------------------------------

use persistent_sorted_set::legacy_edn::{
    metadata_from_edn, node_from_edn, datom_to_edn,
    parse_edn as le_parse_edn,
};
use persistent_sorted_set::datom::{Attr, Value};

#[test]
fn test_bp7_parse_metadata_via_legacy_edn() {
    let conn = bp7_conn();
    let content: String = conn
        .query_row("SELECT content FROM datascript WHERE addr = 0", [], |row| row.get(0))
        .unwrap();

    let edn = le_parse_edn(&content);
    let meta = metadata_from_edn(&edn);

    assert_eq!(meta.eavt_root, 1081805);
    assert_eq!(meta.branching_factor, 512);
    assert!(meta.max_eid > 0);
    assert!(meta.max_tx > 0);

    // Schema should have :db/ident with unique identity
    let db_ident = Attr::Keyword { ns: Some("db".into()), name: "ident".into() };
    assert!(meta.rschema.unique_identity.contains(&db_ident));
}

#[test]
fn test_bp7_parse_leaf_node_via_legacy_edn() {
    let conn = bp7_conn();
    let rows = read_all_rows(&conn);

    let meta_edn = le_parse_edn(rows.get(&0).unwrap());
    let meta = metadata_from_edn(&meta_edn);

    // Find a leaf node (level 0) by scanning from the EAVT root
    fn find_leaf(rows: &HashMap<i64, String>, addr: i64) -> i64 {
        let content = rows.get(&addr).unwrap();
        let edn = le_parse_edn(content);
        let level = edn_get_int(&edn, "level");
        if level == 0 {
            addr
        } else {
            // Follow first child
            if let Some(addrs_val) = rows.get(&addr).map(|c| le_parse_edn(c)) {
                if let EdnValue::Map(ref m) = addrs_val {
                    for (k, v) in m.iter() {
                        if let EdnValue::Keyword(ref kw) = k {
                            if kw_matches(kw, "addresses") {
                                if let EdnValue::Vector(ref arr) = v {
                                    if let Some(EdnValue::Integer(child_addr)) = arr.first() {
                                        return find_leaf(rows, *child_addr);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            panic!("could not find leaf");
        }
    }

    let leaf_addr = find_leaf(&rows, meta.eavt_root);
    let leaf_content = rows.get(&leaf_addr).unwrap();
    let leaf_edn = le_parse_edn(leaf_content);

    let (level, keys, addrs) = node_from_edn(&leaf_edn, &meta.rschema);
    assert_eq!(level, 0);
    assert!(!keys.is_empty(), "leaf should have datoms");
    assert!(addrs.is_none(), "leaf should have no addresses");

    // Each datom should have valid fields
    for d in &keys {
        assert!(d.e > 0, "entity should be positive");
        assert!(d.a.is_some(), "attr should be Some");
        assert!(d.tx > 0, "tx should be positive");
    }

    // Verify round-trip: datom_to_edn should produce parseable EDN
    let first_edn = datom_to_edn(&keys[0]);
    assert!(first_edn.starts_with('['), "should be a vector");
    assert!(first_edn.ends_with(']'), "should end with ]");
}

#[test]
fn test_bp7_parse_all_datoms_via_legacy_edn() {
    let conn = bp7_conn();
    let rows = read_all_rows(&conn);
    let meta_edn = le_parse_edn(rows.get(&0).unwrap());
    let meta = metadata_from_edn(&meta_edn);

    // Recursively collect all datoms using our Datom type
    fn collect_typed_datoms(
        rows: &HashMap<i64, String>,
        addr: i64,
        rschema: &persistent_sorted_set::schema::ReverseSchema,
    ) -> Vec<persistent_sorted_set::datom::Datom> {
        let content = rows.get(&addr).unwrap();
        let edn = le_parse_edn(content);
        let (level, keys, addresses) = node_from_edn(&edn, rschema);

        if level == 0 {
            keys
        } else {
            let mut result = Vec::new();
            for child_addr in addresses.unwrap() {
                result.extend(collect_typed_datoms(rows, child_addr, rschema));
            }
            result
        }
    }

    let eavt_datoms = collect_typed_datoms(&rows, meta.eavt_root, &meta.rschema);
    let aevt_datoms = collect_typed_datoms(&rows, meta.aevt_root, &meta.rschema);
    let avet_datoms = collect_typed_datoms(&rows, meta.avet_root, &meta.rschema);

    assert_eq!(eavt_datoms.len(), aevt_datoms.len());
    assert!(avet_datoms.len() <= eavt_datoms.len());
    assert!(!eavt_datoms.is_empty());

    println!(
        "Parsed via legacy_edn — EAVT: {}, AEVT: {}, AVET: {}",
        eavt_datoms.len(), aevt_datoms.len(), avet_datoms.len()
    );

    // Verify diverse value types
    let mut has_long = false;
    let mut has_str = false;
    let mut has_kw = false;
    let mut has_bool = false;
    for d in &eavt_datoms {
        match &d.v {
            Value::Long(_) | Value::Ref(_) => has_long = true,
            Value::Str(_) => has_str = true,
            Value::Keyword(_) => has_kw = true,
            Value::Bool(_) => has_bool = true,
            _ => {}
        }
    }
    assert!(has_long, "should have integer/ref values");
    assert!(has_str, "should have string values");
    assert!(has_kw, "should have keyword values");
}
