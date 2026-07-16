//! Transaction processing for DataScript databases.
//!
//! Mirrors the CLJS `transact-tx-data-impl` from `db.cljc:1699-1944`.
//! Processes transaction data (`:db/add`, `:db/retract`, map entities, etc.)
//! and returns a `TxReport` with the resulting datoms and tempid mappings.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fmt;

use edn::Value as EdnValue;

use crate::datom::{Attr, Datom, Value};
use crate::legacy_edn::{attr_from_edn_keyword, parse_edn, value_from_edn};
use crate::schema::{ReverseSchema, Schema};

/// Abstraction over DataScriptDB (native test) and WasmDataScript (WASM).
pub trait TransactableDB {
    fn search_eav(&self, e: i64, a: &Attr, v: &Value) -> Option<Datom>;
    fn search_ea(&self, e: i64, a: &Attr) -> Vec<Datom>;
    fn search_e(&self, e: i64) -> Vec<Datom>;
    fn search_av(&self, a: &Attr, v: &Value) -> Vec<Datom>;
    fn search_a_refs(&self, a: &Attr, v_ref: i64) -> Vec<Datom>;
    fn apply_datom(&mut self, datom: Datom);
    fn schema(&self) -> &Schema;
    fn rschema(&self) -> &ReverseSchema;
    fn max_eid(&self) -> i64;
    fn set_max_eid(&mut self, eid: i64);
    fn max_tx(&self) -> i64;
    fn set_max_tx(&mut self, tx: i64);
}

// ---------------------------------------------------------------------------
// Transaction data types
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TempId {
    Neg(i64),
    Str(String),
}

impl fmt::Display for TempId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TempId::Neg(n) => write!(f, "{}", n),
            TempId::Str(s) => write!(f, "\"{}\"", s),
        }
    }
}

#[derive(Clone, Debug)]
pub enum EntityRef {
    Eid(i64),
    TempId(TempId),
    LookupRef(Attr, Value),
    CurrentTx,
}

#[derive(Clone, Debug)]
pub enum TxValue {
    Val(Value),
    TempId(TempId),
    LookupRef(Attr, Value),
    CurrentTx,
    Nested(Box<TxEntity>),
}

#[derive(Clone, Debug)]
pub enum TxEntity {
    Add { e: EntityRef, a: Attr, v: TxValue },
    Retract { e: EntityRef, a: Attr, v: TxValue },
    RetractAttribute { e: EntityRef, a: Attr },
    RetractEntity { e: EntityRef },
    MapEntity { id: Option<EntityRef>, attrs: Vec<(Attr, TxValue)> },
}

// ---------------------------------------------------------------------------
// TxReport and errors
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct TxReport {
    pub tx_data: Vec<Datom>,
    pub tempids: HashMap<TempId, i64>,
    pub current_tx: i64,
}

#[derive(Debug)]
pub enum TransactError {
    UniqueConflict {
        attr: Attr,
        value: Value,
        existing_eid: i64,
        new_eid: i64,
    },
    TempidNotAllowed {
        op: String,
        tempid: TempId,
    },
    EntityNotFound {
        lookup_ref: (Attr, Value),
    },
    ParseError {
        message: String,
    },
}

impl fmt::Display for TransactError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransactError::UniqueConflict { attr, existing_eid, new_eid, .. } => {
                write!(f, "Unique conflict on {:?}: entity {} vs {}", attr, existing_eid, new_eid)
            }
            TransactError::TempidNotAllowed { op, tempid } => {
                write!(f, "Tempid {} not allowed in {}", tempid, op)
            }
            TransactError::EntityNotFound { lookup_ref } => {
                write!(f, "Entity not found for lookup ref {:?}", lookup_ref)
            }
            TransactError::ParseError { message } => {
                write!(f, "Parse error: {}", message)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// EDN tx-data parser
// ---------------------------------------------------------------------------

/// Parse EDN tx-data string into a Vec<TxEntity>.
pub fn parse_tx_edn(s: &str, rschema: &ReverseSchema) -> Result<Vec<TxEntity>, TransactError> {
    let edn = parse_edn(s);
    parse_tx_entities(&edn, rschema)
}

fn parse_tx_entities(edn: &EdnValue, rschema: &ReverseSchema) -> Result<Vec<TxEntity>, TransactError> {
    match edn {
        EdnValue::Vector(items) => {
            items.iter().map(|item| parse_tx_entity(item, rschema)).collect()
        }
        EdnValue::List(items) => {
            items.iter().map(|item| parse_tx_entity(item, rschema)).collect()
        }
        _ => Err(TransactError::ParseError {
            message: format!("Expected vector of tx-data, got {:?}", edn),
        }),
    }
}

fn parse_tx_entity(edn: &EdnValue, rschema: &ReverseSchema) -> Result<TxEntity, TransactError> {
    match edn {
        EdnValue::Vector(items) => parse_vector_entity(items, rschema),
        EdnValue::List(items) => {
            let v: Vec<_> = items.iter().cloned().collect();
            parse_vector_entity(&v, rschema)
        }
        EdnValue::Map(pairs) => parse_map_entity(pairs, rschema),
        _ => Err(TransactError::ParseError {
            message: format!("Expected vector or map tx-data, got {:?}", edn),
        }),
    }
}

fn parse_vector_entity(items: &[EdnValue], rschema: &ReverseSchema) -> Result<TxEntity, TransactError> {
    if items.is_empty() {
        return Err(TransactError::ParseError {
            message: "Empty vector in tx-data".into(),
        });
    }

    let op = match &items[0] {
        EdnValue::Keyword(kw) => kw.as_str(),
        _ => return Err(TransactError::ParseError {
            message: format!("Expected keyword op, got {:?}", items[0]),
        }),
    };

    match op {
        ":db/add" | "db/add" => {
            if items.len() < 4 {
                return Err(TransactError::ParseError {
                    message: format!(":db/add requires [op e a v], got {} elements", items.len()),
                });
            }
            let e = parse_entity_ref(&items[1])?;
            let a = parse_attr(&items[2])?;
            let is_ref = rschema.is_ref(&a);
            let v = parse_tx_value(&items[3], is_ref, rschema)?;
            Ok(TxEntity::Add { e, a, v })
        }
        ":db/retract" | "db/retract" => {
            if items.len() < 3 {
                return Err(TransactError::ParseError {
                    message: ":db/retract requires at least [op e a]".into(),
                });
            }
            let e = parse_entity_ref(&items[1])?;
            let a = parse_attr(&items[2])?;
            if items.len() >= 4 {
                let is_ref = rschema.is_ref(&a);
                let v = parse_tx_value(&items[3], is_ref, rschema)?;
                Ok(TxEntity::Retract { e, a, v })
            } else {
                Ok(TxEntity::RetractAttribute { e, a })
            }
        }
        ":db.fn/retractAttribute" | "db.fn/retractAttribute" => {
            if items.len() < 3 {
                return Err(TransactError::ParseError {
                    message: ":db.fn/retractAttribute requires [op e a]".into(),
                });
            }
            let e = parse_entity_ref(&items[1])?;
            let a = parse_attr(&items[2])?;
            Ok(TxEntity::RetractAttribute { e, a })
        }
        ":db.fn/retractEntity" | "db.fn/retractEntity"
        | ":db/retractEntity" | "db/retractEntity" => {
            if items.len() < 2 {
                return Err(TransactError::ParseError {
                    message: ":db.fn/retractEntity requires [op e]".into(),
                });
            }
            let e = parse_entity_ref(&items[1])?;
            Ok(TxEntity::RetractEntity { e })
        }
        _ => Err(TransactError::ParseError {
            message: format!("Unknown tx op: {}", op),
        }),
    }
}

fn parse_map_entity(
    pairs: &BTreeMap<EdnValue, EdnValue>,
    rschema: &ReverseSchema,
) -> Result<TxEntity, TransactError> {
    let mut id: Option<EntityRef> = None;
    let mut attrs = Vec::new();

    for (k, v) in pairs {
        let key_str = match k {
            EdnValue::Keyword(kw) => kw.clone(),
            _ => return Err(TransactError::ParseError {
                message: format!("Expected keyword key in map entity, got {:?}", k),
            }),
        };

        if key_str == ":db/id" || key_str == "db/id" {
            id = Some(parse_entity_ref(v)?);
        } else {
            let a = attr_from_edn_keyword(&key_str);
            let (is_reverse, _) = check_reverse_ref(&a);

            // Reverse refs (:ns/_attr): the value is an ENTITY REF — an eid,
            // tempid, or lookup ref like [:worksheet/uuid "..."]. The reverse
            // attr itself is not in the schema, so parse_tx_value would
            // stringify a lookup-ref vector; parse as entity ref(s) instead.
            if is_reverse {
                match parse_entity_ref(v) {
                    Ok(eref) => attrs.push((a, entity_ref_to_tx_value(eref))),
                    Err(err) => match v {
                        // Multiple referring entities: {:a/_b [ref1 ref2]}
                        EdnValue::Vector(items) | EdnValue::List(items) => {
                            for item in items {
                                let eref = parse_entity_ref(item)?;
                                attrs.push((a.clone(), entity_ref_to_tx_value(eref)));
                            }
                        }
                        _ => return Err(err),
                    },
                }
                continue;
            }

            let is_ref = rschema.is_ref(&a);
            let is_many = rschema.is_multival(&a);

            // For cardinality-many attrs, explode vector/set values into
            // individual (attr, val) pairs — matching DataScript semantics.
            if is_many {
                match v {
                    EdnValue::Vector(items) | EdnValue::List(items) => {
                        for item in items {
                            let tv = parse_tx_value(item, is_ref, rschema)?;
                            attrs.push((a.clone(), tv));
                        }
                    }
                    EdnValue::Set(items) => {
                        for item in items {
                            let tv = parse_tx_value(item, is_ref, rschema)?;
                            attrs.push((a.clone(), tv));
                        }
                    }
                    _ => {
                        let tv = parse_tx_value(v, is_ref, rschema)?;
                        attrs.push((a, tv));
                    }
                }
            } else {
                let tv = parse_tx_value(v, is_ref, rschema)?;
                attrs.push((a, tv));
            }
        }
    }

    Ok(TxEntity::MapEntity { id, attrs })
}

/// Encode an entity ref as a TxValue (for reverse-ref attr values, where the
/// "value" is really the referring entity).
fn entity_ref_to_tx_value(eref: EntityRef) -> TxValue {
    match eref {
        EntityRef::Eid(n) => TxValue::Val(Value::Ref(n)),
        EntityRef::TempId(tid) => TxValue::TempId(tid),
        EntityRef::LookupRef(a, v) => TxValue::LookupRef(a, v),
        EntityRef::CurrentTx => TxValue::CurrentTx,
    }
}

fn parse_entity_ref(edn: &EdnValue) -> Result<EntityRef, TransactError> {
    match edn {
        EdnValue::Integer(n) => {
            if *n < 0 {
                Ok(EntityRef::TempId(TempId::Neg(*n)))
            } else {
                Ok(EntityRef::Eid(*n))
            }
        }
        EdnValue::String(s) => Ok(EntityRef::TempId(TempId::Str(s.clone()))),
        EdnValue::Keyword(kw) if kw == ":db/current-tx" || kw == "db/current-tx" => {
            Ok(EntityRef::CurrentTx)
        }
        EdnValue::Vector(items) if items.len() == 2 => {
            let a = parse_attr(&items[0])?;
            let v = value_from_edn(&items[1], false);
            Ok(EntityRef::LookupRef(a, v))
        }
        _ => Err(TransactError::ParseError {
            message: format!("Invalid entity ref: {:?}", edn),
        }),
    }
}

fn parse_attr(edn: &EdnValue) -> Result<Attr, TransactError> {
    match edn {
        EdnValue::Keyword(kw) => Ok(attr_from_edn_keyword(kw)),
        _ => Err(TransactError::ParseError {
            message: format!("Expected keyword attribute, got {:?}", edn),
        }),
    }
}

fn parse_tx_value(
    edn: &EdnValue,
    is_ref: bool,
    rschema: &ReverseSchema,
) -> Result<TxValue, TransactError> {
    match edn {
        EdnValue::Integer(n) if *n < 0 && is_ref => {
            Ok(TxValue::TempId(TempId::Neg(*n)))
        }
        EdnValue::String(s) if is_ref => {
            // Could be a tempid string or a real string value.
            // In DataScript, strings in ref position are tempids.
            // But we'll treat them as tempids only if they look like tempids.
            // Actually, DataScript treats strings as values, not tempids, in value position.
            // Only :db/id position uses string tempids.
            Ok(TxValue::Val(Value::Str(s.clone())))
        }
        EdnValue::Keyword(kw) if (kw == ":db/current-tx" || kw == "db/current-tx") && is_ref => {
            Ok(TxValue::CurrentTx)
        }
        EdnValue::Vector(items) if items.len() == 2 && is_ref => {
            // Lookup ref in value position for ref attrs
            if let EdnValue::Keyword(_) = &items[0] {
                let a = parse_attr(&items[0])?;
                let v = value_from_edn(&items[1], false);
                Ok(TxValue::LookupRef(a, v))
            } else {
                Ok(TxValue::Val(value_from_edn(edn, is_ref)))
            }
        }
        EdnValue::Map(pairs) if is_ref => {
            // Nested map entity in ref value position
            let entity = parse_map_entity(pairs, rschema)?;
            Ok(TxValue::Nested(Box::new(entity)))
        }
        _ => Ok(TxValue::Val(value_from_edn(edn, is_ref))),
    }
}

// ---------------------------------------------------------------------------
// Core transact logic
// ---------------------------------------------------------------------------

struct TransactState {
    tx_data: Vec<Datom>,
    tempids: HashMap<TempId, i64>,
    current_tx: i64,
    next_auto_eid: i64,
}

impl TransactState {
    fn new(max_tx: i64, max_eid: i64) -> Self {
        Self {
            tx_data: Vec::new(),
            tempids: HashMap::new(),
            current_tx: max_tx + 1,
            next_auto_eid: max_eid + 1,
        }
    }

    fn alloc_eid(&mut self) -> i64 {
        let eid = self.next_auto_eid;
        self.next_auto_eid += 1;
        eid
    }
}

/// Process tx-data against a database, returning a TxReport.
pub fn transact<DB: TransactableDB>(
    db: &mut DB,
    tx_data: Vec<TxEntity>,
) -> Result<TxReport, TransactError> {
    let mut state = TransactState::new(db.max_tx(), db.max_eid());
    let mut queue: VecDeque<TxEntity> = tx_data.into();

    // Assign auto-tempids to map entities without :db/id
    let mut queue2 = VecDeque::new();
    while let Some(entity) = queue.pop_front() {
        match entity {
            TxEntity::MapEntity { id: None, attrs } => {
                let tempid = TempId::Neg(-(state.next_auto_eid as i64 + 1000000));
                state.next_auto_eid += 0; // don't alloc yet, just use unique neg number
                queue2.push_back(TxEntity::MapEntity {
                    id: Some(EntityRef::TempId(tempid)),
                    attrs,
                });
            }
            other => queue2.push_back(other),
        }
    }
    queue = queue2;

    while let Some(entity) = queue.pop_front() {
        match entity {
            TxEntity::Add { e, a, v } => {
                process_add(db, &mut state, e, a, v, &mut queue)?;
            }
            TxEntity::Retract { e, a, v } => {
                process_retract(db, &mut state, e, a, v)?;
            }
            TxEntity::RetractAttribute { e, a } => {
                process_retract_attribute(db, &mut state, e, a, &mut queue)?;
            }
            TxEntity::RetractEntity { e } => {
                process_retract_entity(db, &mut state, e, &mut queue)?;
            }
            TxEntity::MapEntity { id, attrs } => {
                explode_map(db, &mut state, id, attrs, &mut queue)?;
            }
        }
    }

    // Finalize: increment max_tx
    db.set_max_tx(state.current_tx);

    // Update max_eid if we allocated any
    if state.next_auto_eid - 1 > db.max_eid() {
        db.set_max_eid(state.next_auto_eid - 1);
    }

    Ok(TxReport {
        tx_data: state.tx_data,
        tempids: state.tempids,
        current_tx: state.current_tx,
    })
}

// ---------------------------------------------------------------------------
// Entity ref resolution
// ---------------------------------------------------------------------------

fn resolve_entity_ref<DB: TransactableDB>(
    db: &DB,
    state: &mut TransactState,
    eref: EntityRef,
) -> Result<i64, TransactError> {
    match eref {
        EntityRef::Eid(n) => {
            // Track for correct tempid allocation
            if n >= state.next_auto_eid {
                state.next_auto_eid = n + 1;
            }
            Ok(n)
        }
        EntityRef::TempId(ref tid) => {
            if let Some(&eid) = state.tempids.get(tid) {
                Ok(eid)
            } else {
                let eid = state.alloc_eid();
                state.tempids.insert(tid.clone(), eid);
                Ok(eid)
            }
        }
        EntityRef::LookupRef(a, v) => {
            let datoms = db.search_av(&a, &v);
            if let Some(d) = datoms.first() {
                Ok(d.e)
            } else {
                Err(TransactError::EntityNotFound {
                    lookup_ref: (a, v),
                })
            }
        }
        EntityRef::CurrentTx => Ok(state.current_tx),
    }
}

fn resolve_tx_value<DB: TransactableDB>(
    db: &DB,
    state: &mut TransactState,
    a: &Attr,
    v: TxValue,
    queue: &mut VecDeque<TxEntity>,
) -> Result<Value, TransactError> {
    let is_ref = db.rschema().is_ref(a);
    match v {
        TxValue::Val(val) => Ok(val),
        TxValue::TempId(tid) => {
            let eid = if let Some(&existing) = state.tempids.get(&tid) {
                existing
            } else {
                let eid = state.alloc_eid();
                state.tempids.insert(tid, eid);
                eid
            };
            if is_ref {
                Ok(Value::Ref(eid))
            } else {
                Ok(Value::Long(eid))
            }
        }
        TxValue::LookupRef(la, lv) => {
            let datoms = db.search_av(&la, &lv);
            if let Some(d) = datoms.first() {
                if is_ref {
                    Ok(Value::Ref(d.e))
                } else {
                    Ok(Value::Long(d.e))
                }
            } else {
                Err(TransactError::EntityNotFound {
                    lookup_ref: (la, lv),
                })
            }
        }
        TxValue::CurrentTx => {
            if is_ref {
                Ok(Value::Ref(state.current_tx))
            } else {
                Ok(Value::Long(state.current_tx))
            }
        }
        TxValue::Nested(entity) => {
            // Allocate a new entity for the nested map, queue it, return ref
            let nested_eid = state.alloc_eid();
            let TxEntity::MapEntity { id: _, attrs } = *entity else {
                return Err(TransactError::ParseError {
                    message: "Nested value must be a map entity".into(),
                });
            };
            // Give the nested entity a concrete eid
            let tempid = TempId::Neg(-(nested_eid + 2000000));
            state.tempids.insert(tempid.clone(), nested_eid);
            queue.push_back(TxEntity::MapEntity {
                id: Some(EntityRef::Eid(nested_eid)),
                attrs,
            });
            if is_ref {
                Ok(Value::Ref(nested_eid))
            } else {
                Ok(Value::Long(nested_eid))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Process operations
// ---------------------------------------------------------------------------

/// Mirrors CLJS `transact-add` from `db.cljc:1606-1628`.
fn process_add<DB: TransactableDB>(
    db: &mut DB,
    state: &mut TransactState,
    e: EntityRef,
    a: Attr,
    v: TxValue,
    queue: &mut VecDeque<TxEntity>,
) -> Result<(), TransactError> {
    // Tempid in e position: check for upsert via unique/identity
    let eid = match &e {
        EntityRef::TempId(tid) => {
            // Check if this tempid is already allocated
            if let Some(&existing) = state.tempids.get(tid) {
                existing
            } else {
                // If the attr is unique/identity and we can find the value, upsert
                let upserted = if db.rschema().unique_identity.contains(&a) {
                    // We need to resolve the value first to check for upsert
                    let v_resolved = resolve_tx_value(db, state, &a, v.clone(), queue)?;
                    let datoms = db.search_av(&a, &v_resolved);
                    datoms.first().map(|d| d.e)
                } else {
                    None
                };

                if let Some(upserted_eid) = upserted {
                    state.tempids.insert(tid.clone(), upserted_eid);
                    upserted_eid
                } else {
                    let eid = state.alloc_eid();
                    state.tempids.insert(tid.clone(), eid);
                    eid
                }
            }
        }
        _ => resolve_entity_ref(db, state, e)?,
    };

    let v_resolved = resolve_tx_value(db, state, &a, v, queue)?;

    // Validate uniqueness
    validate_unique(db, eid, &a, &v_resolved)?;

    let is_multival = db.rschema().is_multival(&a);
    let new_datom = Datom::new(eid, Some(a.clone()), v_resolved.clone(), state.current_tx);

    if is_multival {
        // Cardinality many: check if exact [e a v] already exists
        if let Some(_existing) = db.search_eav(eid, &a, &v_resolved) {
            // Redundant — already exists, skip
            return Ok(());
        }
        // Add the new datom
        apply_and_record(db, state, new_datom);
    } else {
        // Cardinality one: check if [e a] already has a value
        let old_datoms = db.search_ea(eid, &a);
        if let Some(old) = old_datoms.first() {
            if old.v == v_resolved {
                // Same value — redundant, skip
                return Ok(());
            }
            // Different value — retract old, add new
            let retract_datom = Datom::new(
                eid,
                Some(a.clone()),
                old.v.clone(),
                -state.current_tx,
            );
            apply_and_record(db, state, retract_datom);
        }
        apply_and_record(db, state, new_datom);
    }

    Ok(())
}

fn process_retract<DB: TransactableDB>(
    db: &mut DB,
    state: &mut TransactState,
    e: EntityRef,
    a: Attr,
    v: TxValue,
    // No queue needed for simple retract
) -> Result<(), TransactError> {
    let eid = match resolve_entity_ref(db, state, e) {
        Ok(eid) => eid,
        Err(_) => return Ok(()), // Entity not found → no-op
    };
    let mut dummy_queue = VecDeque::new();
    let v_resolved = resolve_tx_value(db, state, &a, v, &mut dummy_queue)?;

    if let Some(_existing) = db.search_eav(eid, &a, &v_resolved) {
        let retract_datom = Datom::new(eid, Some(a), v_resolved, -state.current_tx);
        apply_and_record(db, state, retract_datom);
    }
    // Not found → no-op

    Ok(())
}

fn process_retract_attribute<DB: TransactableDB>(
    db: &mut DB,
    state: &mut TransactState,
    e: EntityRef,
    a: Attr,
    queue: &mut VecDeque<TxEntity>,
) -> Result<(), TransactError> {
    let eid = match resolve_entity_ref(db, state, e) {
        Ok(eid) => eid,
        Err(_) => return Ok(()),
    };

    let datoms = db.search_ea(eid, &a);
    for d in &datoms {
        let retract = Datom::new(d.e, d.a.clone(), d.v.clone(), -state.current_tx);
        apply_and_record(db, state, retract);
    }

    // Cascade component retractions
    retract_components(db, &datoms, queue);

    Ok(())
}

fn process_retract_entity<DB: TransactableDB>(
    db: &mut DB,
    state: &mut TransactState,
    e: EntityRef,
    queue: &mut VecDeque<TxEntity>,
) -> Result<(), TransactError> {
    let eid = match resolve_entity_ref(db, state, e) {
        Ok(eid) => eid,
        Err(_) => return Ok(()),
    };

    // Retract all forward datoms for this entity
    let e_datoms = db.search_e(eid);
    for d in &e_datoms {
        let retract = Datom::new(d.e, d.a.clone(), d.v.clone(), -state.current_tx);
        apply_and_record(db, state, retract);
    }

    // Retract all incoming refs (other entities pointing to this one)
    let ref_attrs: Vec<Attr> = db.rschema().ref_attrs.iter().cloned().collect();
    for ref_attr in &ref_attrs {
        let v_datoms = db.search_a_refs(ref_attr, eid);
        for d in &v_datoms {
            let retract = Datom::new(d.e, d.a.clone(), d.v.clone(), -state.current_tx);
            apply_and_record(db, state, retract);
        }
    }

    // Cascade component retractions
    retract_components(db, &e_datoms, queue);

    Ok(())
}

// ---------------------------------------------------------------------------
// Map entity explosion (mirrors CLJS `explode` from db.cljc:1583-1604)
// ---------------------------------------------------------------------------

fn explode_map<DB: TransactableDB>(
    db: &DB,
    state: &mut TransactState,
    id: Option<EntityRef>,
    attrs: Vec<(Attr, TxValue)>,
    queue: &mut VecDeque<TxEntity>,
) -> Result<(), TransactError> {
    let e = match id {
        Some(eref) => {
            // For upsert: check unique/identity attrs before allocating
            let mut upserted_eid: Option<i64> = None;

            if let EntityRef::TempId(ref tid) = eref {
                if state.tempids.get(tid).is_none() {
                    // Check all unique/identity attrs in this map for upsert
                    for (a, v) in &attrs {
                        if db.rschema().unique_identity.contains(a) {
                            if let TxValue::Val(ref val) = v {
                                let datoms = db.search_av(a, val);
                                if let Some(d) = datoms.first() {
                                    if let Some(prev) = upserted_eid {
                                        if prev != d.e {
                                            return Err(TransactError::UniqueConflict {
                                                attr: a.clone(),
                                                value: val.clone(),
                                                existing_eid: prev,
                                                new_eid: d.e,
                                            });
                                        }
                                    } else {
                                        upserted_eid = Some(d.e);
                                    }
                                }
                            }
                        }
                    }

                    if let Some(eid) = upserted_eid {
                        state.tempids.insert(tid.clone(), eid);
                    }
                }
            }

            eref
        }
        None => {
            // Auto-tempid — should have been assigned already
            return Err(TransactError::ParseError {
                message: "Map entity without :db/id should have been assigned a tempid".into(),
            });
        }
    };

    for (a, v) in attrs {
        // Check for reverse ref (attribute name starts with '_')
        let (is_reverse, actual_attr) = check_reverse_ref(&a);

        if is_reverse {
            // :_parent on entity X means [:db/add <v> :parent X]
            queue.push_back(TxEntity::Add {
                e: match &v {
                    TxValue::Val(Value::Long(n)) => EntityRef::Eid(*n),
                    TxValue::Val(Value::Ref(n)) => EntityRef::Eid(*n),
                    TxValue::TempId(tid) => EntityRef::TempId(tid.clone()),
                    TxValue::LookupRef(la, lv) => {
                        EntityRef::LookupRef(la.clone(), lv.clone())
                    }
                    TxValue::CurrentTx => EntityRef::CurrentTx,
                    _ => return Err(TransactError::ParseError {
                        message: format!("Reverse ref value must be an entity ref, got {:?}", v),
                    }),
                },
                a: actual_attr,
                v: match &e {
                    EntityRef::Eid(n) => TxValue::Val(Value::Ref(*n)),
                    EntityRef::TempId(tid) => TxValue::TempId(tid.clone()),
                    EntityRef::CurrentTx => TxValue::CurrentTx,
                    EntityRef::LookupRef(a, v) => TxValue::LookupRef(a.clone(), v.clone()),
                },
            });
        } else if db.rschema().is_multival(&a) {
            // Multival: if value is a collection, explode into multiple adds
            // For EDN parsed data, collections come in as Val
            // For now, each attr/val pair becomes one add
            queue.push_back(TxEntity::Add {
                e: e.clone(),
                a,
                v,
            });
        } else {
            queue.push_back(TxEntity::Add {
                e: e.clone(),
                a,
                v,
            });
        }
    }

    Ok(())
}

/// Check if an attribute is a reverse ref (name starts with '_').
fn check_reverse_ref(attr: &Attr) -> (bool, Attr) {
    match attr {
        Attr::Keyword { ns, name } => {
            if let Some(stripped) = name.strip_prefix('_') {
                (true, Attr::Keyword {
                    ns: ns.clone(),
                    name: stripped.to_string(),
                })
            } else {
                (false, attr.clone())
            }
        }
        Attr::Str(s) => {
            if let Some(stripped) = s.strip_prefix('_') {
                (true, Attr::Str(stripped.to_string()))
            } else {
                (false, attr.clone())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn validate_unique<DB: TransactableDB>(
    db: &DB,
    eid: i64,
    a: &Attr,
    v: &Value,
) -> Result<(), TransactError> {
    if db.rschema().unique_attrs.contains(a) {
        let datoms = db.search_av(a, v);
        if let Some(existing) = datoms.first() {
            if existing.e != eid {
                return Err(TransactError::UniqueConflict {
                    attr: a.clone(),
                    value: v.clone(),
                    existing_eid: existing.e,
                    new_eid: eid,
                });
            }
        }
    }
    Ok(())
}

fn apply_and_record<DB: TransactableDB>(
    db: &mut DB,
    state: &mut TransactState,
    datom: Datom,
) {
    // Track max eid for correct tempid allocation
    if datom.tx > 0 && datom.e >= state.next_auto_eid {
        state.next_auto_eid = datom.e + 1;
    }
    db.apply_datom(datom.clone());
    state.tx_data.push(datom);
}

fn retract_components<DB: TransactableDB>(
    db: &DB,
    datoms: &[Datom],
    queue: &mut VecDeque<TxEntity>,
) {
    for d in datoms {
        if let Some(ref a) = d.a {
            if db.rschema().is_component(a) {
                if let Value::Ref(ref_eid) = d.v {
                    queue.push_back(TxEntity::RetractEntity {
                        e: EntityRef::Eid(ref_eid),
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_edn_db_add() {
        let rs = ReverseSchema::default();
        let entities = parse_tx_edn(
            "[[:db/add 1 :name \"Alice\"]]",
            &rs,
        ).unwrap();
        assert_eq!(entities.len(), 1);
        match &entities[0] {
            TxEntity::Add { e: EntityRef::Eid(1), .. } => {}
            other => panic!("Expected Add with eid 1, got {:?}", other),
        }
    }

    #[test]
    fn parse_edn_tempid() {
        let rs = ReverseSchema::default();
        let entities = parse_tx_edn(
            "[[:db/add -1 :name \"Alice\"]]",
            &rs,
        ).unwrap();
        match &entities[0] {
            TxEntity::Add { e: EntityRef::TempId(TempId::Neg(-1)), .. } => {}
            other => panic!("Expected Add with tempid -1, got {:?}", other),
        }
    }

    #[test]
    fn parse_edn_map_entity() {
        let rs = ReverseSchema::default();
        let entities = parse_tx_edn(
            "[{:db/id -1 :name \"Alice\" :age 30}]",
            &rs,
        ).unwrap();
        match &entities[0] {
            TxEntity::MapEntity { id: Some(EntityRef::TempId(TempId::Neg(-1))), attrs } => {
                assert_eq!(attrs.len(), 2);
            }
            other => panic!("Expected MapEntity with tempid -1, got {:?}", other),
        }
    }

    #[test]
    fn parse_edn_retract_entity() {
        let rs = ReverseSchema::default();
        let entities = parse_tx_edn(
            "[[:db.fn/retractEntity 3]]",
            &rs,
        ).unwrap();
        match &entities[0] {
            TxEntity::RetractEntity { e: EntityRef::Eid(3) } => {}
            other => panic!("Expected RetractEntity 3, got {:?}", other),
        }
    }

    #[test]
    fn parse_edn_retract_attribute() {
        let rs = ReverseSchema::default();
        let entities = parse_tx_edn(
            "[[:db.fn/retractAttribute 1 :name]]",
            &rs,
        ).unwrap();
        match &entities[0] {
            TxEntity::RetractAttribute { e: EntityRef::Eid(1), .. } => {}
            other => panic!("Expected RetractAttribute, got {:?}", other),
        }
    }

    #[test]
    fn parse_edn_current_tx() {
        let rs = ReverseSchema::default();
        let entities = parse_tx_edn(
            "[[:db/add :db/current-tx :name \"tx-name\"]]",
            &rs,
        ).unwrap();
        match &entities[0] {
            TxEntity::Add { e: EntityRef::CurrentTx, .. } => {}
            other => panic!("Expected Add with CurrentTx, got {:?}", other),
        }
    }

    #[test]
    fn parse_edn_multiple_ops() {
        let rs = ReverseSchema::default();
        let entities = parse_tx_edn(
            "[[:db/add 1 :name \"Alice\"] [:db/retract 2 :name \"Bob\"] {:db/id -1 :age 30}]",
            &rs,
        ).unwrap();
        assert_eq!(entities.len(), 3);
    }
}
