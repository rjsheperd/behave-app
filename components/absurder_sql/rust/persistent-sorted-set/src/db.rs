//! DataScript database: schema + three sorted indexes (EAVT, AEVT, AVET).
//!
//! On native targets this uses `Vec<Datom>` for indexes (for testing).
//! On WASM targets this will use `PersistentSortedSet` with `Key = Datom`.

use std::collections::HashMap;
use std::cmp::Ordering;

use crate::comparator::{cmp_datoms, IndexType, value_cmp};
use crate::datom::{Attr, Datom, Value};
use crate::pull::PullSource;
use crate::relation::{PatternEl, PatternResolver, Relation, Tuple};
use crate::schema::{ReverseSchema, Schema, build_rschema};
use crate::transact::TransactableDB;

/// DataScript constants matching CLJS `db.cljc`.
pub const E0: i64 = 0;
pub const TX0: i64 = 0x20000000; // 536870912 — first valid tx id
/// Minimum tx value for slice bounds (0, not TX0).
pub const TX_MIN: i64 = 0;
pub const EMAX: i64 = 0x7FFFFFFF;
pub const TXMAX: i64 = 0x7FFFFFFF;

/// A sorted index backed by a Vec<Datom> (native testing only).
/// Maintains datoms in sorted order according to its IndexType.
#[derive(Clone, Debug)]
pub struct SortedIndex {
    index_type: IndexType,
    datoms: Vec<Datom>,
}

impl SortedIndex {
    pub fn new(index_type: IndexType) -> Self {
        Self { index_type, datoms: Vec::new() }
    }

    fn cmp(&self, a: &Datom, b: &Datom) -> Ordering {
        cmp_datoms(self.index_type, a, b)
    }

    /// Insert a datom maintaining sort order. No duplicates.
    pub fn conj(&mut self, datom: Datom) {
        match self.datoms.binary_search_by(|d| self.cmp(d, &datom)) {
            Ok(_) => {} // already present
            Err(pos) => self.datoms.insert(pos, datom),
        }
    }

    /// Remove a datom (by e, a, v — ignoring tx sign for matching).
    pub fn disj(&mut self, datom: &Datom) {
        if let Ok(pos) = self.datoms.binary_search_by(|d| self.cmp(d, datom)) {
            self.datoms.remove(pos);
        }
    }

    pub fn len(&self) -> usize {
        self.datoms.len()
    }

    pub fn is_empty(&self) -> bool {
        self.datoms.is_empty()
    }

    /// Bounded slice: returns all datoms where `from <= d <= to`.
    /// Uses the wildcard-aware comparator (None attr / Nil value = match anything).
    pub fn slice(&self, from: &Datom, to: &Datom) -> Vec<&Datom> {
        self.datoms.iter()
            .filter(|d| {
                self.cmp(d, from) != Ordering::Less
                    && self.cmp(d, to) != Ordering::Greater
            })
            .collect()
    }

    /// Forward scan from `from` to the end.
    pub fn seek(&self, from: &Datom) -> Vec<&Datom> {
        self.datoms.iter()
            .filter(|d| self.cmp(d, from) != Ordering::Less)
            .collect()
    }

    /// Reverse scan from `from` to the start.
    pub fn rseek(&self, from: &Datom) -> Vec<&Datom> {
        self.datoms.iter().rev()
            .filter(|d| self.cmp(d, from) != Ordering::Greater)
            .collect()
    }

    pub fn all(&self) -> &[Datom] {
        &self.datoms
    }

    /// Find exact match by e, a, v (first match).
    pub fn find(&self, e: i64, a: &Option<Attr>, v: &Value) -> Option<&Datom> {
        self.datoms.iter().find(|d| {
            d.e == e
                && d.a == *a
                && datom_val_eq(&d.v, v)
        })
    }
}

/// Value equality for datom matching (not Nil-wildcard — strict equality).
fn datom_val_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Nil, Value::Nil) => true,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Long(a), Value::Long(b)) => a == b,
        (Value::Double(a), Value::Double(b)) => a.to_bits() == b.to_bits(),
        (Value::Str(a), Value::Str(b)) => a == b,
        (Value::Keyword(a), Value::Keyword(b)) => a == b,
        (Value::Ref(a), Value::Ref(b)) => a == b,
        (Value::Instant(a), Value::Instant(b)) => a == b,
        (Value::Uuid(a), Value::Uuid(b)) => a == b,
        (Value::Bytes(a), Value::Bytes(b)) => a == b,
        _ => false,
    }
}

/// The DataScript database.
#[derive(Clone, Debug)]
pub struct DataScriptDB {
    pub schema: Schema,
    pub rschema: ReverseSchema,
    pub eavt: SortedIndex,
    pub aevt: SortedIndex,
    pub avet: SortedIndex,
    pub max_eid: i64,
    pub max_tx: i64,
}

impl DataScriptDB {
    /// Create an empty database with the given schema.
    pub fn empty(schema: Schema) -> Self {
        let rschema = build_rschema(&schema);
        Self {
            schema,
            rschema,
            eavt: SortedIndex::new(IndexType::EAVT),
            aevt: SortedIndex::new(IndexType::AEVT),
            avet: SortedIndex::new(IndexType::AVET),
            max_eid: E0,
            max_tx: TX0,
        }
    }

    /// Add or retract a datom.
    /// Matches CLJS `with-datom` from `db.cljc:1438-1454`.
    pub fn with_datom(&mut self, datom: Datom) {
        let indexing = self.rschema.is_indexed(
            datom.a.as_ref().expect("datom must have attribute"),
        );

        if datom.tx > 0 {
            // Adding
            self.eavt.conj(datom.clone());
            self.aevt.conj(datom.clone());
            if indexing {
                self.avet.conj(datom.clone());
            }
            self.advance_max_eid(datom.e);
        } else {
            // Retracting — find the existing datom first
            if let Some(existing) = self.eavt.find(datom.e, &datom.a, &datom.v).cloned() {
                self.eavt.disj(&existing);
                self.aevt.disj(&existing);
                if indexing {
                    self.avet.disj(&existing);
                }
            }
        }
    }

    /// Batch add/retract datoms.
    pub fn with_datoms(&mut self, datoms: Vec<Datom>) {
        for datom in datoms {
            self.with_datom(datom);
        }
    }

    fn advance_max_eid(&mut self, e: i64) {
        if e > self.max_eid {
            self.max_eid = e;
        }
    }

    pub fn count(&self) -> usize {
        self.eavt.len()
    }

    /// Search by pattern `[e, a, v, tx]` where None/Nil = wildcard.
    /// Mirrors the 16-case dispatch tree from CLJS `db.cljc:737-777`.
    pub fn search(
        &self,
        e: Option<i64>,
        a: Option<&Attr>,
        v: Option<&Value>,
        tx: Option<i64>,
    ) -> Vec<&Datom> {
        match (e, a, v, tx) {
            // [e a v tx] — exact match in EAVT
            (Some(e), Some(a), Some(v), Some(tx)) => {
                let from = Datom::new(e, Some(a.clone()), v.clone(), tx);
                let to = Datom::new(e, Some(a.clone()), v.clone(), tx);
                self.eavt.slice(&from, &to)
            }
            // [e a v _] — EAVT slice on e,a,v
            (Some(e), Some(a), Some(v), None) => {
                let from = Datom::new(e, Some(a.clone()), v.clone(), TX0);
                let to = Datom::new(e, Some(a.clone()), v.clone(), TXMAX);
                self.eavt.slice(&from, &to)
            }
            // [e a _ _] — EAVT slice on e,a
            (Some(e), Some(a), None, None) => {
                let from = Datom::new(e, Some(a.clone()), Value::Nil, TX0);
                let to = Datom::new(e, Some(a.clone()), Value::Nil, TXMAX);
                self.eavt.slice(&from, &to)
            }
            // [e a _ tx] — EAVT slice on e,a, filter by tx
            (Some(e), Some(a), None, Some(tx)) => {
                let from = Datom::new(e, Some(a.clone()), Value::Nil, TX0);
                let to = Datom::new(e, Some(a.clone()), Value::Nil, TXMAX);
                self.eavt.slice(&from, &to)
                    .into_iter()
                    .filter(|d| d.tx_id() == tx)
                    .collect()
            }
            // [e _ _ _] — EAVT slice on e
            (Some(e), None, None, None) => {
                let from = Datom::new(e, None, Value::Nil, TX0);
                let to = Datom::new(e, None, Value::Nil, TXMAX);
                self.eavt.slice(&from, &to)
            }
            // [e _ v _] — EAVT slice on e, filter by v
            (Some(e), None, Some(v), None) => {
                let from = Datom::new(e, None, Value::Nil, TX0);
                let to = Datom::new(e, None, Value::Nil, TXMAX);
                self.eavt.slice(&from, &to)
                    .into_iter()
                    .filter(|d| value_cmp(&d.v, v) == Ordering::Equal)
                    .collect()
            }
            // [e _ _ tx] — EAVT slice on e, filter by tx
            (Some(e), None, None, Some(tx)) => {
                let from = Datom::new(e, None, Value::Nil, TX0);
                let to = Datom::new(e, None, Value::Nil, TXMAX);
                self.eavt.slice(&from, &to)
                    .into_iter()
                    .filter(|d| d.tx_id() == tx)
                    .collect()
            }
            // [e _ v tx] — EAVT slice on e, filter by v and tx
            (Some(e), None, Some(v), Some(tx)) => {
                let from = Datom::new(e, None, Value::Nil, TX0);
                let to = Datom::new(e, None, Value::Nil, TXMAX);
                self.eavt.slice(&from, &to)
                    .into_iter()
                    .filter(|d| value_cmp(&d.v, v) == Ordering::Equal && d.tx_id() == tx)
                    .collect()
            }
            // [_ a v _] — AVET if indexed, else AEVT filter
            (None, Some(a), Some(v), None) => {
                if self.rschema.is_indexed(a) {
                    let from = Datom::new(E0, Some(a.clone()), v.clone(), TX0);
                    let to = Datom::new(EMAX, Some(a.clone()), v.clone(), TXMAX);
                    self.avet.slice(&from, &to)
                } else {
                    let from = Datom::new(E0, Some(a.clone()), Value::Nil, TX0);
                    let to = Datom::new(EMAX, Some(a.clone()), Value::Nil, TXMAX);
                    self.aevt.slice(&from, &to)
                        .into_iter()
                        .filter(|d| value_cmp(&d.v, v) == Ordering::Equal)
                        .collect()
                }
            }
            // [_ a v tx] — like above but also filter by tx
            (None, Some(a), Some(v), Some(tx)) => {
                if self.rschema.is_indexed(a) {
                    let from = Datom::new(E0, Some(a.clone()), v.clone(), TX0);
                    let to = Datom::new(EMAX, Some(a.clone()), v.clone(), TXMAX);
                    self.avet.slice(&from, &to)
                        .into_iter()
                        .filter(|d| d.tx_id() == tx)
                        .collect()
                } else {
                    let from = Datom::new(E0, Some(a.clone()), Value::Nil, TX0);
                    let to = Datom::new(EMAX, Some(a.clone()), Value::Nil, TXMAX);
                    self.aevt.slice(&from, &to)
                        .into_iter()
                        .filter(|d| value_cmp(&d.v, v) == Ordering::Equal && d.tx_id() == tx)
                        .collect()
                }
            }
            // [_ a _ _] — AEVT slice on a
            (None, Some(a), None, None) => {
                let from = Datom::new(E0, Some(a.clone()), Value::Nil, TX0);
                let to = Datom::new(EMAX, Some(a.clone()), Value::Nil, TXMAX);
                self.aevt.slice(&from, &to)
            }
            // [_ a _ tx] — AEVT slice on a, filter by tx
            (None, Some(a), None, Some(tx)) => {
                let from = Datom::new(E0, Some(a.clone()), Value::Nil, TX0);
                let to = Datom::new(EMAX, Some(a.clone()), Value::Nil, TXMAX);
                self.aevt.slice(&from, &to)
                    .into_iter()
                    .filter(|d| d.tx_id() == tx)
                    .collect()
            }
            // [_ _ v _] — EAVT full scan, filter by v
            (None, None, Some(v), None) => {
                self.eavt.all().iter()
                    .filter(|d| value_cmp(&d.v, v) == Ordering::Equal)
                    .collect()
            }
            // [_ _ v tx] — EAVT full scan, filter by v and tx
            (None, None, Some(v), Some(tx)) => {
                self.eavt.all().iter()
                    .filter(|d| value_cmp(&d.v, v) == Ordering::Equal && d.tx_id() == tx)
                    .collect()
            }
            // [_ _ _ tx] — EAVT full scan, filter by tx
            (None, None, None, Some(tx)) => {
                self.eavt.all().iter()
                    .filter(|d| d.tx_id() == tx)
                    .collect()
            }
            // [_ _ _ _] — return all
            (None, None, None, None) => {
                self.eavt.all().iter().collect()
            }
        }
    }

    /// `datoms(index, c0, c1, c2, c3)` — bounded slice on named index.
    /// Components are in index-specific order (e.g., for :aevt, c0=attr, c1=entity).
    /// Mirrors CLJS `db.cljc:780-785`.
    pub fn datoms(
        &self,
        index: IndexType,
        c0: Option<&Datom>,
        c1: Option<&Datom>,
    ) -> Vec<&Datom> {
        let idx = match index {
            IndexType::EAVT => &self.eavt,
            IndexType::AEVT => &self.aevt,
            IndexType::AVET => &self.avet,
        };
        match (c0, c1) {
            (Some(from), Some(to)) => idx.slice(from, to),
            (Some(from), None) => idx.seek(from),
            (None, Some(to)) => idx.slice(
                &Datom::new(E0, None, Value::Nil, TX0),
                to,
            ),
            (None, None) => idx.all().iter().collect(),
        }
    }

    /// `seek_datoms(index, from)` — forward scan from pattern.
    pub fn seek_datoms(&self, index: IndexType, from: &Datom) -> Vec<&Datom> {
        let idx = match index {
            IndexType::EAVT => &self.eavt,
            IndexType::AEVT => &self.aevt,
            IndexType::AVET => &self.avet,
        };
        idx.seek(from)
    }

    /// `rseek_datoms(index, from)` — reverse scan from pattern.
    pub fn rseek_datoms(&self, index: IndexType, from: &Datom) -> Vec<&Datom> {
        let idx = match index {
            IndexType::EAVT => &self.eavt,
            IndexType::AEVT => &self.aevt,
            IndexType::AVET => &self.avet,
        };
        idx.rseek(from)
    }

    /// `index_range(attr, start, end)` — value range on AVET.
    pub fn index_range(&self, attr: &Attr, start: &Value, end: &Value) -> Vec<&Datom> {
        let from = Datom::new(E0, Some(attr.clone()), start.clone(), TX0);
        let to = Datom::new(EMAX, Some(attr.clone()), end.clone(), TXMAX);
        self.avet.slice(&from, &to)
    }
}

impl TransactableDB for DataScriptDB {
    fn search_eav(&self, e: i64, a: &Attr, v: &Value) -> Option<Datom> {
        self.search(Some(e), Some(a), Some(v), None)
            .first()
            .cloned()
            .cloned()
    }

    fn search_ea(&self, e: i64, a: &Attr) -> Vec<Datom> {
        self.search(Some(e), Some(a), None, None)
            .into_iter()
            .cloned()
            .collect()
    }

    fn search_e(&self, e: i64) -> Vec<Datom> {
        self.search(Some(e), None, None, None)
            .into_iter()
            .cloned()
            .collect()
    }

    fn search_av(&self, a: &Attr, v: &Value) -> Vec<Datom> {
        self.search(None, Some(a), Some(v), None)
            .into_iter()
            .cloned()
            .collect()
    }

    fn search_a_refs(&self, a: &Attr, v_ref: i64) -> Vec<Datom> {
        self.search(None, Some(a), Some(&Value::Ref(v_ref)), None)
            .into_iter()
            .cloned()
            .collect()
    }

    fn apply_datom(&mut self, datom: Datom) {
        self.with_datom(datom);
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }

    fn rschema(&self) -> &ReverseSchema {
        &self.rschema
    }

    fn max_eid(&self) -> i64 {
        self.max_eid
    }

    fn set_max_eid(&mut self, eid: i64) {
        self.max_eid = eid;
    }

    fn max_tx(&self) -> i64 {
        self.max_tx
    }

    fn set_max_tx(&mut self, tx: i64) {
        self.max_tx = tx;
    }
}

impl PatternResolver for DataScriptDB {
    fn resolve_pattern(&self, pattern: &[PatternEl; 4]) -> Relation {
        let e = match &pattern[0] {
            PatternEl::Const(Value::Long(n)) | PatternEl::Const(Value::Ref(n)) => Some(*n),
            _ => None,
        };
        let a = match &pattern[1] {
            PatternEl::Const(Value::Keyword(attr)) => Some(attr),
            _ => None,
        };
        // For the value position: if the attribute is ref-typed and the constant
        // is Long(n), search with Ref(n) instead (DataScript stores refs as Ref).
        // Also handle the reverse: if Ref(n) is given, try both.
        let v = match &pattern[2] {
            PatternEl::Const(Value::Long(n)) if a.map_or(false, |a| self.rschema.is_ref(a)) => {
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

        let datoms = self.search(e, a, v_ref, tx);

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
                                    // This matches DataScript semantics where refs are
                                    // just numbers.
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

impl PullSource for DataScriptDB {
    fn entity_datoms(&self, eid: i64) -> Vec<Datom> {
        let from = Datom::new(eid, None, Value::Nil, TX_MIN);
        let to = Datom::new(eid, None, Value::Nil, TXMAX);
        self.eavt.slice(&from, &to).into_iter().cloned().collect()
    }

    fn reverse_datoms(&self, attr: &Attr, eid: i64) -> Vec<Datom> {
        // Reverse lookup: find all datoms where a=attr and v=Ref(eid).
        // Ref attrs are always indexed, so use AVET.
        let from = Datom::new(E0, Some(attr.clone()), Value::Ref(eid), TX_MIN);
        let to = Datom::new(EMAX, Some(attr.clone()), Value::Ref(eid), TXMAX);
        self.avet.slice(&from, &to).into_iter().cloned().collect()
    }

    fn resolve_lookup_ref(&self, attr: &Attr, v: &Value) -> Option<i64> {
        let from = Datom::new(E0, Some(attr.clone()), v.clone(), TX_MIN);
        let to = Datom::new(EMAX, Some(attr.clone()), v.clone(), TXMAX);
        self.avet.slice(&from, &to).first().map(|d| d.e)
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }

    fn rschema(&self) -> &ReverseSchema {
        &self.rschema
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{AttrSchema, Cardinality, Schema, Unique, ValueType, kw};

    fn test_schema() -> Schema {
        let mut schema = Schema::default();
        schema.attrs.insert(kw("name"), AttrSchema { index: true, ..Default::default() });
        schema.attrs.insert(kw("age"), AttrSchema::default());
        schema.attrs.insert(kw("parent"), AttrSchema { value_type: Some(ValueType::Ref), ..Default::default() });
        schema.attrs.insert(kw("email"), AttrSchema { unique: Some(Unique::Identity), ..Default::default() });
        schema.attrs.insert(kw("aka"), AttrSchema { cardinality: Cardinality::Many, ..Default::default() });
        schema
    }

    fn a(name: &str) -> Option<Attr> {
        Some(kw(name))
    }

    /// Create a datom with TX0-based tx (matches real DataScript behavior).
    /// Use negative tx for retraction.
    fn d(e: i64, attr: &str, v: Value, tx: i64) -> Datom {
        if tx < 0 {
            Datom::new(e, Some(kw(attr)), v, -(TX0 + (-tx)))
        } else {
            Datom::new(e, Some(kw(attr)), v, TX0 + tx)
        }
    }

    // --- Module 2: with_datom routing tests ---

    #[test]
    fn with_datom_add_non_indexed() {
        let mut db = DataScriptDB::empty(test_schema());
        db.with_datom(d(1, "age", Value::Long(30), 100));

        assert_eq!(db.eavt.len(), 1);
        assert_eq!(db.aevt.len(), 1);
        assert_eq!(db.avet.len(), 0, "non-indexed attr should not be in AVET");
    }

    #[test]
    fn with_datom_add_indexed() {
        let mut db = DataScriptDB::empty(test_schema());
        db.with_datom(d(1, "name", Value::Str("Alice".into()), 100));

        assert_eq!(db.eavt.len(), 1);
        assert_eq!(db.aevt.len(), 1);
        assert_eq!(db.avet.len(), 1, "indexed attr should be in AVET");
    }

    #[test]
    fn with_datom_add_ref() {
        let mut db = DataScriptDB::empty(test_schema());
        db.with_datom(d(1, "parent", Value::Ref(2), 100));

        assert_eq!(db.eavt.len(), 1);
        assert_eq!(db.aevt.len(), 1);
        assert_eq!(db.avet.len(), 1, "ref attr should be in AVET (refs are implicitly indexed)");
    }

    #[test]
    fn with_datom_add_unique() {
        let mut db = DataScriptDB::empty(test_schema());
        db.with_datom(d(1, "email", Value::Str("a@b.com".into()), 100));

        assert_eq!(db.eavt.len(), 1);
        assert_eq!(db.aevt.len(), 1);
        assert_eq!(db.avet.len(), 1, "unique attr should be in AVET");
    }

    #[test]
    fn with_datom_retract() {
        let mut db = DataScriptDB::empty(test_schema());
        db.with_datom(d(1, "name", Value::Str("Alice".into()), 100));
        assert_eq!(db.eavt.len(), 1);

        // Retract (negative tx)
        db.with_datom(d(1, "name", Value::Str("Alice".into()), -100));
        assert_eq!(db.eavt.len(), 0);
        assert_eq!(db.aevt.len(), 0);
        assert_eq!(db.avet.len(), 0);
    }

    #[test]
    fn with_datom_retract_non_indexed() {
        let mut db = DataScriptDB::empty(test_schema());
        db.with_datom(d(1, "age", Value::Long(30), 100));
        assert_eq!(db.eavt.len(), 1);
        assert_eq!(db.avet.len(), 0);

        db.with_datom(d(1, "age", Value::Long(30), -100));
        assert_eq!(db.eavt.len(), 0);
        assert_eq!(db.aevt.len(), 0);
        assert_eq!(db.avet.len(), 0, "AVET was never touched");
    }

    #[test]
    fn with_datom_retract_nonexistent() {
        let mut db = DataScriptDB::empty(test_schema());
        let before_count = db.count();
        db.with_datom(d(1, "name", Value::Str("Ghost".into()), -100));
        assert_eq!(db.count(), before_count, "retract of nonexistent should be no-op");
    }

    #[test]
    fn with_datom_multiple_entities() {
        let mut db = DataScriptDB::empty(test_schema());
        // 5 indexed (name, email) + 5 non-indexed (age, aka)
        db.with_datom(d(1, "name", Value::Str("Alice".into()), 100));
        db.with_datom(d(1, "age", Value::Long(30), 100));
        db.with_datom(d(1, "email", Value::Str("a@b.com".into()), 100));
        db.with_datom(d(2, "name", Value::Str("Bob".into()), 100));
        db.with_datom(d(2, "age", Value::Long(25), 100));
        db.with_datom(d(2, "email", Value::Str("b@b.com".into()), 100));
        db.with_datom(d(3, "name", Value::Str("Carol".into()), 100));
        db.with_datom(d(3, "age", Value::Long(35), 100));
        db.with_datom(d(3, "aka", Value::Str("C".into()), 100));
        db.with_datom(d(3, "parent", Value::Ref(1), 100));

        assert_eq!(db.eavt.len(), 10);
        assert_eq!(db.aevt.len(), 10);
        // Indexed: name(3) + email(2) + parent(1) = 6
        assert_eq!(db.avet.len(), 6, "AVET should have only indexed attr datoms");
    }

    #[test]
    fn with_datom_advance_max_eid() {
        let mut db = DataScriptDB::empty(test_schema());
        db.with_datom(d(5, "name", Value::Str("A".into()), 100));
        assert_eq!(db.max_eid, 5);

        db.with_datom(d(3, "name", Value::Str("B".into()), 100));
        assert_eq!(db.max_eid, 5, "max_eid should not decrease");

        db.with_datom(d(10, "name", Value::Str("C".into()), 100));
        assert_eq!(db.max_eid, 10);
    }

    // --- Module 3: search dispatch tests ---

    fn populated_db() -> DataScriptDB {
        let mut db = DataScriptDB::empty(test_schema());
        db.with_datom(d(1, "name", Value::Str("Alice".into()), 100));
        db.with_datom(d(1, "age", Value::Long(30), 100));
        db.with_datom(d(1, "email", Value::Str("a@b.com".into()), 100));
        db.with_datom(d(2, "name", Value::Str("Bob".into()), 100));
        db.with_datom(d(2, "age", Value::Long(25), 200));
        db.with_datom(d(2, "email", Value::Str("b@b.com".into()), 200));
        db.with_datom(d(3, "name", Value::Str("Carol".into()), 200));
        db.with_datom(d(3, "age", Value::Long(35), 200));
        db
    }

    #[test]
    fn search_eavtx() {
        let db = populated_db();
        let results = db.search(
            Some(1), Some(&kw("name")), Some(&Value::Str("Alice".into())), Some(TX0 + 100),
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].e, 1);
    }

    #[test]
    fn search_eav() {
        let db = populated_db();
        let results = db.search(
            Some(1), Some(&kw("name")), Some(&Value::Str("Alice".into())), None,
        );
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn search_ea() {
        let db = populated_db();
        let results = db.search(Some(1), Some(&kw("name")), None, None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].v, Value::Str("Alice".into()));
    }

    #[test]
    fn search_e() {
        let db = populated_db();
        let results = db.search(Some(1), None, None, None);
        assert_eq!(results.len(), 3, "entity 1 has name, age, email");
    }

    #[test]
    fn search_av_indexed() {
        let db = populated_db();
        // :name is indexed → should use AVET
        let results = db.search(
            None, Some(&kw("name")), Some(&Value::Str("Alice".into())), None,
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].e, 1);
    }

    #[test]
    fn search_av_not_indexed() {
        let db = populated_db();
        // :age is NOT indexed → should fall back to AEVT filter
        let results = db.search(
            None, Some(&kw("age")), Some(&Value::Long(30)), None,
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].e, 1);
    }

    #[test]
    fn search_a() {
        let db = populated_db();
        let results = db.search(None, Some(&kw("name")), None, None);
        assert_eq!(results.len(), 3, "3 entities have :name");
    }

    #[test]
    fn search_full_scan() {
        let db = populated_db();
        let results = db.search(None, None, None, None);
        assert_eq!(results.len(), 8, "total datoms");
    }

    // --- Module 4: datoms/seek-datoms/rseek-datoms/index-range ---

    #[test]
    fn datoms_eavt_by_entity() {
        let db = populated_db();
        let from = Datom::new(1, None, Value::Nil, TX0);
        let to = Datom::new(1, None, Value::Nil, TXMAX);
        let results = db.datoms(IndexType::EAVT, Some(&from), Some(&to));
        assert_eq!(results.len(), 3);
        for d in &results {
            assert_eq!(d.e, 1);
        }
    }

    #[test]
    fn datoms_aevt_by_attr() {
        let db = populated_db();
        let from = Datom::new(E0, a("name"), Value::Nil, TX0);
        let to = Datom::new(EMAX, a("name"), Value::Nil, TXMAX);
        let results = db.datoms(IndexType::AEVT, Some(&from), Some(&to));
        assert_eq!(results.len(), 3);
        for d in &results {
            assert_eq!(d.a, a("name"));
        }
    }

    #[test]
    fn datoms_avet_by_attr_value() {
        let db = populated_db();
        let from = Datom::new(E0, a("name"), Value::Str("Alice".into()), TX0);
        let to = Datom::new(EMAX, a("name"), Value::Str("Alice".into()), TXMAX);
        let results = db.datoms(IndexType::AVET, Some(&from), Some(&to));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].e, 1);
    }

    #[test]
    fn seek_datoms_forward() {
        let db = populated_db();
        // Seek from entity 2 forward in EAVT
        let from = Datom::new(2, None, Value::Nil, TX0);
        let results = db.seek_datoms(IndexType::EAVT, &from);
        // Should include entity 2 (3 datoms) + entity 3 (2 datoms)
        assert!(results.len() >= 5, "got {} datoms from entity 2 onward", results.len());
        for d in &results {
            assert!(d.e >= 2);
        }
    }

    #[test]
    fn rseek_datoms_backward() {
        let db = populated_db();
        // Reverse seek from entity 2 backward in EAVT
        let from = Datom::new(2, None, Value::Nil, TXMAX);
        let results = db.rseek_datoms(IndexType::EAVT, &from);
        // Should include entity 1 (3 datoms) + entity 2 (3 datoms) in reverse
        assert!(results.len() >= 3, "got {} datoms through entity 2", results.len());
        // Results are reversed
        if results.len() >= 2 {
            assert!(results[0].e >= results[results.len() - 1].e);
        }
    }

    #[test]
    fn index_range_avet() {
        let mut db = DataScriptDB::empty({
            let mut s = Schema::default();
            s.attrs.insert(kw("age"), AttrSchema { index: true, ..Default::default() });
            s
        });
        for (e, age) in [(1, 20), (2, 25), (3, 30), (4, 35), (5, 40)] {
            db.with_datom(d(e, "age", Value::Long(age), 1));
        }

        let results = db.index_range(&kw("age"), &Value::Long(25), &Value::Long(35));
        let ages: Vec<i64> = results.iter().map(|d| match &d.v {
            Value::Long(n) => *n,
            _ => panic!("expected Long"),
        }).collect();
        assert_eq!(ages, vec![25, 30, 35]);
    }
}
