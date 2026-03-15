//! Query resolution: pattern lookup + relational algebra over WasmDataScript indexes.
//!
//! Provides `search_internal` (returns `Vec<Datom>` without JS marshalling) and
//! `lookup_pattern` (resolves a WHERE pattern against the DB into a Relation).
//! Also exposes WASM entry points for batch pattern resolution and full
//! clause-level query execution.

use std::collections::HashMap;
use std::cmp::Ordering;

use persistent_sorted_set::comparator::value_cmp;
use persistent_sorted_set::datom::{Attr, Datom, Value};
use persistent_sorted_set::relation::{
    self, Relation, Tuple, Var,
    collapse_rels, project,
};
use persistent_sorted_set::set::PersistentSortedSet;

use crate::wasm_datascript::WasmDataScript;

/// DataScript constants.
const E0: i64 = 0;
const TX0: i64 = 0x20000000;
const EMAX: i64 = 0x7FFFFFFF;
const TXMAX: i64 = 0x7FFFFFFF;

// ---------------------------------------------------------------------------
// Internal search (no JS marshalling)
// ---------------------------------------------------------------------------

/// Collect datoms from a PSS slice into a Vec.
fn slice_to_vec(pss: &PersistentSortedSet, from: &Datom, to: &Datom) -> Vec<Datom> {
    match pss.slice(Some(from), Some(to)) {
        Some(seq) => seq.to_vec(),
        None => Vec::new(),
    }
}

/// Search the WasmDataScript indexes by pattern `[e, a, v, tx]`.
/// None/Nil = wildcard. Returns `Vec<Datom>` entirely in Rust — no WASM boundary.
///
/// Mirrors the 16-case dispatch tree in `wasm_datascript.rs:search` and
/// CLJS `db.cljc:-search`.
pub fn search_internal(
    db: &WasmDataScript,
    e: Option<i64>,
    a: Option<&Attr>,
    v: Option<&Value>,
    tx: Option<i64>,
) -> Vec<Datom> {
    let eavt = db.eavt();
    let aevt = db.aevt();
    let avet = db.avet();

    match (e, a, v, tx) {
        (Some(e), Some(a), Some(v), Some(tx)) => {
            let d = Datom::new(e, Some(a.clone()), v.clone(), tx);
            slice_to_vec(eavt, &d, &d)
        }
        (Some(e), Some(a), Some(v), None) => {
            let from = Datom::new(e, Some(a.clone()), v.clone(), TX0);
            let to = Datom::new(e, Some(a.clone()), v.clone(), TXMAX);
            slice_to_vec(eavt, &from, &to)
        }
        (Some(e), Some(a), None, None) => {
            let from = Datom::new(e, Some(a.clone()), Value::Nil, TX0);
            let to = Datom::new(e, Some(a.clone()), Value::Nil, TXMAX);
            slice_to_vec(eavt, &from, &to)
        }
        (Some(e), Some(a), None, Some(tx)) => {
            let from = Datom::new(e, Some(a.clone()), Value::Nil, TX0);
            let to = Datom::new(e, Some(a.clone()), Value::Nil, TXMAX);
            slice_to_vec(eavt, &from, &to)
                .into_iter()
                .filter(|d| d.tx_id() == tx)
                .collect()
        }
        (Some(e), None, None, None) => {
            let from = Datom::new(e, None, Value::Nil, TX0);
            let to = Datom::new(e, None, Value::Nil, TXMAX);
            slice_to_vec(eavt, &from, &to)
        }
        (Some(e), None, Some(v), None) => {
            let from = Datom::new(e, None, Value::Nil, TX0);
            let to = Datom::new(e, None, Value::Nil, TXMAX);
            slice_to_vec(eavt, &from, &to)
                .into_iter()
                .filter(|d| value_cmp(&d.v, v) == Ordering::Equal)
                .collect()
        }
        (Some(e), None, None, Some(tx)) => {
            let from = Datom::new(e, None, Value::Nil, TX0);
            let to = Datom::new(e, None, Value::Nil, TXMAX);
            slice_to_vec(eavt, &from, &to)
                .into_iter()
                .filter(|d| d.tx_id() == tx)
                .collect()
        }
        (Some(e), None, Some(v), Some(tx)) => {
            let from = Datom::new(e, None, Value::Nil, TX0);
            let to = Datom::new(e, None, Value::Nil, TXMAX);
            slice_to_vec(eavt, &from, &to)
                .into_iter()
                .filter(|d| value_cmp(&d.v, v) == Ordering::Equal && d.tx_id() == tx)
                .collect()
        }
        (None, Some(a), Some(v), None) => {
            if db.is_indexed_pub(a) {
                let from = Datom::new(E0, Some(a.clone()), v.clone(), TX0);
                let to = Datom::new(EMAX, Some(a.clone()), v.clone(), TXMAX);
                slice_to_vec(avet, &from, &to)
            } else {
                let from = Datom::new(E0, Some(a.clone()), Value::Nil, TX0);
                let to = Datom::new(EMAX, Some(a.clone()), Value::Nil, TXMAX);
                slice_to_vec(aevt, &from, &to)
                    .into_iter()
                    .filter(|d| value_cmp(&d.v, v) == Ordering::Equal)
                    .collect()
            }
        }
        (None, Some(a), Some(v), Some(tx)) => {
            if db.is_indexed_pub(a) {
                let from = Datom::new(E0, Some(a.clone()), v.clone(), TX0);
                let to = Datom::new(EMAX, Some(a.clone()), v.clone(), TXMAX);
                slice_to_vec(avet, &from, &to)
                    .into_iter()
                    .filter(|d| d.tx_id() == tx)
                    .collect()
            } else {
                let from = Datom::new(E0, Some(a.clone()), Value::Nil, TX0);
                let to = Datom::new(EMAX, Some(a.clone()), Value::Nil, TXMAX);
                slice_to_vec(aevt, &from, &to)
                    .into_iter()
                    .filter(|d| value_cmp(&d.v, v) == Ordering::Equal && d.tx_id() == tx)
                    .collect()
            }
        }
        (None, Some(a), None, None) => {
            let from = Datom::new(E0, Some(a.clone()), Value::Nil, TX0);
            let to = Datom::new(EMAX, Some(a.clone()), Value::Nil, TXMAX);
            slice_to_vec(aevt, &from, &to)
        }
        (None, Some(a), None, Some(tx)) => {
            let from = Datom::new(E0, Some(a.clone()), Value::Nil, TX0);
            let to = Datom::new(EMAX, Some(a.clone()), Value::Nil, TXMAX);
            slice_to_vec(aevt, &from, &to)
                .into_iter()
                .filter(|d| d.tx_id() == tx)
                .collect()
        }
        (None, None, Some(v), None) => {
            eavt.to_vec()
                .into_iter()
                .filter(|d| value_cmp(&d.v, v) == Ordering::Equal)
                .collect()
        }
        (None, None, Some(v), Some(tx)) => {
            eavt.to_vec()
                .into_iter()
                .filter(|d| value_cmp(&d.v, v) == Ordering::Equal && d.tx_id() == tx)
                .collect()
        }
        (None, None, None, Some(tx)) => {
            eavt.to_vec()
                .into_iter()
                .filter(|d| d.tx_id() == tx)
                .collect()
        }
        (None, None, None, None) => {
            eavt.to_vec()
        }
    }
}

// ---------------------------------------------------------------------------
// Pattern → Relation
// ---------------------------------------------------------------------------

/// A pattern element: either a concrete value or a variable to bind.
#[derive(Clone, Debug)]
pub enum PatternEl {
    /// A bound variable name (e.g. "?e", "?name")
    Var(String),
    /// A concrete value to match
    Const(Value),
    /// Wildcard — match anything, don't bind
    Blank,
}

/// Resolve a single WHERE pattern clause against the DB, producing a Relation.
///
/// Mirrors CLJS `lookup-pattern-db` (query.cljc:417-427):
/// 1. Extract constants from the pattern for the search call
/// 2. Call `search_internal` with concrete values (vars → None)
/// 3. Build a Relation with free vars mapped to datom fields
pub fn lookup_pattern(db: &WasmDataScript, pattern: &[PatternEl; 4]) -> Relation {
    // Extract search arguments: Const → Some, Var/Blank → None
    let e = match &pattern[0] {
        PatternEl::Const(Value::Long(n)) => Some(*n),
        PatternEl::Const(Value::Ref(n)) => Some(*n),
        _ => None,
    };
    let a = match &pattern[1] {
        PatternEl::Const(Value::Keyword(attr)) => Some(attr.clone()),
        PatternEl::Const(Value::Str(s)) => Some(Attr::Keyword {
            ns: None,
            name: s.clone(),
        }),
        _ => None,
    };
    let v = match &pattern[2] {
        PatternEl::Const(val) => Some(val),
        _ => None,
    };
    let tx = match &pattern[3] {
        PatternEl::Const(Value::Long(n)) => Some(*n),
        _ => None,
    };

    let datoms = search_internal(db, e, a.as_ref(), v, tx);

    // Build attrs map: only free variables get columns
    // Datom fields are mapped as: [0]=e, [1]=a, [2]=v, [3]=tx
    let mut attrs = HashMap::new();
    let mut col = 0usize;
    for el in pattern.iter() {
        if let PatternEl::Var(name) = el {
            attrs.insert(name.clone(), col);
            col += 1;
        }
    }

    // Build tuples
    let tuples: Vec<Tuple> = datoms
        .into_iter()
        .map(|d| {
            let mut tuple = Vec::with_capacity(col);
            for (i, el) in pattern.iter().enumerate() {
                if let PatternEl::Var(_) = el {
                    tuple.push(match i {
                        0 => Value::Long(d.e),
                        1 => match &d.a {
                            Some(attr) => Value::Keyword(attr.clone()),
                            None => Value::Nil,
                        },
                        2 => d.v.clone(),
                        3 => Value::Long(d.tx_id()),
                        _ => unreachable!(),
                    });
                }
            }
            tuple
        })
        .collect();

    Relation::new(attrs, tuples)
}

/// Resolve multiple pattern clauses against the DB, joining results.
/// Returns a single joined Relation.
///
/// This is the key optimization: all pattern lookups + joins happen in Rust
/// without any WASM boundary crossings.
pub fn resolve_patterns(db: &WasmDataScript, patterns: &[[PatternEl; 4]]) -> Relation {
    let mut rels: Vec<Relation> = Vec::new();

    for pattern in patterns {
        let rel = lookup_pattern(db, pattern);
        // Short-circuit: if any pattern returns empty, the whole join is empty
        if rel.is_empty() && !rel.attrs.is_empty() {
            // Collect all vars from all patterns
            let all_vars: Vec<Var> = patterns
                .iter()
                .flat_map(|p| {
                    p.iter().filter_map(|el| {
                        if let PatternEl::Var(name) = el {
                            Some(name.clone())
                        } else {
                            None
                        }
                    })
                })
                .collect();
            return Relation::empty(&all_vars);
        }
        rels = collapse_rels(&rels, rel);
    }

    if rels.is_empty() {
        relation::unit_rel()
    } else if rels.len() == 1 {
        rels.into_iter().next().unwrap()
    } else {
        // Multiple disjoint rels — cartesian product
        rels.into_iter().reduce(|a, b| relation::prod_rel(&a, &b)).unwrap()
    }
}

/// Collect result tuples for the given find variables from a resolved relation.
pub fn collect_results(rel: &Relation, find_vars: &[Var]) -> Vec<Tuple> {
    let projected = project(rel, find_vars);
    projected.tuples
}
