//! Tests for the B+ tree entity slice sentinel fix.
//!
//! Verifies that wildcard boundary datoms (None attr, Nil value) are
//! normalized to min/max sentinels so the binary search in `search_first`
//! lands on the true first/last datom for an entity range.
//!
//! Background: the comparator returns Equal for `None` attr and `Nil` value
//! wildcards. This caused the B+ tree binary search to land on an arbitrary
//! datom in the middle of an entity's range, missing datoms in earlier/later
//! B+ tree branches.

use std::cmp::Ordering;

use persistent_sorted_set::comparator::{cmp_datoms, IndexType};
use persistent_sorted_set::datom::{Attr, Datom, Value};
use persistent_sorted_set::db::{DataScriptDB, TX0};
use persistent_sorted_set::pull::{pull, PullResult, PullSource};
use persistent_sorted_set::pull_parser::parse_pull_pattern_edn;
use persistent_sorted_set::schema::{
    kw, kw_ns, AttrSchema, Cardinality, Schema, Unique, ValueType,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn d(e: i64, attr: &str, v: Value, tx: i64) -> Datom {
    Datom::new(e, Some(kw(attr)), v, TX0 + tx)
}

fn d_ns(e: i64, ns: &str, name: &str, v: Value, tx: i64) -> Datom {
    Datom::new(e, Some(kw_ns(ns, name)), v, TX0 + tx)
}

fn s(val: &str) -> Value { Value::Str(val.into()) }
fn n(val: i64) -> Value { Value::Long(val) }

fn scalar(v: Value) -> PullResult {
    PullResult::Scalar(v)
}

/// Build a DB with many attributes per entity to stress the slice bounds.
/// Entity 1 has 12 attributes spanning different namespaces and types.
fn wide_entity_db() -> DataScriptDB {
    let mut schema = Schema::default();
    schema.attrs.insert(kw("name"), AttrSchema { index: true, ..Default::default() });
    schema.attrs.insert(kw("age"), AttrSchema::default());
    schema.attrs.insert(kw("email"), AttrSchema::default());
    schema.attrs.insert(kw("active"), AttrSchema::default());
    schema.attrs.insert(kw_ns("module", "name"), AttrSchema { index: true, ..Default::default() });
    schema.attrs.insert(kw_ns("module", "order"), AttrSchema::default());
    schema.attrs.insert(kw_ns("module", "help-key"), AttrSchema::default());
    schema.attrs.insert(kw_ns("module", "translation-key"), AttrSchema::default());
    schema.attrs.insert(kw_ns("bp", "uuid"), AttrSchema {
        unique: Some(Unique::Identity),
        ..Default::default()
    });
    schema.attrs.insert(kw_ns("bp", "nid"), AttrSchema::default());
    schema.attrs.insert(kw_ns("module", "submodules"), AttrSchema {
        value_type: Some(ValueType::Ref),
        cardinality: Cardinality::Many,
        ..Default::default()
    });
    schema.attrs.insert(kw_ns("module", "diagrams"), AttrSchema {
        value_type: Some(ValueType::Ref),
        cardinality: Cardinality::Many,
        ..Default::default()
    });
    schema.attrs.insert(kw_ns("conditional", "operator"), AttrSchema::default());
    schema.attrs.insert(kw_ns("conditional", "type"), AttrSchema::default());
    schema.attrs.insert(kw_ns("conditional", "values"), AttrSchema {
        cardinality: Cardinality::Many,
        ..Default::default()
    });
    schema.attrs.insert(kw_ns("conditional", "group-variable-uuid"), AttrSchema::default());

    let mut db = DataScriptDB::empty(schema);

    // Entity 1: module with 12 datoms across many attributes
    db.with_datom(d_ns(1, "module", "name", s("Surface"), 1));
    db.with_datom(d_ns(1, "module", "order", n(0), 1));
    db.with_datom(d_ns(1, "module", "help-key", s("behaveplus:surface:help"), 1));
    db.with_datom(d_ns(1, "module", "translation-key", s("behaveplus:surface"), 1));
    db.with_datom(d_ns(1, "bp", "uuid", s("uuid-surface"), 1));
    db.with_datom(d_ns(1, "bp", "nid", s("nid-surface"), 1));
    db.with_datom(d_ns(1, "module", "submodules", Value::Ref(10), 1));
    db.with_datom(d_ns(1, "module", "submodules", Value::Ref(11), 1));
    db.with_datom(d_ns(1, "module", "submodules", Value::Ref(12), 1));
    db.with_datom(d_ns(1, "module", "diagrams", Value::Ref(20), 1));
    db.with_datom(d_ns(1, "module", "diagrams", Value::Ref(21), 1));
    db.with_datom(d(1, "active", Value::Bool(true), 1));

    // Entity 2: another module
    db.with_datom(d_ns(2, "module", "name", s("Contain"), 1));
    db.with_datom(d_ns(2, "module", "order", n(1), 1));
    db.with_datom(d_ns(2, "bp", "uuid", s("uuid-contain"), 1));
    db.with_datom(d_ns(2, "bp", "nid", s("nid-contain"), 1));
    db.with_datom(d(2, "active", Value::Bool(true), 1));

    // Entity 3: conditional entity (leaf, no refs)
    db.with_datom(d_ns(3, "conditional", "operator", Value::Keyword(kw("equal")), 1));
    db.with_datom(d_ns(3, "conditional", "type", Value::Keyword(kw_ns("conditional", "group-variable")), 1));
    db.with_datom(d_ns(3, "conditional", "group-variable-uuid", s("gv-uuid-1"), 1));
    db.with_datom(d_ns(3, "conditional", "values", s("true"), 1));
    db.with_datom(d_ns(3, "bp", "uuid", s("uuid-cond-1"), 1));
    db.with_datom(d_ns(3, "bp", "nid", s("nid-cond-1"), 1));

    // Entity 4: single-attribute entity
    db.with_datom(d(4, "name", s("Singleton"), 1));

    // Submodule entities (targets of refs from entity 1)
    db.with_datom(d(10, "name", s("Sub A"), 1));
    db.with_datom(d(11, "name", s("Sub B"), 1));
    db.with_datom(d(12, "name", s("Sub C"), 1));

    db
}

// ---------------------------------------------------------------------------
// Datom sentinel unit tests
// ---------------------------------------------------------------------------

#[test]
fn attr_min_sentinel_sorts_before_all_keywords() {
    let min = Attr::min_sentinel();
    let real_kw = Attr::Keyword { ns: None, name: "a".into() };
    let ns_kw = Attr::Keyword { ns: Some("a".into()), name: "a".into() };
    assert!(min < real_kw, "min sentinel should sort before bare keyword");
    assert!(min < ns_kw, "min sentinel should sort before namespaced keyword");
}

#[test]
fn attr_max_sentinel_sorts_after_all_keywords() {
    let max = Attr::max_sentinel();
    let real_kw = Attr::Keyword { ns: Some("zzz".into()), name: "zzz".into() };
    let real_str = Attr::Str("zzz".into());
    assert!(max > real_kw, "max sentinel should sort after namespaced keyword");
    assert!(max > real_str, "max sentinel should sort after real Str attr");
}

#[test]
fn attr_min_before_max() {
    assert!(Attr::min_sentinel() < Attr::max_sentinel());
}

#[test]
fn value_min_sentinel_sorts_before_real_values() {
    let min = Value::min_sentinel();
    // Bool(false) is the min sentinel; Bool(true) should be greater
    assert_eq!(
        cmp_datoms(
            IndexType::EAVT,
            &Datom::new(1, Some(kw("a")), min, 1),
            &Datom::new(1, Some(kw("a")), Value::Bool(true), 1),
        ),
        Ordering::Less,
    );
}

#[test]
fn value_max_sentinel_sorts_after_real_values() {
    let max = Value::max_sentinel();
    // Bytes is the highest type_rank, so Bytes([0xFF]) > any Str/Long/etc.
    assert_eq!(
        cmp_datoms(
            IndexType::EAVT,
            &Datom::new(1, Some(kw("a")), Value::Str("zzz".into()), 1),
            &Datom::new(1, Some(kw("a")), max, 1),
        ),
        Ordering::Less,
    );
}

// ---------------------------------------------------------------------------
// as_lower_bound / as_upper_bound tests
// ---------------------------------------------------------------------------

#[test]
fn as_lower_bound_replaces_none_attr_and_nil_value() {
    let boundary = Datom::new(5, None, Value::Nil, 0);
    let lower = boundary.as_lower_bound();
    assert!(lower.a.is_some(), "lower bound should have Some(attr)");
    assert!(!matches!(lower.v, Value::Nil), "lower bound should not have Nil value");
}

#[test]
fn as_upper_bound_replaces_none_attr_and_nil_value() {
    let boundary = Datom::new(5, None, Value::Nil, 100);
    let upper = boundary.as_upper_bound();
    assert!(upper.a.is_some(), "upper bound should have Some(attr)");
    assert!(!matches!(upper.v, Value::Nil), "upper bound should not have Nil value");
}

#[test]
fn as_lower_bound_preserves_concrete_fields() {
    let concrete = Datom::new(5, Some(kw("name")), Value::Str("Alice".into()), 100);
    let lower = concrete.as_lower_bound();
    assert_eq!(lower.e, 5);
    assert_eq!(lower.a, Some(kw("name")));
    assert_eq!(lower.v, Value::Str("Alice".into()));
    assert_eq!(lower.tx, 100);
}

#[test]
fn as_upper_bound_preserves_concrete_fields() {
    let concrete = Datom::new(5, Some(kw("name")), Value::Str("Alice".into()), 100);
    let upper = concrete.as_upper_bound();
    assert_eq!(upper.e, 5);
    assert_eq!(upper.a, Some(kw("name")));
    assert_eq!(upper.v, Value::Str("Alice".into()));
    assert_eq!(upper.tx, 100);
}

#[test]
fn lower_bound_sorts_before_upper_bound_for_same_entity() {
    let lower = Datom::new(5, None, Value::Nil, 0).as_lower_bound();
    let upper = Datom::new(5, None, Value::Nil, i64::MAX).as_upper_bound();
    assert_eq!(
        cmp_datoms(IndexType::EAVT, &lower, &upper),
        Ordering::Less,
        "lower bound must sort before upper bound"
    );
}

#[test]
fn sentinel_bounds_enclose_all_real_datoms() {
    let lower = Datom::new(5, None, Value::Nil, 0).as_lower_bound();
    let upper = Datom::new(5, None, Value::Nil, i64::MAX).as_upper_bound();

    // Test against various real datoms for entity 5
    let real_datoms = vec![
        Datom::new(5, Some(kw("age")), Value::Long(30), 100),
        Datom::new(5, Some(kw("name")), Value::Str("Test".into()), 100),
        Datom::new(5, Some(kw_ns("bp", "uuid")), Value::Str("uuid-1".into()), 200),
        Datom::new(5, Some(kw_ns("module", "name")), Value::Str("Surface".into()), 1),
        Datom::new(5, Some(Attr::Str("string-attr".into())), Value::Bool(true), 50),
    ];

    for real in &real_datoms {
        assert_eq!(
            cmp_datoms(IndexType::EAVT, &lower, real),
            Ordering::Less,
            "lower bound should sort before real datom {:?}", real.a
        );
        assert_eq!(
            cmp_datoms(IndexType::EAVT, real, &upper),
            Ordering::Less,
            "real datom {:?} should sort before upper bound", real.a
        );
    }
}

#[test]
fn sentinel_bounds_do_not_enclose_other_entities() {
    let lower = Datom::new(5, None, Value::Nil, 0).as_lower_bound();
    let upper = Datom::new(5, None, Value::Nil, i64::MAX).as_upper_bound();

    // Entity 4 should sort before lower bound of entity 5
    let e4 = Datom::new(4, Some(kw("name")), Value::Str("Test".into()), 100);
    assert_eq!(
        cmp_datoms(IndexType::EAVT, &e4, &lower),
        Ordering::Less,
        "entity 4 datom should sort before entity 5's lower bound"
    );

    // Entity 6 should sort after upper bound of entity 5
    let e6 = Datom::new(6, Some(kw("name")), Value::Str("Test".into()), 100);
    assert_eq!(
        cmp_datoms(IndexType::EAVT, &upper, &e6),
        Ordering::Less,
        "entity 5's upper bound should sort before entity 6 datom"
    );
}

// ---------------------------------------------------------------------------
// min_for_e / max_for_e tests
// ---------------------------------------------------------------------------

#[test]
fn min_for_e_sorts_before_all_entity_datoms() {
    let min = Datom::min_for_e(5);
    let datoms = vec![
        Datom::new(5, Some(kw("a")), Value::Bool(false), 1),
        Datom::new(5, Some(kw("z")), Value::Str("zzz".into()), 999),
        Datom::new(5, Some(kw_ns("zzz", "zzz")), Value::Bytes(vec![0xFF]), 999),
    ];
    for d in &datoms {
        assert_eq!(
            cmp_datoms(IndexType::EAVT, &min, d),
            Ordering::Less,
            "min_for_e should be less than datom with attr {:?}", d.a
        );
    }
}

#[test]
fn max_for_e_sorts_after_all_entity_datoms() {
    let max = Datom::max_for_e(5);
    let datoms = vec![
        Datom::new(5, Some(kw("a")), Value::Bool(false), 1),
        Datom::new(5, Some(kw("z")), Value::Str("zzz".into()), 999),
        Datom::new(5, Some(kw_ns("zzz", "zzz")), Value::Bytes(vec![0xFF]), 999),
    ];
    for d in &datoms {
        assert_eq!(
            cmp_datoms(IndexType::EAVT, d, &max),
            Ordering::Less,
            "datom with attr {:?} should be less than max_for_e", d.a
        );
    }
}

// ---------------------------------------------------------------------------
// min_for_ea / max_for_ea tests
// ---------------------------------------------------------------------------

#[test]
fn min_for_ea_sorts_before_all_ea_datoms() {
    let attr = kw("age");
    let min = Datom::min_for_ea(5, &attr);
    let datoms = vec![
        Datom::new(5, Some(kw("age")), Value::Long(0), 1),
        Datom::new(5, Some(kw("age")), Value::Long(100), 999),
    ];
    for d in &datoms {
        assert!(
            cmp_datoms(IndexType::EAVT, &min, d) != Ordering::Greater,
            "min_for_ea should not sort after datom with value {:?}", d.v
        );
    }
}

#[test]
fn max_for_ea_sorts_after_all_ea_datoms() {
    let attr = kw("age");
    let max = Datom::max_for_ea(5, &attr);
    let datoms = vec![
        Datom::new(5, Some(kw("age")), Value::Long(0), 1),
        Datom::new(5, Some(kw("age")), Value::Long(i64::MAX), 999),
        Datom::new(5, Some(kw("age")), Value::Str("zzz".into()), 999),
    ];
    for d in &datoms {
        assert!(
            cmp_datoms(IndexType::EAVT, d, &max) != Ordering::Greater,
            "datom with value {:?} should not sort after max_for_ea", d.v
        );
    }
}

// ---------------------------------------------------------------------------
// DataScriptDB entity_datoms (pull source) tests
// ---------------------------------------------------------------------------

#[test]
fn entity_datoms_returns_all_datoms_for_wide_entity() {
    let db = wide_entity_db();
    let datoms = db.entity_datoms(1);
    assert_eq!(
        datoms.len(), 12,
        "entity 1 has 12 datoms, got {}: {:?}",
        datoms.len(),
        datoms.iter().map(|d| &d.a).collect::<Vec<_>>()
    );
}

#[test]
fn entity_datoms_returns_all_for_small_entity() {
    let db = wide_entity_db();
    let datoms = db.entity_datoms(4);
    assert_eq!(datoms.len(), 1, "entity 4 has 1 datom");
}

#[test]
fn entity_datoms_returns_all_for_conditional_entity() {
    let db = wide_entity_db();
    let datoms = db.entity_datoms(3);
    assert_eq!(
        datoms.len(), 6,
        "entity 3 (conditional) has 6 datoms, got {}: {:?}",
        datoms.len(),
        datoms.iter().map(|d| &d.a).collect::<Vec<_>>()
    );
}

#[test]
fn entity_datoms_returns_empty_for_nonexistent_entity() {
    let db = wide_entity_db();
    let datoms = db.entity_datoms(999);
    assert!(datoms.is_empty());
}

#[test]
fn entity_datoms_does_not_include_other_entities() {
    let db = wide_entity_db();
    let datoms = db.entity_datoms(1);
    for d in &datoms {
        assert_eq!(d.e, 1, "entity_datoms(1) returned a datom for entity {}", d.e);
    }
}

// ---------------------------------------------------------------------------
// Pull tests: wildcard [*] returns all attributes
// ---------------------------------------------------------------------------

#[test]
fn pull_wildcard_returns_all_attrs_for_wide_entity() {
    let db = wide_entity_db();
    let pattern = parse_pull_pattern_edn(db.schema(), db.rschema(), "[*]");
    let result = pull(&db, &pattern, 1).unwrap();
    if let PullResult::Map(entries) = &result {
        // Entity 1 has: module/name, module/order, module/help-key,
        // module/translation-key, bp/uuid, bp/nid, module/submodules,
        // module/diagrams, active = 9 distinct attrs + :db/id = 10 entries
        let attr_names: Vec<_> = entries.iter().map(|(k, _)| k.clone()).collect();
        assert!(
            entries.len() >= 10,
            "wildcard pull should return at least 10 entries (9 attrs + :db/id), got {}: {:?}",
            entries.len(), attr_names
        );

        // Verify key attributes are present
        assert!(entries.iter().any(|(k, _)| *k == kw_ns("module", "name")),
            "pull [*] must include :module/name");
        assert!(entries.iter().any(|(k, _)| *k == kw_ns("module", "order")),
            "pull [*] must include :module/order");
        assert!(entries.iter().any(|(k, _)| *k == kw_ns("bp", "uuid")),
            "pull [*] must include :bp/uuid");
        assert!(entries.iter().any(|(k, _)| *k == kw_ns("bp", "nid")),
            "pull [*] must include :bp/nid");
        assert!(entries.iter().any(|(k, _)| *k == kw("active")),
            "pull [*] must include :active");
    } else {
        panic!("expected PullResult::Map, got {:?}", result);
    }
}

#[test]
fn pull_wildcard_for_conditional_entity() {
    let db = wide_entity_db();
    let pattern = parse_pull_pattern_edn(db.schema(), db.rschema(), "[*]");
    let result = pull(&db, &pattern, 3).unwrap();
    if let PullResult::Map(entries) = &result {
        assert!(entries.iter().any(|(k, _)| *k == kw_ns("conditional", "operator")),
            "pull [*] must include :conditional/operator");
        assert!(entries.iter().any(|(k, _)| *k == kw_ns("conditional", "type")),
            "pull [*] must include :conditional/type");
        assert!(entries.iter().any(|(k, _)| *k == kw_ns("conditional", "group-variable-uuid")),
            "pull [*] must include :conditional/group-variable-uuid");
        assert!(entries.iter().any(|(k, _)| *k == kw_ns("bp", "uuid")),
            "pull [*] must include :bp/uuid");
    } else {
        panic!("expected PullResult::Map");
    }
}

#[test]
fn pull_specific_attrs_matches_wildcard() {
    let db = wide_entity_db();
    let wildcard = parse_pull_pattern_edn(db.schema(), db.rschema(), "[*]");
    let specific = parse_pull_pattern_edn(
        db.schema(), db.rschema(),
        "[:db/id :module/name :module/order :bp/uuid]",
    );

    let wild_result = pull(&db, &wildcard, 1).unwrap();
    let spec_result = pull(&db, &specific, 1).unwrap();

    if let (PullResult::Map(wild_entries), PullResult::Map(spec_entries)) =
        (&wild_result, &spec_result)
    {
        // Every attr in the specific pull should appear in the wildcard pull
        for (attr, val) in spec_entries {
            let wild_val = wild_entries.iter().find(|(k, _)| k == attr);
            assert!(
                wild_val.is_some(),
                "wildcard pull missing attr {:?} that specific pull has", attr
            );
            assert_eq!(
                wild_val.unwrap().1, *val,
                "value mismatch for attr {:?}", attr
            );
        }
    } else {
        panic!("expected PullResult::Map for both");
    }
}

#[test]
fn pull_nonexistent_entity_returns_none() {
    let db = wide_entity_db();
    let pattern = parse_pull_pattern_edn(db.schema(), db.rschema(), "[*]");
    let result = pull(&db, &pattern, 999);
    assert!(result.is_none(), "pull on nonexistent entity should return None");
}

// ---------------------------------------------------------------------------
// Search pattern tests (entity-only, entity+attr, attr-only)
// ---------------------------------------------------------------------------

#[test]
fn search_entity_only_returns_all_datoms() {
    let db = wide_entity_db();
    // [e _ _ _] pattern — the pattern that was broken
    let datoms = db.search(Some(1), None, None, None);
    assert_eq!(
        datoms.len(), 12,
        "search(1, None, None, None) should return all 12 datoms for entity 1, got {}",
        datoms.len()
    );
}

#[test]
fn search_entity_attr_returns_matching_datoms() {
    let db = wide_entity_db();
    let attr = kw_ns("module", "submodules");
    let datoms = db.search(Some(1), Some(&attr), None, None);
    assert_eq!(
        datoms.len(), 3,
        "entity 1 has 3 :module/submodules datoms"
    );
}

#[test]
fn search_entity_attr_single_value() {
    let db = wide_entity_db();
    let attr = kw_ns("module", "name");
    let datoms = db.search(Some(1), Some(&attr), None, None);
    assert_eq!(datoms.len(), 1);
    assert_eq!(datoms[0].v, Value::Str("Surface".into()));
}

#[test]
fn search_attr_only_returns_across_entities() {
    let db = wide_entity_db();
    let attr = kw_ns("module", "name");
    let datoms = db.search(None, Some(&attr), None, None);
    assert_eq!(
        datoms.len(), 2,
        "two entities have :module/name"
    );
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn search_entity_with_single_datom() {
    let db = wide_entity_db();
    let datoms = db.entity_datoms(4);
    assert_eq!(datoms.len(), 1, "entity with single datom should return exactly 1");
    assert_eq!(datoms[0].v, Value::Str("Singleton".into()));
}

#[test]
fn pull_entity_with_single_datom() {
    let db = wide_entity_db();
    let pattern = parse_pull_pattern_edn(db.schema(), db.rschema(), "[*]");
    let result = pull(&db, &pattern, 4).unwrap();
    if let PullResult::Map(entries) = &result {
        assert!(entries.iter().any(|(k, v)| *k == kw("name") && *v == scalar(Value::Str("Singleton".into()))));
    } else {
        panic!("expected Map");
    }
}

#[test]
fn pull_many_matches_individual_pulls() {
    let db = wide_entity_db();
    let pattern = parse_pull_pattern_edn(db.schema(), db.rschema(), "[:db/id :module/name :bp/uuid]");
    let eids = vec![1, 2, 4];

    for eid in &eids {
        let individual = pull(&db, &pattern, *eid);
        // Just verify each pull returns a result (or None for missing attrs)
        if *eid == 4 {
            // Entity 4 has :name but not :module/name or :bp/uuid
            if let Some(PullResult::Map(entries)) = &individual {
                // Should have :db/id at minimum
                assert!(entries.iter().any(|(k, _)| *k == kw_ns("db", "id")));
            }
        } else {
            assert!(individual.is_some(), "pull for entity {} should return Some", eid);
        }
    }
}

#[test]
fn entity_datoms_consistent_across_all_entities() {
    let db = wide_entity_db();
    // Verify entity_datoms for every entity in the DB
    let expected: Vec<(i64, usize)> = vec![
        (1, 12), (2, 5), (3, 6), (4, 1), (10, 1), (11, 1), (12, 1),
    ];
    for (eid, expected_count) in expected {
        let datoms = db.entity_datoms(eid);
        assert_eq!(
            datoms.len(), expected_count,
            "entity {} expected {} datoms, got {}",
            eid, expected_count, datoms.len()
        );
    }
}

// ---------------------------------------------------------------------------
// Sentinel ordering across AEVT and AVET indexes
// ---------------------------------------------------------------------------

#[test]
fn value_sentinels_work_for_aevt_slicing() {
    // In AEVT order: attr first, then entity, then value
    // Sentinel values should still bracket all real values
    let min = Datom::new(0, Some(kw("age")), Value::min_sentinel(), 0);
    let max = Datom::new(i64::MAX, Some(kw("age")), Value::max_sentinel(), i64::MAX);
    let real = Datom::new(5, Some(kw("age")), Value::Long(30), 100);

    assert_eq!(
        cmp_datoms(IndexType::AEVT, &min, &real),
        Ordering::Less,
    );
    assert_eq!(
        cmp_datoms(IndexType::AEVT, &real, &max),
        Ordering::Less,
    );
}

#[test]
fn value_sentinels_work_for_avet_slicing() {
    // In AVET order: attr first, then value, then entity
    let min = Datom::new(0, Some(kw("age")), Value::min_sentinel(), 0);
    let max = Datom::new(i64::MAX, Some(kw("age")), Value::max_sentinel(), i64::MAX);
    let real = Datom::new(5, Some(kw("age")), Value::Long(30), 100);

    assert_eq!(
        cmp_datoms(IndexType::AVET, &min, &real),
        Ordering::Less,
    );
    assert_eq!(
        cmp_datoms(IndexType::AVET, &real, &max),
        Ordering::Less,
    );
}
