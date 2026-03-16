//! Tests for the Rust transact implementation.
//!
//! All tests use `DataScriptDB` (native, no WASM needed).

use persistent_sorted_set::datom::Value;
use persistent_sorted_set::db::{DataScriptDB, TX0};
use persistent_sorted_set::schema::{
    AttrSchema, Cardinality, Schema, Unique, ValueType, kw,
};
use persistent_sorted_set::transact::{
    EntityRef, TempId, TransactError, TxEntity, TxValue, parse_tx_edn, transact,
};

fn test_schema() -> Schema {
    let mut schema = Schema::default();
    schema.attrs.insert(kw("name"), AttrSchema { index: true, ..Default::default() });
    schema.attrs.insert(kw("age"), AttrSchema::default());
    schema.attrs.insert(kw("email"), AttrSchema {
        unique: Some(Unique::Identity),
        ..Default::default()
    });
    schema.attrs.insert(kw("code"), AttrSchema {
        unique: Some(Unique::Value),
        ..Default::default()
    });
    schema.attrs.insert(kw("aka"), AttrSchema {
        cardinality: Cardinality::Many,
        ..Default::default()
    });
    schema.attrs.insert(kw("parent"), AttrSchema {
        value_type: Some(ValueType::Ref),
        ..Default::default()
    });
    schema.attrs.insert(kw("child"), AttrSchema {
        value_type: Some(ValueType::Ref),
        is_component: true,
        ..Default::default()
    });
    schema
}

fn empty_db() -> DataScriptDB {
    DataScriptDB::empty(test_schema())
}

// ---------------------------------------------------------------------------
// 1. Basic :db/add
// ---------------------------------------------------------------------------

#[test]
fn basic_add() {
    let mut db = empty_db();
    let tx = vec![TxEntity::Add {
        e: EntityRef::Eid(1),
        a: kw("name"),
        v: TxValue::Val(Value::Str("Alice".into())),
    }];
    let report = transact(&mut db, tx).unwrap();

    assert_eq!(report.tx_data.len(), 1);
    assert_eq!(report.tx_data[0].e, 1);
    assert_eq!(report.tx_data[0].v, Value::Str("Alice".into()));
    assert_eq!(report.tx_data[0].tx, TX0 + 1);
    assert_eq!(db.count(), 1);
}

#[test]
fn basic_add_multiple() {
    let mut db = empty_db();
    let tx = vec![
        TxEntity::Add {
            e: EntityRef::Eid(1),
            a: kw("name"),
            v: TxValue::Val(Value::Str("Alice".into())),
        },
        TxEntity::Add {
            e: EntityRef::Eid(1),
            a: kw("age"),
            v: TxValue::Val(Value::Long(30)),
        },
    ];
    let report = transact(&mut db, tx).unwrap();

    assert_eq!(report.tx_data.len(), 2);
    assert_eq!(db.count(), 2);
}

// ---------------------------------------------------------------------------
// 2. Tempid resolution
// ---------------------------------------------------------------------------

#[test]
fn tempid_neg_number() {
    let mut db = empty_db();
    let tx = vec![
        TxEntity::Add {
            e: EntityRef::TempId(TempId::Neg(-1)),
            a: kw("name"),
            v: TxValue::Val(Value::Str("Alice".into())),
        },
    ];
    let report = transact(&mut db, tx).unwrap();

    assert_eq!(report.tx_data.len(), 1);
    let allocated_eid = report.tempids[&TempId::Neg(-1)];
    assert!(allocated_eid > 0);
    assert_eq!(report.tx_data[0].e, allocated_eid);
}

#[test]
fn tempid_string() {
    let mut db = empty_db();
    let tx = vec![
        TxEntity::Add {
            e: EntityRef::TempId(TempId::Str("user-1".into())),
            a: kw("name"),
            v: TxValue::Val(Value::Str("Alice".into())),
        },
    ];
    let report = transact(&mut db, tx).unwrap();

    let allocated_eid = report.tempids[&TempId::Str("user-1".into())];
    assert!(allocated_eid > 0);
}

// ---------------------------------------------------------------------------
// 3. Shared tempid across ops
// ---------------------------------------------------------------------------

#[test]
fn shared_tempid() {
    let mut db = empty_db();
    let tx = vec![
        TxEntity::Add {
            e: EntityRef::TempId(TempId::Neg(-1)),
            a: kw("name"),
            v: TxValue::Val(Value::Str("Alice".into())),
        },
        TxEntity::Add {
            e: EntityRef::TempId(TempId::Neg(-1)),
            a: kw("age"),
            v: TxValue::Val(Value::Long(30)),
        },
    ];
    let report = transact(&mut db, tx).unwrap();

    assert_eq!(report.tx_data.len(), 2);
    assert_eq!(report.tx_data[0].e, report.tx_data[1].e);
    assert_eq!(db.count(), 2);
}

// ---------------------------------------------------------------------------
// 4. Cardinality one replacement
// ---------------------------------------------------------------------------

#[test]
fn cardinality_one_replace() {
    let mut db = empty_db();
    // First transact: set name
    let tx1 = vec![TxEntity::Add {
        e: EntityRef::Eid(1),
        a: kw("name"),
        v: TxValue::Val(Value::Str("Alice".into())),
    }];
    transact(&mut db, tx1).unwrap();
    assert_eq!(db.count(), 1);

    // Second transact: replace name
    let tx2 = vec![TxEntity::Add {
        e: EntityRef::Eid(1),
        a: kw("name"),
        v: TxValue::Val(Value::Str("Bob".into())),
    }];
    let report = transact(&mut db, tx2).unwrap();

    // Should have retract + add
    assert_eq!(report.tx_data.len(), 2);
    assert!(report.tx_data[0].tx < 0, "first datom should be retraction");
    assert!(report.tx_data[1].tx > 0, "second datom should be assertion");
    assert_eq!(db.count(), 1);

    // Verify the value was replaced
    let results = db.search(Some(1), Some(&kw("name")), None, None);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].v, Value::Str("Bob".into()));
}

#[test]
fn cardinality_one_same_value_skip() {
    let mut db = empty_db();
    let tx1 = vec![TxEntity::Add {
        e: EntityRef::Eid(1),
        a: kw("name"),
        v: TxValue::Val(Value::Str("Alice".into())),
    }];
    transact(&mut db, tx1).unwrap();

    // Same value again — should be redundant (no new datoms)
    let tx2 = vec![TxEntity::Add {
        e: EntityRef::Eid(1),
        a: kw("name"),
        v: TxValue::Val(Value::Str("Alice".into())),
    }];
    let report = transact(&mut db, tx2).unwrap();
    assert_eq!(report.tx_data.len(), 0, "redundant add should produce no datoms");
    assert_eq!(db.count(), 1);
}

// ---------------------------------------------------------------------------
// 5. Cardinality many
// ---------------------------------------------------------------------------

#[test]
fn cardinality_many_add() {
    let mut db = empty_db();
    let tx = vec![
        TxEntity::Add {
            e: EntityRef::Eid(1),
            a: kw("aka"),
            v: TxValue::Val(Value::Str("A".into())),
        },
        TxEntity::Add {
            e: EntityRef::Eid(1),
            a: kw("aka"),
            v: TxValue::Val(Value::Str("B".into())),
        },
    ];
    transact(&mut db, tx).unwrap();
    assert_eq!(db.count(), 2, "both values should be present");
}

#[test]
fn cardinality_many_skip_duplicate() {
    let mut db = empty_db();
    let tx1 = vec![TxEntity::Add {
        e: EntityRef::Eid(1),
        a: kw("aka"),
        v: TxValue::Val(Value::Str("A".into())),
    }];
    transact(&mut db, tx1).unwrap();

    let tx2 = vec![TxEntity::Add {
        e: EntityRef::Eid(1),
        a: kw("aka"),
        v: TxValue::Val(Value::Str("A".into())),
    }];
    let report = transact(&mut db, tx2).unwrap();
    assert_eq!(report.tx_data.len(), 0, "duplicate multival should be skipped");
    assert_eq!(db.count(), 1);
}

// ---------------------------------------------------------------------------
// 6. Retract with value
// ---------------------------------------------------------------------------

#[test]
fn retract_with_value() {
    let mut db = empty_db();
    let tx1 = vec![TxEntity::Add {
        e: EntityRef::Eid(1),
        a: kw("name"),
        v: TxValue::Val(Value::Str("Alice".into())),
    }];
    transact(&mut db, tx1).unwrap();
    assert_eq!(db.count(), 1);

    let tx2 = vec![TxEntity::Retract {
        e: EntityRef::Eid(1),
        a: kw("name"),
        v: TxValue::Val(Value::Str("Alice".into())),
    }];
    let report = transact(&mut db, tx2).unwrap();
    assert_eq!(report.tx_data.len(), 1);
    assert!(report.tx_data[0].tx < 0);
    assert_eq!(db.count(), 0);
}

// ---------------------------------------------------------------------------
// 7. Retract nonexistent (no-op)
// ---------------------------------------------------------------------------

#[test]
fn retract_nonexistent() {
    let mut db = empty_db();
    let tx = vec![TxEntity::Retract {
        e: EntityRef::Eid(999),
        a: kw("name"),
        v: TxValue::Val(Value::Str("Ghost".into())),
    }];
    let report = transact(&mut db, tx).unwrap();
    assert_eq!(report.tx_data.len(), 0);
}

// ---------------------------------------------------------------------------
// 8. :db.fn/retractAttribute
// ---------------------------------------------------------------------------

#[test]
fn retract_attribute() {
    let mut db = empty_db();
    let tx1 = vec![
        TxEntity::Add { e: EntityRef::Eid(1), a: kw("name"), v: TxValue::Val(Value::Str("Alice".into())) },
        TxEntity::Add { e: EntityRef::Eid(1), a: kw("age"), v: TxValue::Val(Value::Long(30)) },
        TxEntity::Add { e: EntityRef::Eid(1), a: kw("aka"), v: TxValue::Val(Value::Str("A".into())) },
        TxEntity::Add { e: EntityRef::Eid(1), a: kw("aka"), v: TxValue::Val(Value::Str("B".into())) },
    ];
    transact(&mut db, tx1).unwrap();
    assert_eq!(db.count(), 4);

    let tx2 = vec![TxEntity::RetractAttribute {
        e: EntityRef::Eid(1),
        a: kw("aka"),
    }];
    let report = transact(&mut db, tx2).unwrap();
    assert_eq!(report.tx_data.len(), 2, "both :aka values should be retracted");
    assert_eq!(db.count(), 2, "name and age remain");
}

// ---------------------------------------------------------------------------
// 9. :db.fn/retractEntity + incoming refs + components
// ---------------------------------------------------------------------------

#[test]
fn retract_entity() {
    let mut db = empty_db();
    let tx1 = vec![
        TxEntity::Add { e: EntityRef::Eid(1), a: kw("name"), v: TxValue::Val(Value::Str("Alice".into())) },
        TxEntity::Add { e: EntityRef::Eid(1), a: kw("age"), v: TxValue::Val(Value::Long(30)) },
        TxEntity::Add { e: EntityRef::Eid(2), a: kw("name"), v: TxValue::Val(Value::Str("Bob".into())) },
        TxEntity::Add { e: EntityRef::Eid(2), a: kw("parent"), v: TxValue::Val(Value::Ref(1)) },
    ];
    transact(&mut db, tx1).unwrap();
    assert_eq!(db.count(), 4);

    let tx2 = vec![TxEntity::RetractEntity {
        e: EntityRef::Eid(1),
    }];
    let report = transact(&mut db, tx2).unwrap();

    // Entity 1 had: name, age (2 datoms)
    // Incoming ref: entity 2's :parent → 1 (1 datom)
    // Total retracted: 3
    assert_eq!(report.tx_data.len(), 3);
    // Only Bob's :name should remain
    assert_eq!(db.count(), 1);
    let remaining = db.search(None, None, None, None);
    assert_eq!(remaining[0].e, 2);
    assert_eq!(remaining[0].v, Value::Str("Bob".into()));
}

#[test]
fn retract_entity_cascades_components() {
    let mut db = empty_db();
    // Entity 1 has component child → entity 2
    let tx1 = vec![
        TxEntity::Add { e: EntityRef::Eid(1), a: kw("name"), v: TxValue::Val(Value::Str("Parent".into())) },
        TxEntity::Add { e: EntityRef::Eid(1), a: kw("child"), v: TxValue::Val(Value::Ref(2)) },
        TxEntity::Add { e: EntityRef::Eid(2), a: kw("name"), v: TxValue::Val(Value::Str("Child".into())) },
    ];
    transact(&mut db, tx1).unwrap();
    assert_eq!(db.count(), 3);

    let tx2 = vec![TxEntity::RetractEntity {
        e: EntityRef::Eid(1),
    }];
    transact(&mut db, tx2).unwrap();

    // Entity 1 retracted (name, child) + entity 2 cascaded (name)
    assert_eq!(db.count(), 0, "component entity should be cascade-retracted");
}

// ---------------------------------------------------------------------------
// 10. Unique identity upsert
// ---------------------------------------------------------------------------

#[test]
fn unique_identity_upsert() {
    let mut db = empty_db();
    // Create entity with unique email
    let tx1 = vec![
        TxEntity::Add { e: EntityRef::Eid(1), a: kw("email"), v: TxValue::Val(Value::Str("a@b.com".into())) },
        TxEntity::Add { e: EntityRef::Eid(1), a: kw("name"), v: TxValue::Val(Value::Str("Alice".into())) },
    ];
    transact(&mut db, tx1).unwrap();

    // Upsert: tempid with same email should resolve to entity 1
    let tx2 = vec![TxEntity::MapEntity {
        id: Some(EntityRef::TempId(TempId::Neg(-1))),
        attrs: vec![
            (kw("email"), TxValue::Val(Value::Str("a@b.com".into()))),
            (kw("age"), TxValue::Val(Value::Long(30))),
        ],
    }];
    let report = transact(&mut db, tx2).unwrap();

    assert_eq!(report.tempids[&TempId::Neg(-1)], 1, "tempid should resolve to existing entity");
    // Entity 1 now has name, email, and age
    let e1_datoms = db.search(Some(1), None, None, None);
    assert_eq!(e1_datoms.len(), 3);
}

// ---------------------------------------------------------------------------
// 11. Unique value violation
// ---------------------------------------------------------------------------

#[test]
fn unique_value_violation() {
    let mut db = empty_db();
    let tx1 = vec![
        TxEntity::Add { e: EntityRef::Eid(1), a: kw("code"), v: TxValue::Val(Value::Str("ABC".into())) },
    ];
    transact(&mut db, tx1).unwrap();

    let tx2 = vec![
        TxEntity::Add { e: EntityRef::Eid(2), a: kw("code"), v: TxValue::Val(Value::Str("ABC".into())) },
    ];
    let result = transact(&mut db, tx2);
    assert!(result.is_err(), "should fail with unique violation");
    if let Err(TransactError::UniqueConflict { existing_eid, new_eid, .. }) = result {
        assert_eq!(existing_eid, 1);
        assert_eq!(new_eid, 2);
    }
}

// ---------------------------------------------------------------------------
// 12. Map entity expansion
// ---------------------------------------------------------------------------

#[test]
fn map_entity_expansion() {
    let mut db = empty_db();
    let tx = vec![TxEntity::MapEntity {
        id: Some(EntityRef::TempId(TempId::Neg(-1))),
        attrs: vec![
            (kw("name"), TxValue::Val(Value::Str("Alice".into()))),
            (kw("age"), TxValue::Val(Value::Long(30))),
        ],
    }];
    let report = transact(&mut db, tx).unwrap();

    assert_eq!(report.tx_data.len(), 2);
    let eid = report.tempids[&TempId::Neg(-1)];
    assert_eq!(report.tx_data[0].e, eid);
    assert_eq!(report.tx_data[1].e, eid);
    assert_eq!(db.count(), 2);
}

// ---------------------------------------------------------------------------
// 13. Nested map entity
// ---------------------------------------------------------------------------

#[test]
fn nested_map_entity() {
    let mut db = empty_db();
    let tx = vec![TxEntity::MapEntity {
        id: Some(EntityRef::TempId(TempId::Neg(-1))),
        attrs: vec![
            (kw("name"), TxValue::Val(Value::Str("Parent".into()))),
            (kw("child"), TxValue::Nested(Box::new(TxEntity::MapEntity {
                id: None,
                attrs: vec![
                    (kw("name"), TxValue::Val(Value::Str("Child".into()))),
                ],
            }))),
        ],
    }];
    let report = transact(&mut db, tx).unwrap();

    // Parent: name + child ref = 2 datoms
    // Child: name = 1 datom
    assert_eq!(db.count(), 3);

    let parent_eid = report.tempids[&TempId::Neg(-1)];
    let parent_datoms = db.search(Some(parent_eid), None, None, None);
    assert_eq!(parent_datoms.len(), 2);
}

// ---------------------------------------------------------------------------
// 14. Reverse ref
// ---------------------------------------------------------------------------

#[test]
fn reverse_ref() {
    let mut db = empty_db();
    let tx = vec![TxEntity::MapEntity {
        id: Some(EntityRef::Eid(2)),
        attrs: vec![
            (kw("name"), TxValue::Val(Value::Str("Child".into()))),
            (kw("_parent"), TxValue::Val(Value::Long(1))),
        ],
    }];
    // Note: _parent means [:db/add 1 :parent 2] (entity 1 has parent → entity 2)
    // Wait, reverse ref _parent on entity 2 means: "entity <value> has :parent → entity 2"
    // So [:db/add 1 :parent 2]
    let _report = transact(&mut db, tx).unwrap();

    // Entity 2 should have :name
    let e2 = db.search(Some(2), Some(&kw("name")), None, None);
    assert_eq!(e2.len(), 1);

    // Entity 1 should have :parent → 2
    let e1 = db.search(Some(1), Some(&kw("parent")), None, None);
    assert_eq!(e1.len(), 1);
    assert_eq!(e1[0].v, Value::Ref(2));
}

// ---------------------------------------------------------------------------
// 15. Lookup ref in entity position
// ---------------------------------------------------------------------------

#[test]
fn lookup_ref_entity() {
    let mut db = empty_db();
    // Setup: entity 1 with unique email
    let tx1 = vec![
        TxEntity::Add { e: EntityRef::Eid(1), a: kw("email"), v: TxValue::Val(Value::Str("a@b.com".into())) },
    ];
    transact(&mut db, tx1).unwrap();

    // Use lookup ref to add name to the same entity
    let tx2 = vec![TxEntity::Add {
        e: EntityRef::LookupRef(kw("email"), Value::Str("a@b.com".into())),
        a: kw("name"),
        v: TxValue::Val(Value::Str("Alice".into())),
    }];
    let report = transact(&mut db, tx2).unwrap();

    assert_eq!(report.tx_data[0].e, 1);
    let e1 = db.search(Some(1), None, None, None);
    assert_eq!(e1.len(), 2); // email + name
}

// ---------------------------------------------------------------------------
// 16. :db/current-tx
// ---------------------------------------------------------------------------

#[test]
fn current_tx_in_entity() {
    let mut db = empty_db();
    let tx = vec![TxEntity::Add {
        e: EntityRef::CurrentTx,
        a: kw("name"),
        v: TxValue::Val(Value::Str("tx-metadata".into())),
    }];
    let report = transact(&mut db, tx).unwrap();

    assert_eq!(report.tx_data[0].e, report.current_tx);
}

// ---------------------------------------------------------------------------
// 17. tx-report structure
// ---------------------------------------------------------------------------

#[test]
fn tx_report_structure() {
    let mut db = empty_db();
    let initial_count = db.count();
    let initial_max_tx = db.max_tx;

    let tx = vec![
        TxEntity::Add { e: EntityRef::Eid(1), a: kw("name"), v: TxValue::Val(Value::Str("Alice".into())) },
    ];
    let report = transact(&mut db, tx).unwrap();

    assert_eq!(report.current_tx, initial_max_tx + 1);
    assert_eq!(report.tx_data.len(), 1);
    assert_eq!(db.count(), initial_count + 1);
    assert_eq!(db.max_tx, initial_max_tx + 1);
}

// ---------------------------------------------------------------------------
// 18. Sequential transactions
// ---------------------------------------------------------------------------

#[test]
fn sequential_transactions() {
    let mut db = empty_db();

    let tx1 = vec![TxEntity::Add {
        e: EntityRef::TempId(TempId::Neg(-1)),
        a: kw("name"),
        v: TxValue::Val(Value::Str("Alice".into())),
    }];
    let r1 = transact(&mut db, tx1).unwrap();
    let tx1_id = r1.current_tx;
    let alice_eid = r1.tempids[&TempId::Neg(-1)];

    let tx2 = vec![TxEntity::Add {
        e: EntityRef::TempId(TempId::Neg(-1)),
        a: kw("name"),
        v: TxValue::Val(Value::Str("Bob".into())),
    }];
    let r2 = transact(&mut db, tx2).unwrap();
    let tx2_id = r2.current_tx;
    let bob_eid = r2.tempids[&TempId::Neg(-1)];

    assert_eq!(tx2_id, tx1_id + 1, "tx ids should increment");
    assert_ne!(alice_eid, bob_eid, "different tempids in different tx should get different eids");
    assert_eq!(db.count(), 2);
}

// ---------------------------------------------------------------------------
// 19. EDN parsing integration
// ---------------------------------------------------------------------------

#[test]
fn edn_integration_basic() {
    let mut db = empty_db();
    let entities = parse_tx_edn(
        "[[:db/add 1 :name \"Alice\"] [:db/add 1 :age 30]]",
        &db.rschema,
    ).unwrap();
    let report = transact(&mut db, entities).unwrap();

    assert_eq!(report.tx_data.len(), 2);
    assert_eq!(db.count(), 2);
}

#[test]
fn edn_integration_map_entity() {
    let mut db = empty_db();
    let entities = parse_tx_edn(
        "[{:db/id -1 :name \"Alice\" :age 30}]",
        &db.rschema,
    ).unwrap();
    let report = transact(&mut db, entities).unwrap();

    assert_eq!(report.tx_data.len(), 2);
    assert_eq!(db.count(), 2);
}

#[test]
fn edn_integration_retract() {
    let mut db = empty_db();
    // Add first
    let tx1 = parse_tx_edn(
        "[[:db/add 1 :name \"Alice\"]]",
        &db.rschema,
    ).unwrap();
    transact(&mut db, tx1).unwrap();

    // Retract
    let tx2 = parse_tx_edn(
        "[[:db/retract 1 :name \"Alice\"]]",
        &db.rschema,
    ).unwrap();
    transact(&mut db, tx2).unwrap();
    assert_eq!(db.count(), 0);
}

#[test]
fn edn_integration_retract_entity() {
    let mut db = empty_db();
    let tx1 = parse_tx_edn(
        "[[:db/add 1 :name \"Alice\"] [:db/add 1 :age 30]]",
        &db.rschema,
    ).unwrap();
    transact(&mut db, tx1).unwrap();
    assert_eq!(db.count(), 2);

    let tx2 = parse_tx_edn(
        "[[:db.fn/retractEntity 1]]",
        &db.rschema,
    ).unwrap();
    transact(&mut db, tx2).unwrap();
    assert_eq!(db.count(), 0);
}

#[test]
fn edn_retract_entity_alias() {
    // [:db/retractEntity id] is the alias form (vs [:db.fn/retractEntity id])
    let mut db = empty_db();
    let tx1 = parse_tx_edn(
        "[[:db/add 1 :name \"Alice\"] [:db/add 1 :age 30]]",
        &db.rschema,
    ).unwrap();
    transact(&mut db, tx1).unwrap();
    assert_eq!(db.count(), 2);

    let tx2 = parse_tx_edn(
        "[[:db/retractEntity 1]]",
        &db.rschema,
    ).unwrap();
    transact(&mut db, tx2).unwrap();
    assert_eq!(db.count(), 0);
}

#[test]
fn edn_retract_entity_with_refs_and_components() {
    // Full scenario: entity with incoming refs and component children
    let mut db = empty_db();
    let tx1 = parse_tx_edn(
        concat!(
            "[[:db/add 1 :name \"Parent\"]",
            " [:db/add 1 :child 2]",      // component ref → 2
            " [:db/add 2 :name \"Child\"]",
            " [:db/add 3 :name \"Friend\"]",
            " [:db/add 3 :parent 1]]",    // non-component ref → 1
        ),
        &db.rschema,
    ).unwrap();
    transact(&mut db, tx1).unwrap();
    assert_eq!(db.count(), 5);

    // Retract entity 1:
    //   - forward datoms: [1 :name "Parent"], [1 :child 2]
    //   - incoming refs: [3 :parent 1]
    //   - component cascade: entity 2 ([2 :name "Child"])
    let tx2 = parse_tx_edn(
        "[[:db/retractEntity 1]]",
        &db.rschema,
    ).unwrap();
    transact(&mut db, tx2).unwrap();

    // Only [3 :name "Friend"] should remain
    assert_eq!(db.count(), 1);
    let remaining = db.search(None, None, None, None);
    assert_eq!(remaining[0].e, 3);
    assert_eq!(remaining[0].v, Value::Str("Friend".into()));
}

#[test]
fn edn_integration_mixed() {
    let mut db = empty_db();
    let entities = parse_tx_edn(
        "[[:db/add 1 :name \"Alice\"] {:db/id -1 :name \"Bob\" :age 25} [:db/add 3 :name \"Carol\"]]",
        &db.rschema,
    ).unwrap();
    let report = transact(&mut db, entities).unwrap();

    // Alice: 1 datom, Bob: 2 datoms, Carol: 1 datom = 4 total
    assert_eq!(report.tx_data.len(), 4);
    assert_eq!(db.count(), 4);
}

// ---------------------------------------------------------------------------
// Ref tempid in value position
// ---------------------------------------------------------------------------

#[test]
fn ref_tempid_in_value() {
    let mut db = empty_db();
    let tx = vec![
        TxEntity::Add {
            e: EntityRef::TempId(TempId::Neg(-1)),
            a: kw("name"),
            v: TxValue::Val(Value::Str("Child".into())),
        },
        TxEntity::Add {
            e: EntityRef::TempId(TempId::Neg(-2)),
            a: kw("name"),
            v: TxValue::Val(Value::Str("Parent".into())),
        },
        TxEntity::Add {
            e: EntityRef::TempId(TempId::Neg(-1)),
            a: kw("parent"),
            v: TxValue::TempId(TempId::Neg(-2)),
        },
    ];
    let report = transact(&mut db, tx).unwrap();

    let child_eid = report.tempids[&TempId::Neg(-1)];
    let parent_eid = report.tempids[&TempId::Neg(-2)];

    // Child should have :parent → parent
    let child_parent = db.search(Some(child_eid), Some(&kw("parent")), None, None);
    assert_eq!(child_parent.len(), 1);
    assert_eq!(child_parent[0].v, Value::Ref(parent_eid));
}
