//! Pull integration tests against `DataScriptDB`.

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

fn scalar(v: Value) -> PullResult {
    PullResult::Scalar(v)
}

fn test_db() -> DataScriptDB {
    let mut schema = Schema::default();
    schema.attrs.insert(kw("name"), AttrSchema { index: true, ..Default::default() });
    schema.attrs.insert(kw("age"), AttrSchema::default());
    schema.attrs.insert(kw("aka"), AttrSchema {
        cardinality: Cardinality::Many,
        ..Default::default()
    });
    schema.attrs.insert(kw("parent"), AttrSchema {
        value_type: Some(ValueType::Ref),
        ..Default::default()
    });
    schema.attrs.insert(kw("children"), AttrSchema {
        value_type: Some(ValueType::Ref),
        cardinality: Cardinality::Many,
        ..Default::default()
    });
    schema.attrs.insert(kw_ns("bp", "uuid"), AttrSchema {
        unique: Some(Unique::Identity),
        ..Default::default()
    });
    schema.attrs.insert(kw_ns("ws", "inputs"), AttrSchema {
        value_type: Some(ValueType::Ref),
        cardinality: Cardinality::Many,
        is_component: true,
        ..Default::default()
    });
    schema.attrs.insert(kw_ns("input", "value"), AttrSchema::default());

    let mut db = DataScriptDB::empty(schema);

    // Entity 1: Alice
    db.with_datom(d(1, "name", Value::Str("Alice".into()), 1));
    db.with_datom(d(1, "age", Value::Long(30), 1));
    db.with_datom(d(1, "aka", Value::Str("A".into()), 1));
    db.with_datom(d(1, "aka", Value::Str("Ali".into()), 1));
    db.with_datom(d_ns(1, "bp", "uuid", Value::Str("uuid-alice".into()), 1));

    // Entity 2: Bob, child of Alice
    db.with_datom(d(2, "name", Value::Str("Bob".into()), 1));
    db.with_datom(d(2, "age", Value::Long(10), 1));
    db.with_datom(d(2, "parent", Value::Ref(1), 1));
    db.with_datom(d_ns(2, "bp", "uuid", Value::Str("uuid-bob".into()), 1));

    // Entity 3: Carol, child of Alice
    db.with_datom(d(3, "name", Value::Str("Carol".into()), 1));
    db.with_datom(d(3, "parent", Value::Ref(1), 1));

    // Alice has children
    db.with_datom(d(1, "children", Value::Ref(2), 1));
    db.with_datom(d(1, "children", Value::Ref(3), 1));

    // Worksheet entity 10 with component inputs 11, 12
    db.with_datom(d_ns(10, "ws", "inputs", Value::Ref(11), 1));
    db.with_datom(d_ns(10, "ws", "inputs", Value::Ref(12), 1));
    db.with_datom(d_ns(11, "input", "value", Value::Str("v1".into()), 1));
    db.with_datom(d_ns(12, "input", "value", Value::Str("v2".into()), 1));

    db
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn pull_simple_attrs() {
    let db = test_db();
    let pattern = parse_pull_pattern_edn(db.schema(), db.rschema(), "[:name :age]");
    let result = pull(&db, &pattern, 1).unwrap();
    if let PullResult::Map(entries) = &result {
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|(k, v)| *k == kw("name") && *v == scalar(Value::Str("Alice".into()))));
        assert!(entries.iter().any(|(k, v)| *k == kw("age") && *v == scalar(Value::Long(30))));
    } else {
        panic!("expected Map, got {:?}", result);
    }
}

#[test]
fn pull_wildcard() {
    let db = test_db();
    let pattern = parse_pull_pattern_edn(db.schema(), db.rschema(), "[*]");
    let result = pull(&db, &pattern, 2).unwrap();
    if let PullResult::Map(entries) = &result {
        // Should have :db/id, :name, :age, :parent, :bp/uuid
        assert!(entries.iter().any(|(k, _)| *k == kw_ns("db", "id")));
        assert!(entries.iter().any(|(k, _)| *k == kw("name")));
        assert!(entries.iter().any(|(k, _)| *k == kw("age")));
        assert!(entries.iter().any(|(k, _)| *k == kw("parent")));
        assert!(entries.iter().any(|(k, _)| *k == kw_ns("bp", "uuid")));
    } else {
        panic!("expected Map, got {:?}", result);
    }
}

#[test]
fn pull_db_id() {
    let db = test_db();
    let pattern = parse_pull_pattern_edn(db.schema(), db.rschema(), "[:db/id :name]");
    let result = pull(&db, &pattern, 1).unwrap();
    if let PullResult::Map(entries) = &result {
        assert!(entries.iter().any(|(k, v)| *k == kw_ns("db", "id") && *v == scalar(Value::Long(1))));
    } else {
        panic!("expected Map");
    }
}

#[test]
fn pull_multival() {
    let db = test_db();
    let pattern = parse_pull_pattern_edn(db.schema(), db.rschema(), "[:aka]");
    let result = pull(&db, &pattern, 1).unwrap();
    if let PullResult::Map(entries) = &result {
        let aka = entries.iter().find(|(k, _)| *k == kw("aka")).unwrap();
        if let PullResult::Vec(items) = &aka.1 {
            assert_eq!(items.len(), 2);
        } else {
            panic!("expected Vec for multival");
        }
    } else {
        panic!("expected Map");
    }
}

#[test]
fn pull_ref_no_pattern() {
    let db = test_db();
    let pattern = parse_pull_pattern_edn(db.schema(), db.rschema(), "[:parent]");
    let result = pull(&db, &pattern, 2).unwrap();
    if let PullResult::Map(entries) = &result {
        let parent = entries.iter().find(|(k, _)| *k == kw("parent")).unwrap();
        // Without nested pattern: should return {:db/id eid}
        if let PullResult::Map(inner) = &parent.1 {
            assert!(inner.iter().any(|(k, v)| *k == kw_ns("db", "id") && *v == scalar(Value::Long(1))));
        } else {
            panic!("expected Map for ref");
        }
    } else {
        panic!("expected Map");
    }
}

#[test]
fn pull_ref_with_nested_pattern() {
    let db = test_db();
    let pattern = parse_pull_pattern_edn(
        db.schema(), db.rschema(),
        "[:name {:parent [:name :age]}]"
    );
    let result = pull(&db, &pattern, 2).unwrap();
    if let PullResult::Map(entries) = &result {
        let parent = entries.iter().find(|(k, _)| *k == kw("parent")).unwrap();
        if let PullResult::Map(inner) = &parent.1 {
            assert!(inner.iter().any(|(k, v)| *k == kw("name") && *v == scalar(Value::Str("Alice".into()))));
            assert!(inner.iter().any(|(k, v)| *k == kw("age") && *v == scalar(Value::Long(30))));
        } else {
            panic!("expected Map for nested ref");
        }
    } else {
        panic!("expected Map");
    }
}

#[test]
fn pull_multival_ref() {
    let db = test_db();
    let pattern = parse_pull_pattern_edn(
        db.schema(), db.rschema(),
        "{:children [:name]}"
    );
    let result = pull(&db, &pattern, 1).unwrap();
    if let PullResult::Map(entries) = &result {
        let children = entries.iter().find(|(k, _)| *k == kw("children")).unwrap();
        if let PullResult::Vec(items) = &children.1 {
            assert_eq!(items.len(), 2);
            // Each should be a map with :name
            for item in items {
                if let PullResult::Map(child_entries) = item {
                    assert!(child_entries.iter().any(|(k, _)| *k == kw("name")));
                } else {
                    panic!("expected Map for child");
                }
            }
        } else {
            panic!("expected Vec for multival ref");
        }
    } else {
        panic!("expected Map");
    }
}

#[test]
fn pull_reverse_ref() {
    let db = test_db();
    let pattern = parse_pull_pattern_edn(
        db.schema(), db.rschema(),
        "[{:_parent [:name]}]"
    );
    let result = pull(&db, &pattern, 1).unwrap();
    if let PullResult::Map(entries) = &result {
        // :_parent should give us Bob and Carol
        let rev = entries.iter().find(|(k, _)| {
            *k == Attr::Keyword { ns: None, name: "_parent".into() }
        }).unwrap();
        if let PullResult::Vec(items) = &rev.1 {
            assert_eq!(items.len(), 2);
        } else {
            panic!("expected Vec for reverse ref");
        }
    } else {
        panic!("expected Map");
    }
}

#[test]
fn pull_nonexistent_entity() {
    let db = test_db();
    let pattern = parse_pull_pattern_edn(db.schema(), db.rschema(), "[:name]");
    assert!(pull(&db, &pattern, 999).is_none());
}

#[test]
fn pull_component_wildcard_auto_expand() {
    let db = test_db();
    let pattern = parse_pull_pattern_edn(
        db.schema(), db.rschema(),
        "[{:ws/inputs [*]}]"
    );
    let result = pull(&db, &pattern, 10).unwrap();
    if let PullResult::Map(entries) = &result {
        let inputs = entries.iter().find(|(k, _)| *k == kw_ns("ws", "inputs")).unwrap();
        if let PullResult::Vec(items) = &inputs.1 {
            assert_eq!(items.len(), 2);
            for item in items {
                if let PullResult::Map(ie) = item {
                    // Should have :db/id and :input/value
                    assert!(ie.iter().any(|(k, _)| *k == kw_ns("db", "id")));
                    assert!(ie.iter().any(|(k, _)| *k == kw_ns("input", "value")));
                } else {
                    panic!("expected Map");
                }
            }
        } else {
            panic!("expected Vec");
        }
    } else {
        panic!("expected Map");
    }
}

#[test]
fn pull_lookup_ref() {
    let db = test_db();
    let pattern = parse_pull_pattern_edn(db.schema(), db.rschema(), "[:name]");
    // Resolve lookup ref manually (as the caller would)
    let eid = db.resolve_lookup_ref(
        &kw_ns("bp", "uuid"),
        &Value::Str("uuid-alice".into()),
    ).unwrap();
    assert_eq!(eid, 1);
    let result = pull(&db, &pattern, eid).unwrap();
    if let PullResult::Map(entries) = &result {
        assert!(entries.iter().any(|(k, v)| *k == kw("name") && *v == scalar(Value::Str("Alice".into()))));
    } else {
        panic!("expected Map");
    }
}

#[test]
fn pull_missing_attr_omitted() {
    let db = test_db();
    // Entity 3 (Carol) has no :age
    let pattern = parse_pull_pattern_edn(db.schema(), db.rschema(), "[:name :age]");
    let result = pull(&db, &pattern, 3).unwrap();
    if let PullResult::Map(entries) = &result {
        assert_eq!(entries.len(), 1);
        assert!(entries.iter().any(|(k, _)| *k == kw("name")));
        assert!(!entries.iter().any(|(k, _)| *k == kw("age")));
    } else {
        panic!("expected Map");
    }
}

#[test]
fn pull_deep_nesting() {
    let db = test_db();
    // Pull worksheet with nested inputs
    let pattern = parse_pull_pattern_edn(
        db.schema(), db.rschema(),
        "[{:ws/inputs [:input/value]}]"
    );
    let result = pull(&db, &pattern, 10).unwrap();
    if let PullResult::Map(entries) = &result {
        let inputs = entries.iter().find(|(k, _)| *k == kw_ns("ws", "inputs")).unwrap();
        if let PullResult::Vec(items) = &inputs.1 {
            assert_eq!(items.len(), 2);
            let values: Vec<&Value> = items.iter().filter_map(|item| {
                if let PullResult::Map(ie) = item {
                    ie.iter().find_map(|(k, v)| {
                        if *k == kw_ns("input", "value") {
                            if let PullResult::Scalar(val) = v { Some(val) } else { None }
                        } else {
                            None
                        }
                    })
                } else {
                    None
                }
            }).collect();
            assert_eq!(values.len(), 2);
        } else {
            panic!("expected Vec");
        }
    } else {
        panic!("expected Map");
    }
}

#[test]
fn pull_bare_keyword_component_auto_expands() {
    let db = test_db();
    // Bare keyword :ws/inputs (component ref) — no map spec.
    // Should auto-expand like [{:ws/inputs [*]}], not return {:db/id N}.
    let pattern = parse_pull_pattern_edn(
        db.schema(), db.rschema(),
        "[:ws/inputs]"
    );
    let result = pull(&db, &pattern, 10).unwrap();
    if let PullResult::Map(entries) = &result {
        let inputs = entries.iter()
            .find(|(k, _)| *k == kw_ns("ws", "inputs"))
            .unwrap();
        if let PullResult::Vec(items) = &inputs.1 {
            assert_eq!(items.len(), 2);
            for item in items {
                if let PullResult::Map(ie) = item {
                    assert!(ie.iter().any(|(k, _)| *k == kw_ns("input", "value")),
                        "component auto-expand should include :input/value");
                } else {
                    panic!("expected Map for component entity");
                }
            }
        } else {
            panic!("expected Vec for multi-valued component");
        }
    } else {
        panic!("expected Map");
    }
}

#[test]
fn pull_bare_keyword_component_matches_explicit_map_spec() {
    let db = test_db();
    // Bare keyword should produce same result as explicit map spec
    let bare = parse_pull_pattern_edn(db.schema(), db.rschema(), "[:ws/inputs]");
    let explicit = parse_pull_pattern_edn(db.schema(), db.rschema(), "[{:ws/inputs [*]}]");

    let bare_result = pull(&db, &bare, 10).unwrap();
    let explicit_result = pull(&db, &explicit, 10).unwrap();

    assert_eq!(bare_result, explicit_result,
        "bare keyword component pull should match explicit map spec with [*]");
}

#[test]
fn pull_bare_keyword_non_component_ref_returns_db_id_only() {
    let db = test_db();
    // :children is a ref but NOT a component — bare keyword should return {:db/id N}
    let pattern = parse_pull_pattern_edn(db.schema(), db.rschema(), "[:children]");
    let result = pull(&db, &pattern, 1).unwrap();
    if let PullResult::Map(entries) = &result {
        let children = entries.iter()
            .find(|(k, _)| *k == kw("children"))
            .unwrap();
        if let PullResult::Vec(items) = &children.1 {
            for item in items {
                if let PullResult::Map(ie) = item {
                    // Non-component ref: should only have :db/id
                    assert!(ie.iter().any(|(k, _)| *k == kw_ns("db", "id")));
                    assert!(!ie.iter().any(|(k, _)| *k == kw("name")),
                        "non-component bare keyword should NOT recursively pull :name");
                } else {
                    panic!("expected Map");
                }
            }
        } else {
            panic!("expected Vec");
        }
    } else {
        panic!("expected Map");
    }
}
