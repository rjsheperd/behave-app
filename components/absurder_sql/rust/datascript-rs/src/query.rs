//! Query resolution: pattern lookup + relational algebra over WasmDataScript indexes.
//!
//! Provides `search_internal` (returns `Vec<Datom>` without JS marshalling),
//! `PatternResolver` impl for `WasmDataScript`, and full clause-level query
//! execution with rule support.

use std::collections::HashMap;
use std::cmp::Ordering;

use persistent_sorted_set::comparator::value_cmp;
use persistent_sorted_set::datom::{Attr, Datom, Value};
use persistent_sorted_set::pull::PullSource;
use persistent_sorted_set::relation::{
    self, Clause, PatternEl, PatternResolver, Relation, Rules, Tuple, Var,
    collapse_rels, project, resolve_query, resolve_query_with_initial,
};
use persistent_sorted_set::schema::{ReverseSchema, Schema};
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
            slice_to_vec(eavt, &Datom::min_for_ea(e, a), &Datom::max_for_ea(e, a))
        }
        (Some(e), Some(a), None, Some(tx)) => {
            slice_to_vec(eavt, &Datom::min_for_ea(e, a), &Datom::max_for_ea(e, a))
                .into_iter()
                .filter(|d| d.tx_id() == tx)
                .collect()
        }
        (Some(e), None, None, None) => {
            slice_to_vec(eavt, &Datom::min_for_e(e), &Datom::max_for_e(e))
        }
        (Some(e), None, Some(v), None) => {
            slice_to_vec(eavt, &Datom::min_for_e(e), &Datom::max_for_e(e))
                .into_iter()
                .filter(|d| value_cmp(&d.v, v) == Ordering::Equal)
                .collect()
        }
        (Some(e), None, None, Some(tx)) => {
            slice_to_vec(eavt, &Datom::min_for_e(e), &Datom::max_for_e(e))
                .into_iter()
                .filter(|d| d.tx_id() == tx)
                .collect()
        }
        (Some(e), None, Some(v), Some(tx)) => {
            slice_to_vec(eavt, &Datom::min_for_e(e), &Datom::max_for_e(e))
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
                let from = Datom::new(E0, Some(a.clone()), Value::min_sentinel(), TX0);
                let to = Datom::new(EMAX, Some(a.clone()), Value::max_sentinel(), TXMAX);
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
                let from = Datom::new(E0, Some(a.clone()), Value::min_sentinel(), TX0);
                let to = Datom::new(EMAX, Some(a.clone()), Value::max_sentinel(), TXMAX);
                slice_to_vec(aevt, &from, &to)
                    .into_iter()
                    .filter(|d| value_cmp(&d.v, v) == Ordering::Equal && d.tx_id() == tx)
                    .collect()
            }
        }
        (None, Some(a), None, None) => {
            let from = Datom::new(E0, Some(a.clone()), Value::min_sentinel(), TX0);
            let to = Datom::new(EMAX, Some(a.clone()), Value::max_sentinel(), TXMAX);
            slice_to_vec(aevt, &from, &to)
        }
        (None, Some(a), None, Some(tx)) => {
            let from = Datom::new(E0, Some(a.clone()), Value::min_sentinel(), TX0);
            let to = Datom::new(EMAX, Some(a.clone()), Value::max_sentinel(), TXMAX);
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
// PatternResolver for WasmDataScript
// ---------------------------------------------------------------------------

impl PatternResolver for WasmDataScript {
    fn resolve_pattern(&self, pattern: &[PatternEl; 4]) -> Relation {
        let e = match &pattern[0] {
            PatternEl::Const(Value::Long(n)) | PatternEl::Const(Value::Ref(n)) => Some(*n),
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
        // For the value position: if the attribute is ref-typed and the constant
        // is Long(n), search with Ref(n) instead (DataScript stores refs as Ref).
        let v = match &pattern[2] {
            PatternEl::Const(Value::Long(n))
                if a.as_ref().map_or(false, |a| self.is_indexed_pub(a)) =>
            {
                // Try Ref first for ref-type attrs; is_indexed_pub covers refs
                // since ref attrs are implicitly indexed.
                Some(Value::Ref(*n))
            }
            PatternEl::Const(val) => Some(val.clone()),
            _ => None,
        };
        let v_ref = v.as_ref();
        let tx = match &pattern[3] {
            PatternEl::Const(Value::Long(n)) => Some(*n),
            _ => None,
        };

        let mut datoms = search_internal(self, e, a.as_ref(), v_ref, tx);
        // If Ref search yielded nothing and we converted Long→Ref, try with the original Long
        if datoms.is_empty() {
            if let PatternEl::Const(Value::Long(n)) = &pattern[2] {
                let orig = Value::Long(*n);
                datoms = search_internal(self, e, a.as_ref(), Some(&orig), tx);
            }
        }

        // Build attrs map: only free variables get columns (dedup by name)
        let mut attrs = HashMap::new();
        let mut col = 0usize;
        for el in pattern.iter() {
            if let PatternEl::Var(name) = el {
                if !attrs.contains_key(name) {
                    attrs.insert(name.clone(), col);
                    col += 1;
                }
            }
        }

        let tuples: Vec<Tuple> = datoms
            .into_iter()
            .map(|d| {
                let mut tuple = Vec::with_capacity(col);
                let mut seen = std::collections::HashSet::new();
                for (i, el) in pattern.iter().enumerate() {
                    if let PatternEl::Var(name) = el {
                        if seen.insert(name.clone()) {
                            tuple.push(match i {
                                0 => Value::Long(d.e),
                                1 => match &d.a {
                                    Some(attr) => Value::Keyword(attr.clone()),
                                    None => Value::Nil,
                                },
                                2 => match &d.v {
                                    // Normalize Ref(n) → Long(n) so entity references
                                    // and entity IDs are interchangeable in joins.
                                    Value::Ref(n) => Value::Long(*n),
                                    other => other.clone(),
                                },
                                3 => Value::Long(d.tx_id()),
                                _ => unreachable!(),
                            });
                        }
                    }
                }
                tuple
            })
            .collect();

        Relation::new(attrs, tuples)
    }
}

// ---------------------------------------------------------------------------
// Pattern-only query (backwards compatible)
// ---------------------------------------------------------------------------

/// Resolve multiple pattern clauses against the DB, joining results.
/// Returns a single joined Relation.
///
/// This is the key optimization: all pattern lookups + joins happen in Rust
/// without any WASM boundary crossings.
pub fn resolve_patterns(db: &WasmDataScript, patterns: &[[PatternEl; 4]]) -> Relation {
    let mut rels: Vec<Relation> = Vec::new();

    for pattern in patterns {
        let rel = db.resolve_pattern(pattern);
        // Short-circuit: if any pattern returns empty, the whole join is empty
        if rel.is_empty() && !rel.attrs.is_empty() {
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
        rels.into_iter().reduce(|a, b| relation::prod_rel(&a, &b)).unwrap()
    }
}

/// Collect result tuples for the given find variables from a resolved relation.
pub fn collect_results(rel: &Relation, find_vars: &[Var]) -> Vec<Tuple> {
    let projected = project(rel, find_vars);
    projected.tuples
}

// ---------------------------------------------------------------------------
// Full clause + rule query
// ---------------------------------------------------------------------------

/// Resolve a full query with clauses (including rule calls) and rules,
/// returning result tuples for the given find variables.
pub fn resolve_clauses_with_rules(
    db: &WasmDataScript,
    clauses: &[Clause],
    rules: &Rules,
    find_vars: &[Var],
) -> Vec<Tuple> {
    let result = resolve_query(db, clauses, rules);
    collect_results(&result, find_vars)
}

pub fn resolve_clauses_with_rules_and_initial(
    db: &WasmDataScript,
    clauses: &[Clause],
    rules: &Rules,
    find_vars: &[Var],
    initial_rels: Vec<Relation>,
) -> Vec<Tuple> {
    let result = resolve_query_with_initial(db, clauses, rules, initial_rels);
    collect_results(&result, find_vars)
}

// ---------------------------------------------------------------------------
// PullSource impl for WasmDataScript
// ---------------------------------------------------------------------------

impl PullSource for WasmDataScript {
    fn entity_datoms(&self, eid: i64) -> Vec<Datom> {
        search_internal(self, Some(eid), None, None, None)
    }

    fn reverse_datoms(&self, attr: &Attr, eid: i64) -> Vec<Datom> {
        search_internal(self, None, Some(attr), Some(&Value::Ref(eid)), None)
    }

    fn resolve_lookup_ref(&self, attr: &Attr, v: &Value) -> Option<i64> {
        let results = search_internal(self, None, Some(attr), Some(v), None);
        results.first().map(|d| d.e)
    }

    fn schema(&self) -> &Schema {
        self.schema_ref()
    }

    fn rschema(&self) -> &ReverseSchema {
        self.rschema_ref()
    }
}
