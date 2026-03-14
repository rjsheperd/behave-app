//! Tests that exercise the legacy EDN storage format end-to-end.
//!
//! Uses rusqlite to create a `datascript` table with EDN content,
//! then reads/writes through the `legacy_edn` functions to verify the
//! full pipeline that LegacyStorage (wasm-only) follows.
//!
//! Also tests round-trip: read real bp7 data → parse to Datom → write EDN
//! → re-parse → verify identical datoms.

use std::collections::HashMap;

use edn::Value as EdnValue;
use rusqlite::Connection;

use persistent_sorted_set::datom::{Attr, Datom, Value};
use persistent_sorted_set::legacy_edn::{
    datom_from_edn, datom_to_edn, edn_get_int,
    metadata_from_edn, metadata_to_edn, node_from_edn, node_to_edn, parse_edn, schema_from_edn,
    schema_to_edn, LegacyMetadata,
};
use persistent_sorted_set::schema::{
    kw, AttrSchema, ReverseSchema, Schema, ValueType, build_rschema,
};

/// Path to the production bp7 file.
const BP7_PATH: &str = "../../../../projects/behave/resources/public/behave-test.bp7";

fn bp7_conn() -> Connection {
    Connection::open_with_flags(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(BP7_PATH),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .expect("failed to open behave-test.bp7")
}

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
        .unwrap();
    rows.filter_map(|r| r.ok()).collect()
}

// ===================================================================
// Native EDN store/restore simulation
// Uses rusqlite to mimic what LegacyStorage does on WASM
// ===================================================================

/// Simulates LegacyStorage::store — writes an EDN node to a datascript table.
fn store_node_edn(conn: &Connection, addr: i64, level: u32, keys: &[Datom], addrs: Option<&[i64]>) {
    let content = node_to_edn(level, keys, addrs);
    conn.execute(
        "INSERT OR REPLACE INTO datascript (addr, content) VALUES (?1, ?2)",
        rusqlite::params![addr, content],
    )
    .unwrap();
}

/// Simulates LegacyStorage::restore — reads an EDN node from a datascript table.
fn restore_node_edn(
    conn: &Connection,
    addr: i64,
    rschema: &ReverseSchema,
) -> (u32, Vec<Datom>, Option<Vec<i64>>) {
    let content: String = conn
        .query_row(
            "SELECT content FROM datascript WHERE addr = ?1",
            [addr],
            |row| row.get(0),
        )
        .unwrap();
    let edn = parse_edn(&content);
    node_from_edn(&edn, rschema)
}

/// Write metadata to addr=0.
fn store_metadata_edn(conn: &Connection, meta: &LegacyMetadata) {
    let content = metadata_to_edn(meta);
    conn.execute(
        "INSERT OR REPLACE INTO datascript (addr, content) VALUES (0, ?1)",
        rusqlite::params![content],
    )
    .unwrap();
}

/// Read metadata from addr=0.
fn restore_metadata_edn(conn: &Connection) -> LegacyMetadata {
    let content: String = conn
        .query_row(
            "SELECT content FROM datascript WHERE addr = 0",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let edn = parse_edn(&content);
    metadata_from_edn(&edn)
}

// ===================================================================
// Tests: native store/restore simulation
// ===================================================================

#[test]
fn store_restore_leaf_node() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE datascript (addr INTEGER PRIMARY KEY, content TEXT)",
        [],
    )
    .unwrap();

    let rschema = build_rschema(&Schema::default());
    let datoms = vec![
        Datom::new(1, Some(kw("name")), Value::Str("Alice".into()), 536870913),
        Datom::new(2, Some(kw("name")), Value::Str("Bob".into()), 536870913),
        Datom::new(3, Some(kw("age")), Value::Long(30), 536870913),
    ];

    store_node_edn(&conn, 100, 0, &datoms, None);
    let (level, keys, addrs) = restore_node_edn(&conn, 100, &rschema);

    assert_eq!(level, 0);
    assert_eq!(keys.len(), 3);
    assert!(addrs.is_none());
    assert_eq!(keys[0].e, 1);
    assert_eq!(keys[0].v, Value::Str("Alice".into()));
    assert_eq!(keys[2].v, Value::Long(30));
}

#[test]
fn store_restore_branch_node() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE datascript (addr INTEGER PRIMARY KEY, content TEXT)",
        [],
    )
    .unwrap();

    let rschema = build_rschema(&Schema::default());

    // Store two leaf nodes
    let leaf1 = vec![
        Datom::new(1, Some(kw("name")), Value::Str("Alice".into()), 100),
    ];
    let leaf2 = vec![
        Datom::new(2, Some(kw("name")), Value::Str("Bob".into()), 100),
    ];
    store_node_edn(&conn, 10, 0, &leaf1, None);
    store_node_edn(&conn, 11, 0, &leaf2, None);

    // Store a branch pointing to both leaves
    let branch_keys = vec![
        Datom::new(1, Some(kw("name")), Value::Str("Alice".into()), 100),
    ];
    store_node_edn(&conn, 20, 1, &branch_keys, Some(&[10, 11]));

    // Restore branch
    let (level, keys, addrs) = restore_node_edn(&conn, 20, &rschema);
    assert_eq!(level, 1);
    assert_eq!(keys.len(), 1);
    assert_eq!(addrs.unwrap(), vec![10, 11]);

    // Restore children
    let (l1, k1, _) = restore_node_edn(&conn, 10, &rschema);
    let (l2, k2, _) = restore_node_edn(&conn, 11, &rschema);
    assert_eq!(l1, 0);
    assert_eq!(l2, 0);
    assert_eq!(k1[0].v, Value::Str("Alice".into()));
    assert_eq!(k2[0].v, Value::Str("Bob".into()));
}

#[test]
fn store_restore_metadata() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE datascript (addr INTEGER PRIMARY KEY, content TEXT)",
        [],
    )
    .unwrap();

    let mut schema = Schema::default();
    schema
        .attrs
        .insert(kw("name"), AttrSchema { index: true, ..Default::default() });
    schema.attrs.insert(
        kw("parent"),
        AttrSchema {
            value_type: Some(ValueType::Ref),
            ..Default::default()
        },
    );
    let rschema = build_rschema(&schema);

    let meta = LegacyMetadata {
        schema,
        rschema,
        eavt_root: 500,
        aevt_root: 600,
        avet_root: 700,
        max_eid: 42,
        max_tx: 536870913,
        max_addr: 800,
        branching_factor: 512,
    };

    store_metadata_edn(&conn, &meta);
    let restored = restore_metadata_edn(&conn);

    assert_eq!(restored.eavt_root, 500);
    assert_eq!(restored.aevt_root, 600);
    assert_eq!(restored.avet_root, 700);
    assert_eq!(restored.max_eid, 42);
    assert_eq!(restored.max_tx, 536870913);
    assert_eq!(restored.max_addr, 800);
    assert_eq!(restored.branching_factor, 512);
    assert!(restored.schema.attrs[&kw("name")].index);
    assert_eq!(
        restored.schema.attrs[&kw("parent")].value_type,
        Some(ValueType::Ref)
    );
    assert!(restored.rschema.is_indexed(&kw("name")));
    assert!(restored.rschema.is_ref(&kw("parent")));
}

#[test]
fn store_restore_mixed_value_types() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE datascript (addr INTEGER PRIMARY KEY, content TEXT)",
        [],
    )
    .unwrap();

    let mut schema = Schema::default();
    schema.attrs.insert(
        kw("parent"),
        AttrSchema {
            value_type: Some(ValueType::Ref),
            ..Default::default()
        },
    );
    let rschema = build_rschema(&schema);

    let datoms = vec![
        Datom::new(1, Some(kw("name")), Value::Str("Alice".into()), 100),
        Datom::new(1, Some(kw("age")), Value::Long(30), 100),
        Datom::new(1, Some(kw("active")), Value::Bool(true), 100),
        Datom::new(1, Some(kw("parent")), Value::Ref(2), 100),
        Datom::new(
            1,
            Some(kw("type")),
            Value::Keyword(Attr::Keyword {
                ns: None,
                name: "person".into(),
            }),
            100,
        ),
    ];

    store_node_edn(&conn, 50, 0, &datoms, None);
    let (_, keys, _) = restore_node_edn(&conn, 50, &rschema);

    assert_eq!(keys.len(), 5);
    assert_eq!(keys[0].v, Value::Str("Alice".into()));
    assert_eq!(keys[1].v, Value::Long(30));
    assert_eq!(keys[2].v, Value::Bool(true));
    assert_eq!(keys[3].v, Value::Ref(2));
    assert_eq!(
        keys[4].v,
        Value::Keyword(Attr::Keyword {
            ns: None,
            name: "person".into()
        })
    );
}

#[test]
fn store_restore_empty_leaf() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE datascript (addr INTEGER PRIMARY KEY, content TEXT)",
        [],
    )
    .unwrap();

    let rschema = build_rschema(&Schema::default());
    store_node_edn(&conn, 10, 0, &[], None);
    let (level, keys, addrs) = restore_node_edn(&conn, 10, &rschema);
    assert_eq!(level, 0);
    assert!(keys.is_empty());
    assert!(addrs.is_none());
}

#[test]
fn overwrite_metadata() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE datascript (addr INTEGER PRIMARY KEY, content TEXT)",
        [],
    )
    .unwrap();

    let schema = Schema::default();
    let rschema = build_rschema(&schema);

    let meta1 = LegacyMetadata {
        schema: schema.clone(),
        rschema: rschema.clone(),
        eavt_root: 100,
        aevt_root: 200,
        avet_root: 300,
        max_eid: 10,
        max_tx: 536870913,
        max_addr: 400,
        branching_factor: 512,
    };
    store_metadata_edn(&conn, &meta1);

    // Overwrite with new roots
    let meta2 = LegacyMetadata {
        schema,
        rschema,
        eavt_root: 500,
        aevt_root: 600,
        avet_root: 700,
        max_eid: 50,
        max_tx: 536870914,
        max_addr: 800,
        branching_factor: 512,
    };
    store_metadata_edn(&conn, &meta2);

    let restored = restore_metadata_edn(&conn);
    assert_eq!(restored.eavt_root, 500);
    assert_eq!(restored.max_eid, 50);
    assert_eq!(restored.max_tx, 536870914);
}

#[test]
fn list_and_delete_addresses() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE datascript (addr INTEGER PRIMARY KEY, content TEXT)",
        [],
    )
    .unwrap();

    // Store metadata (addr 0), tail (addr 1), and 3 nodes (addrs 10, 11, 12)
    conn.execute("INSERT INTO datascript VALUES (0, '{}')", []).unwrap();
    conn.execute("INSERT INTO datascript VALUES (1, '[]')", []).unwrap();

    let d = Datom::new(1, Some(kw("x")), Value::Long(1), 100);
    store_node_edn(&conn, 10, 0, &[d.clone()], None);
    store_node_edn(&conn, 11, 0, &[d.clone()], None);
    store_node_edn(&conn, 12, 0, &[d], None);

    // List addresses >= 2 (skip metadata and tail)
    let mut addrs: Vec<i64> = conn
        .prepare("SELECT addr FROM datascript WHERE addr >= 2")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    addrs.sort();
    assert_eq!(addrs, vec![10, 11, 12]);

    // Delete one
    conn.execute("DELETE FROM datascript WHERE addr = 11", []).unwrap();

    let mut remaining: Vec<i64> = conn
        .prepare("SELECT addr FROM datascript WHERE addr >= 2")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    remaining.sort();
    assert_eq!(remaining, vec![10, 12]);
}

// ===================================================================
// Tests against real bp7 data
// ===================================================================

/// Recursively collect all datoms from a tree stored in the datascript table,
/// using the legacy_edn module's typed parsing.
fn collect_typed_datoms(
    rows: &HashMap<i64, String>,
    addr: i64,
    rschema: &ReverseSchema,
) -> Vec<Datom> {
    let content = rows
        .get(&addr)
        .unwrap_or_else(|| panic!("addr {} not found", addr));
    let edn = parse_edn(content);
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

#[test]
fn bp7_datom_roundtrip_through_edn() {
    // Read real bp7 datoms, convert to EDN, re-parse, verify identical
    let conn = bp7_conn();
    let rows = read_all_rows(&conn);
    let meta_edn = parse_edn(rows.get(&0).unwrap());
    let meta = metadata_from_edn(&meta_edn);

    let eavt_datoms = collect_typed_datoms(&rows, meta.eavt_root, &meta.rschema);
    assert!(!eavt_datoms.is_empty());

    // Round-trip each datom through EDN
    for d in eavt_datoms.iter().take(100) {
        let edn_str = datom_to_edn(d);
        let reparsed = parse_edn(&edn_str);
        if let EdnValue::Vector(ref fields) = reparsed {
            let d2 = datom_from_edn(fields, &meta.rschema);
            assert_eq!(d.e, d2.e, "entity mismatch for datom: {}", edn_str);
            assert_eq!(d.a, d2.a, "attr mismatch for datom: {}", edn_str);
            assert_eq!(d.tx, d2.tx, "tx mismatch for datom: {}", edn_str);
            // Value may differ for Ref vs Long if not disambiguated, but
            // the round-trip with the same rschema should match
            assert_eq!(d.v, d2.v, "value mismatch for datom: {}", edn_str);
        } else {
            panic!("failed to re-parse datom EDN: {}", edn_str);
        }
    }
}

#[test]
fn bp7_node_roundtrip_through_edn() {
    // Read a real bp7 leaf node, convert to EDN, re-parse, verify identical
    let conn = bp7_conn();
    let rows = read_all_rows(&conn);
    let meta_edn = parse_edn(rows.get(&0).unwrap());
    let meta = metadata_from_edn(&meta_edn);

    // Find a leaf node
    fn find_leaf(rows: &HashMap<i64, String>, addr: i64) -> i64 {
        let edn = parse_edn(rows.get(&addr).unwrap());
        let level = edn_get_int(&edn, "level");
        if level == 0 {
            return addr;
        }
        if let Some(addrs_val) = persistent_sorted_set::legacy_edn::edn_get(&edn, "addresses") {
            if let EdnValue::Vector(ref arr) = addrs_val {
                if let Some(EdnValue::Integer(child)) = arr.first() {
                    return find_leaf(rows, *child);
                }
            }
        }
        panic!("no leaf found");
    }

    let leaf_addr = find_leaf(&rows, meta.eavt_root);
    let original_content = rows.get(&leaf_addr).unwrap();
    let original_edn = parse_edn(original_content);
    let (level, keys, _) = node_from_edn(&original_edn, &meta.rschema);
    assert_eq!(level, 0);

    // Write the parsed datoms back to EDN
    let regenerated = node_to_edn(0, &keys, None);
    let reparsed = parse_edn(&regenerated);
    let (level2, keys2, _) = node_from_edn(&reparsed, &meta.rschema);

    assert_eq!(level2, 0);
    assert_eq!(keys.len(), keys2.len());
    for (a, b) in keys.iter().zip(keys2.iter()) {
        assert_eq!(a.e, b.e);
        assert_eq!(a.a, b.a);
        assert_eq!(a.v, b.v);
        assert_eq!(a.tx, b.tx);
    }
}

#[test]
fn bp7_metadata_roundtrip_through_edn() {
    let conn = bp7_conn();
    let rows = read_all_rows(&conn);
    let original_content = rows.get(&0).unwrap();
    let original_edn = parse_edn(original_content);
    let meta = metadata_from_edn(&original_edn);

    // Write metadata back to EDN and re-parse
    let regenerated = metadata_to_edn(&meta);
    let reparsed = parse_edn(&regenerated);
    let meta2 = metadata_from_edn(&reparsed);

    assert_eq!(meta.eavt_root, meta2.eavt_root);
    assert_eq!(meta.aevt_root, meta2.aevt_root);
    assert_eq!(meta.avet_root, meta2.avet_root);
    assert_eq!(meta.max_eid, meta2.max_eid);
    assert_eq!(meta.max_tx, meta2.max_tx);
    assert_eq!(meta.max_addr, meta2.max_addr);
    assert_eq!(meta.branching_factor, meta2.branching_factor);
    assert_eq!(meta.schema.attrs.len(), meta2.schema.attrs.len());
}

#[test]
fn bp7_schema_roundtrip_through_edn() {
    let conn = bp7_conn();
    let rows = read_all_rows(&conn);
    let meta_edn = parse_edn(rows.get(&0).unwrap());
    let meta = metadata_from_edn(&meta_edn);

    let schema_edn_str = schema_to_edn(&meta.schema);
    let reparsed = parse_edn(&schema_edn_str);
    let schema2 = schema_from_edn(&reparsed);

    assert_eq!(meta.schema.attrs.len(), schema2.attrs.len());
    for (attr, props) in &meta.schema.attrs {
        let props2 = schema2
            .attrs
            .get(attr)
            .unwrap_or_else(|| panic!("attr {:?} missing after roundtrip", attr));
        assert_eq!(props.index, props2.index, "index mismatch for {:?}", attr);
        assert_eq!(props.unique, props2.unique, "unique mismatch for {:?}", attr);
        assert_eq!(
            props.cardinality, props2.cardinality,
            "cardinality mismatch for {:?}",
            attr
        );
        assert_eq!(
            props.value_type, props2.value_type,
            "value_type mismatch for {:?}",
            attr
        );
        assert_eq!(
            props.is_component, props2.is_component,
            "is_component mismatch for {:?}",
            attr
        );
    }
}

#[test]
fn bp7_write_datoms_to_new_db_then_read_back() {
    // Simulate the full legacy storage pipeline:
    // 1. Read datoms from real bp7
    // 2. Write them as EDN to a fresh in-memory database
    // 3. Read them back and verify
    let conn = bp7_conn();
    let rows = read_all_rows(&conn);
    let meta_edn = parse_edn(rows.get(&0).unwrap());
    let meta = metadata_from_edn(&meta_edn);

    let eavt_datoms = collect_typed_datoms(&rows, meta.eavt_root, &meta.rschema);

    // Create a fresh database
    let fresh = Connection::open_in_memory().unwrap();
    fresh
        .execute(
            "CREATE TABLE datascript (addr INTEGER PRIMARY KEY, content TEXT)",
            [],
        )
        .unwrap();

    // Write datoms as a single leaf node (simplified — real PSS would build a tree)
    let chunks: Vec<&[Datom]> = eavt_datoms.chunks(100).collect();
    let mut addr = 10i64;
    let mut leaf_addrs = Vec::new();
    for chunk in &chunks {
        store_node_edn(&fresh, addr, 0, chunk, None);
        leaf_addrs.push(addr);
        addr += 1;
    }

    // Write metadata
    let rschema = build_rschema(&meta.schema);
    let new_meta = LegacyMetadata {
        schema: meta.schema.clone(),
        rschema,
        eavt_root: leaf_addrs[0], // simplified — just first leaf
        aevt_root: leaf_addrs[0],
        avet_root: leaf_addrs[0],
        max_eid: meta.max_eid,
        max_tx: meta.max_tx,
        max_addr: addr,
        branching_factor: meta.branching_factor,
    };
    store_metadata_edn(&fresh, &new_meta);

    // Read back
    let restored_meta = restore_metadata_edn(&fresh);
    assert_eq!(restored_meta.max_eid, meta.max_eid);
    assert_eq!(restored_meta.max_tx, meta.max_tx);

    // Read back the first leaf and verify datoms match
    let (level, keys, _) = restore_node_edn(&fresh, leaf_addrs[0], &meta.rschema);
    assert_eq!(level, 0);
    assert_eq!(keys.len(), chunks[0].len());
    for (original, restored) in chunks[0].iter().zip(keys.iter()) {
        assert_eq!(original.e, restored.e);
        assert_eq!(original.a, restored.a);
        assert_eq!(original.v, restored.v);
        assert_eq!(original.tx, restored.tx);
    }
}

#[test]
fn bp7_full_tree_datom_count_matches() {
    // Verify that reading all 3 indexes through legacy_edn gives consistent counts
    let conn = bp7_conn();
    let rows = read_all_rows(&conn);
    let meta_edn = parse_edn(rows.get(&0).unwrap());
    let meta = metadata_from_edn(&meta_edn);

    let eavt = collect_typed_datoms(&rows, meta.eavt_root, &meta.rschema);
    let aevt = collect_typed_datoms(&rows, meta.aevt_root, &meta.rschema);
    let avet = collect_typed_datoms(&rows, meta.avet_root, &meta.rschema);

    assert_eq!(
        eavt.len(),
        aevt.len(),
        "EAVT and AEVT should have same datom count"
    );
    assert!(
        avet.len() <= eavt.len(),
        "AVET ({}) should have <= EAVT ({}) datoms",
        avet.len(),
        eavt.len()
    );
    assert!(avet.len() > 0, "AVET should have some indexed datoms");

    // All AVET datoms should be for indexed attrs
    for d in &avet {
        if let Some(ref attr) = d.a {
            assert!(
                meta.rschema.is_indexed(attr),
                "AVET datom has non-indexed attr: {:?}",
                attr
            );
        }
    }
}

#[test]
fn bp7_eavt_datoms_are_entity_sorted() {
    let conn = bp7_conn();
    let rows = read_all_rows(&conn);
    let meta_edn = parse_edn(rows.get(&0).unwrap());
    let meta = metadata_from_edn(&meta_edn);

    let eavt = collect_typed_datoms(&rows, meta.eavt_root, &meta.rschema);

    // EAVT should be sorted by entity (primary), then attribute, then value
    for window in eavt.windows(2) {
        let (a, b) = (&window[0], &window[1]);
        assert!(
            a.e <= b.e,
            "EAVT not sorted by entity: e={} then e={}",
            a.e,
            b.e
        );
    }
}

#[test]
fn bp7_aevt_datoms_are_attr_sorted() {
    let conn = bp7_conn();
    let rows = read_all_rows(&conn);
    let meta_edn = parse_edn(rows.get(&0).unwrap());
    let meta = metadata_from_edn(&meta_edn);

    let aevt = collect_typed_datoms(&rows, meta.aevt_root, &meta.rschema);

    // AEVT should be sorted by attribute (primary), then entity
    for window in aevt.windows(2) {
        let (a, b) = (&window[0], &window[1]);
        if let (Some(ref aa), Some(ref ba)) = (&a.a, &b.a) {
            assert!(
                aa <= ba,
                "AEVT not sorted by attr: {:?} then {:?}",
                aa,
                ba
            );
        }
    }
}
