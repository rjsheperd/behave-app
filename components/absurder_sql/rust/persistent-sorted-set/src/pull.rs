//! Pull executor for DataScript pull expressions.
//!
//! Walks entity datoms against a parsed `PullPattern`, producing nested maps.

use std::collections::HashSet;

use crate::datom::{Attr, Datom, Value};
use crate::pull_parser::{PullAttr, PullPattern, default_wildcard_pattern};
use crate::schema::{ReverseSchema, Schema};

// ---------------------------------------------------------------------------
// Pull result type
// ---------------------------------------------------------------------------

/// Result of a pull operation — nested map with scalar leaves.
#[derive(Clone, Debug, PartialEq)]
pub enum PullResult {
    Scalar(Value),
    Vec(Vec<PullResult>),
    Map(Vec<(Attr, PullResult)>),
}

// ---------------------------------------------------------------------------
// PullSource trait
// ---------------------------------------------------------------------------

/// Trait for database access during pull.
/// Implemented by both `DataScriptDB` (native tests) and `WasmDataScript` (WASM).
pub trait PullSource {
    /// All datoms for entity `eid` (EAVT slice).
    fn entity_datoms(&self, eid: i64) -> Vec<Datom>;

    /// All datoms where `attr = attr` and `v = Ref(eid)` (reverse lookup).
    fn reverse_datoms(&self, attr: &Attr, eid: i64) -> Vec<Datom>;

    /// Resolve an entity ID from a value.
    /// - `Long(n)` / `Ref(n)` → `Some(n)`
    /// - Lookup ref not supported at this level (handled by caller).
    fn entid_scalar(&self, v: &Value) -> Option<i64> {
        match v {
            Value::Long(n) | Value::Ref(n) if *n > 0 => Some(*n),
            _ => None,
        }
    }

    /// Resolve a lookup ref `[attr, value]` to an entity ID via AVET.
    fn resolve_lookup_ref(&self, attr: &Attr, v: &Value) -> Option<i64>;

    fn schema(&self) -> &Schema;
    fn rschema(&self) -> &ReverseSchema;
}

// ---------------------------------------------------------------------------
// Pull executor
// ---------------------------------------------------------------------------

/// Pull an entity by pattern. Returns `None` if the entity has no datoms.
pub fn pull(source: &dyn PullSource, pattern: &PullPattern, eid: i64) -> Option<PullResult> {
    let datoms = source.entity_datoms(eid);
    if datoms.is_empty() {
        return None;
    }
    let mut seen = HashSet::new();
    seen.insert(eid);
    Some(pull_inner(source, pattern, eid, &datoms, &mut seen))
}

fn pull_inner(
    source: &dyn PullSource,
    pattern: &PullPattern,
    eid: i64,
    datoms: &[Datom],
    seen: &mut HashSet<i64>,
) -> PullResult {
    let rschema = source.rschema();
    let mut entries: Vec<(Attr, PullResult)> = Vec::new();

    // Track which attrs were explicitly pulled (for wildcard expansion)
    let mut pulled_attrs: HashSet<Attr> = HashSet::new();

    // Process explicit forward attrs
    for pull_attr in &pattern.attrs {
        pulled_attrs.insert(pull_attr.name.clone());

        // Special case: :db/id
        let db_id = Attr::Keyword {
            ns: Some("db".into()),
            name: "id".into(),
        };
        if pull_attr.name == db_id {
            entries.push((db_id, PullResult::Scalar(Value::Long(eid))));
            continue;
        }

        // Find matching datoms for this attr
        let matching: Vec<&Datom> = datoms
            .iter()
            .filter(|d| d.a.as_ref() == Some(&pull_attr.name))
            .collect();

        if matching.is_empty() {
            continue;
        }

        let result = pull_attr_value(source, pull_attr, &matching, seen);
        entries.push((pull_attr.as_name.clone(), result));
    }

    // Wildcard: include all datom attrs not explicitly pulled
    if pattern.wildcard {
        let mut current_attr: Option<Attr> = None;
        let mut current_datoms: Vec<&Datom> = Vec::new();

        for datom in datoms {
            let attr = match &datom.a {
                Some(a) => a.clone(),
                None => continue,
            };

            if pulled_attrs.contains(&attr) {
                continue;
            }

            if current_attr.as_ref() == Some(&attr) {
                current_datoms.push(datom);
            } else {
                // Flush previous group
                if let Some(ref prev_attr) = current_attr {
                    let wildcard_pull_attr = make_wildcard_attr(prev_attr, rschema);
                    let result = pull_attr_value(source, &wildcard_pull_attr, &current_datoms, seen);
                    entries.push((prev_attr.clone(), result));
                }
                current_attr = Some(attr);
                current_datoms = vec![datom];
            }
        }
        // Flush last group
        if let Some(ref prev_attr) = current_attr {
            let wildcard_pull_attr = make_wildcard_attr(prev_attr, rschema);
            let result = pull_attr_value(source, &wildcard_pull_attr, &current_datoms, seen);
            entries.push((prev_attr.clone(), result));
        }
    }

    // Process reverse attrs
    for pull_attr in &pattern.reverse_attrs {
        let rev_datoms = source.reverse_datoms(&pull_attr.name, eid);
        if rev_datoms.is_empty() {
            continue;
        }

        if pull_attr.component {
            // Component reverse ref: return single entity
            let ref_eid = rev_datoms[0].e;
            let result = pull_ref(source, pull_attr, ref_eid, seen);
            entries.push((pull_attr.as_name.clone(), result));
        } else {
            // Non-component: return vector of pulled entities
            let mut items = Vec::new();
            for d in &rev_datoms {
                let item = pull_ref(source, pull_attr, d.e, seen);
                items.push(item);
            }
            entries.push((pull_attr.as_name.clone(), PullResult::Vec(items)));
        }
    }

    // Sort entries by key for deterministic output
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    PullResult::Map(entries)
}

fn make_wildcard_attr(attr: &Attr, rschema: &ReverseSchema) -> PullAttr {
    let multival = rschema.is_multival(attr);
    let ref_type = rschema.is_ref(attr);
    let component = rschema.is_component(attr);

    // For wildcard + component refs, auto-expand with wildcard pattern
    let pattern = if ref_type && component {
        Some(default_wildcard_pattern())
    } else {
        None
    };

    PullAttr {
        name: attr.clone(),
        as_name: attr.clone(),
        reverse: false,
        multival,
        ref_type,
        component,
        pattern,
    }
}

fn pull_attr_value(
    source: &dyn PullSource,
    pull_attr: &PullAttr,
    datoms: &[&Datom],
    seen: &mut HashSet<i64>,
) -> PullResult {
    if pull_attr.multival {
        // Cardinality/many: collect into vector
        let mut items = Vec::new();
        for d in datoms {
            if pull_attr.ref_type {
                if let Some(ref_eid) = ref_eid_from_value(&d.v) {
                    items.push(pull_ref(source, pull_attr, ref_eid, seen));
                }
            } else {
                items.push(PullResult::Scalar(d.v.clone()));
            }
        }
        PullResult::Vec(items)
    } else if pull_attr.ref_type {
        // Single ref: pull nested entity
        if let Some(ref_eid) = ref_eid_from_value(&datoms[0].v) {
            pull_ref(source, pull_attr, ref_eid, seen)
        } else {
            PullResult::Scalar(datoms[0].v.clone())
        }
    } else {
        // Simple scalar
        PullResult::Scalar(datoms[0].v.clone())
    }
}

fn pull_ref(
    source: &dyn PullSource,
    pull_attr: &PullAttr,
    ref_eid: i64,
    seen: &mut HashSet<i64>,
) -> PullResult {
    match &pull_attr.pattern {
        Some(nested_pattern) => {
            // Cycle detection
            if seen.contains(&ref_eid) {
                // Return just {:db/id eid} for cycles
                let db_id = Attr::Keyword {
                    ns: Some("db".into()),
                    name: "id".into(),
                };
                return PullResult::Map(vec![
                    (db_id, PullResult::Scalar(Value::Long(ref_eid))),
                ]);
            }
            seen.insert(ref_eid);
            let ref_datoms = source.entity_datoms(ref_eid);
            if ref_datoms.is_empty() {
                PullResult::Map(vec![])
            } else {
                pull_inner(source, nested_pattern, ref_eid, &ref_datoms, seen)
            }
        }
        None => {
            // No nested pattern: return {:db/id eid}
            let db_id = Attr::Keyword {
                ns: Some("db".into()),
                name: "id".into(),
            };
            PullResult::Map(vec![
                (db_id, PullResult::Scalar(Value::Long(ref_eid))),
            ])
        }
    }
}

fn ref_eid_from_value(v: &Value) -> Option<i64> {
    match v {
        Value::Ref(n) => Some(*n),
        Value::Long(n) => Some(*n),
        _ => None,
    }
}
