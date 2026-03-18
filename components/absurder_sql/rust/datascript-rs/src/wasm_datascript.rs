//! WasmDataScript — Rust DataScript database with PersistentSortedSet indexes.
//!
//! Combines schema routing logic (from db.rs) with real PSS indexes using
//! Datom keys. Provides store/restore/GC via UnifiedSQLiteStorage.

use std::collections::HashSet;

use js_sys;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn console_log(s: &str);
}

use persistent_sorted_set::comparator::{
    comparator_for_index, parse_index_type, value_cmp, IndexType,
};
use persistent_sorted_set::datom::{Attr, Datom, Value};
use persistent_sorted_set::datom_serde::{
    deserialize_metadata, deserialize_schema, serialize_metadata, serialize_schema, DbMetadata,
};
use persistent_sorted_set::schema::{
    AttrSchema, Cardinality, ReverseSchema, Schema, Unique, ValueType, build_rschema,
};
use persistent_sorted_set::set::PersistentSortedSet;
use persistent_sorted_set::settings::Settings;
use persistent_sorted_set::storage::IStorage;
use persistent_sorted_set::wasm::{attr_from_js, datom_from_js, datom_to_js, value_from_js, value_to_js};

use crate::unified_storage::UnifiedSQLiteStorage;

/// DataScript constants matching CLJS `db.cljc`.
const E0: i64 = 0;
const TX0: i64 = 0x20000000; // 536870912
const EMAX: i64 = 0x7FFFFFFF;
const TXMAX: i64 = 0x7FFFFFFF;

/// Metadata address in the pss_nodes table.
const METADATA_ADDR: i64 = 0;

// ---------------------------------------------------------------------------
// Schema parsing from JS
// ---------------------------------------------------------------------------

fn attr_from_keyword_str(s: &str) -> Attr {
    let inner = if s.starts_with(':') { &s[1..] } else { s };
    if let Some(idx) = inner.find('/') {
        Attr::Keyword {
            ns: Some(inner[..idx].to_string()),
            name: inner[idx + 1..].to_string(),
        }
    } else {
        Attr::Keyword {
            ns: None,
            name: inner.to_string(),
        }
    }
}

/// Parse a CLJS schema map into a Rust Schema.
///
/// CLJS schema format: `{:attr-keyword {:db/index true, :db/valueType :db.type/ref, ...}}`
fn schema_from_js(val: &JsValue) -> Schema {
    let mut schema = Schema::default();

    if val.is_null() || val.is_undefined() {
        return schema;
    }

    let entries = js_sys::Object::entries(&js_sys::Object::from(val.clone()));
    for i in 0..entries.length() {
        let entry = js_sys::Array::from(&entries.get(i));
        let key_js = entry.get(0);
        let val_js = entry.get(1);

        let key_str = match key_js.as_string() {
            Some(s) => s,
            None => continue,
        };
        let attr = attr_from_keyword_str(&key_str);
        let mut attr_schema = AttrSchema::default();

        if let Ok(idx_val) = js_sys::Reflect::get(&val_js, &JsValue::from_str(":db/index")) {
            if idx_val.as_bool() == Some(true) {
                attr_schema.index = true;
            }
        }

        if let Ok(vt) = js_sys::Reflect::get(&val_js, &JsValue::from_str(":db/valueType")) {
            if let Some(s) = vt.as_string() {
                if s == ":db.type/ref" {
                    attr_schema.value_type = Some(ValueType::Ref);
                }
            }
        }

        if let Ok(uniq) = js_sys::Reflect::get(&val_js, &JsValue::from_str(":db/unique")) {
            if let Some(s) = uniq.as_string() {
                match s.as_str() {
                    ":db.unique/identity" => attr_schema.unique = Some(Unique::Identity),
                    ":db.unique/value" => attr_schema.unique = Some(Unique::Value),
                    _ => {}
                }
            }
        }

        if let Ok(card) = js_sys::Reflect::get(&val_js, &JsValue::from_str(":db/cardinality")) {
            if let Some(s) = card.as_string() {
                if s == ":db.cardinality/many" {
                    attr_schema.cardinality = Cardinality::Many;
                }
            }
        }

        if let Ok(comp) = js_sys::Reflect::get(&val_js, &JsValue::from_str(":db/isComponent")) {
            if comp.as_bool() == Some(true) {
                attr_schema.is_component = true;
            }
        }

        schema.attrs.insert(attr, attr_schema);
    }

    schema
}

fn schema_to_js(schema: &Schema) -> JsValue {
    let obj = js_sys::Object::new();
    for (attr, attr_schema) in &schema.attrs {
        let key = match attr {
            Attr::Keyword { ns: Some(ns), name } => format!(":{}/{}", ns, name),
            Attr::Keyword { ns: None, name } => format!(":{}", name),
            Attr::Str(s) => s.clone(),
        };
        let props = js_sys::Object::new();

        if attr_schema.index {
            js_sys::Reflect::set(&props, &JsValue::from_str(":db/index"), &JsValue::TRUE).unwrap();
        }
        match &attr_schema.unique {
            Some(Unique::Identity) => {
                js_sys::Reflect::set(
                    &props,
                    &JsValue::from_str(":db/unique"),
                    &JsValue::from_str(":db.unique/identity"),
                )
                .unwrap();
            }
            Some(Unique::Value) => {
                js_sys::Reflect::set(
                    &props,
                    &JsValue::from_str(":db/unique"),
                    &JsValue::from_str(":db.unique/value"),
                )
                .unwrap();
            }
            None => {}
        }
        if attr_schema.cardinality == Cardinality::Many {
            js_sys::Reflect::set(
                &props,
                &JsValue::from_str(":db/cardinality"),
                &JsValue::from_str(":db.cardinality/many"),
            )
            .unwrap();
        }
        if attr_schema.value_type == Some(ValueType::Ref) {
            js_sys::Reflect::set(
                &props,
                &JsValue::from_str(":db/valueType"),
                &JsValue::from_str(":db.type/ref"),
            )
            .unwrap();
        }
        if attr_schema.is_component {
            js_sys::Reflect::set(&props, &JsValue::from_str(":db/isComponent"), &JsValue::TRUE)
                .unwrap();
        }

        js_sys::Reflect::set(&obj, &JsValue::from_str(&key), &props).unwrap();
    }
    obj.into()
}

// ---------------------------------------------------------------------------
// WasmDataScript
// ---------------------------------------------------------------------------

#[wasm_bindgen]
pub struct WasmDataScript {
    schema: Schema,
    rschema: ReverseSchema,
    eavt: PersistentSortedSet,
    aevt: PersistentSortedSet,
    avet: PersistentSortedSet,
    max_eid: i64,
    max_tx: i64,
    settings: Settings,
}

impl WasmDataScript {
    fn is_indexed(&self, attr: &Attr) -> bool {
        self.rschema.is_indexed(attr)
    }

    /// Public accessor for query module.
    pub fn is_indexed_pub(&self, attr: &Attr) -> bool {
        self.rschema.is_indexed(attr)
    }

    pub fn eavt(&self) -> &PersistentSortedSet {
        &self.eavt
    }

    pub fn aevt(&self) -> &PersistentSortedSet {
        &self.aevt
    }

    pub fn avet(&self) -> &PersistentSortedSet {
        &self.avet
    }

    pub fn schema_ref(&self) -> &Schema {
        &self.schema
    }

    pub fn rschema_ref(&self) -> &ReverseSchema {
        &self.rschema
    }

    fn advance_max_eid(&mut self, e: i64) {
        if e > self.max_eid {
            self.max_eid = e;
        }
    }

    fn with_datom_internal(mut self, datom: Datom) -> Self {
        let attr = datom.a.as_ref().expect("datom must have attribute");
        let indexing = self.is_indexed(attr);

        if datom.tx > 0 {
            self.eavt = self.eavt.conj(&datom);
            self.aevt = self.aevt.conj(&datom);
            if indexing {
                self.avet = self.avet.conj(&datom);
            }
            self.advance_max_eid(datom.e);
        } else {
            // Retracting — construct the positive version to find and remove
            let pos_datom = Datom::new(datom.e, datom.a.clone(), datom.v.clone(), -datom.tx);
            if self.eavt.contains(&pos_datom) {
                self.eavt = self.eavt.disj(&pos_datom);
                self.aevt = self.aevt.disj(&pos_datom);
                if indexing {
                    self.avet = self.avet.disj(&pos_datom);
                }
            }
        }
        self
    }

    /// Collect datoms from PSS slice into a JS array.
    fn slice_to_array(
        pss: &PersistentSortedSet,
        from: &Datom,
        to: &Datom,
    ) -> js_sys::Array {
        let arr = js_sys::Array::new();
        if let Some(seq) = pss.slice(Some(from), Some(to)) {
            for key in seq.to_vec() {
                arr.push(&datom_to_js(&key));
            }
        }
        arr
    }

    /// Collect all datoms from PSS into a JS array.
    fn all_to_array(pss: &PersistentSortedSet) -> js_sys::Array {
        let arr = js_sys::Array::new();
        for key in pss.to_vec() {
            arr.push(&datom_to_js(&key));
        }
        arr
    }

    /// Slice + filter by value match.
    fn slice_filter_v(
        pss: &PersistentSortedSet,
        from: &Datom,
        to: &Datom,
        v: &Value,
    ) -> js_sys::Array {
        let arr = js_sys::Array::new();
        if let Some(seq) = pss.slice(Some(from), Some(to)) {
            for key in seq.to_vec() {
                if value_cmp(&key.v, v) == std::cmp::Ordering::Equal {
                    arr.push(&datom_to_js(&key));
                }
            }
        }
        arr
    }

    /// Slice + filter by tx.
    fn slice_filter_tx(
        pss: &PersistentSortedSet,
        from: &Datom,
        to: &Datom,
        tx: i64,
    ) -> js_sys::Array {
        let arr = js_sys::Array::new();
        if let Some(seq) = pss.slice(Some(from), Some(to)) {
            for key in seq.to_vec() {
                if key.tx_id() == tx {
                    arr.push(&datom_to_js(&key));
                }
            }
        }
        arr
    }

    /// Slice + filter by value and tx.
    fn slice_filter_v_tx(
        pss: &PersistentSortedSet,
        from: &Datom,
        to: &Datom,
        v: &Value,
        tx: i64,
    ) -> js_sys::Array {
        let arr = js_sys::Array::new();
        if let Some(seq) = pss.slice(Some(from), Some(to)) {
            for key in seq.to_vec() {
                if value_cmp(&key.v, v) == std::cmp::Ordering::Equal && key.tx_id() == tx {
                    arr.push(&datom_to_js(&key));
                }
            }
        }
        arr
    }

    /// Full scan + filter by value.
    fn all_filter_v(pss: &PersistentSortedSet, v: &Value) -> js_sys::Array {
        let arr = js_sys::Array::new();
        for key in pss.to_vec() {
            if value_cmp(&key.v, v) == std::cmp::Ordering::Equal {
                arr.push(&datom_to_js(&key));
            }
        }
        arr
    }

    /// Full scan + filter by tx.
    fn all_filter_tx(pss: &PersistentSortedSet, tx: i64) -> js_sys::Array {
        let arr = js_sys::Array::new();
        for key in pss.to_vec() {
            if key.tx_id() == tx {
                arr.push(&datom_to_js(&key));
            }
        }
        arr
    }

    /// Full scan + filter by value and tx.
    fn all_filter_v_tx(pss: &PersistentSortedSet, v: &Value, tx: i64) -> js_sys::Array {
        let arr = js_sys::Array::new();
        for key in pss.to_vec() {
            if value_cmp(&key.v, v) == std::cmp::Ordering::Equal && key.tx_id() == tx {
                arr.push(&datom_to_js(&key));
            }
        }
        arr
    }
}

// ---------------------------------------------------------------------------
// TransactableDB impl
// ---------------------------------------------------------------------------

impl persistent_sorted_set::transact::TransactableDB for WasmDataScript {
    fn search_eav(&self, e: i64, a: &Attr, v: &Value) -> Option<Datom> {
        crate::query::search_internal(self, Some(e), Some(a), Some(v), None)
            .into_iter()
            .next()
    }

    fn search_ea(&self, e: i64, a: &Attr) -> Vec<Datom> {
        crate::query::search_internal(self, Some(e), Some(a), None, None)
    }

    fn search_e(&self, e: i64) -> Vec<Datom> {
        crate::query::search_internal(self, Some(e), None, None, None)
    }

    fn search_av(&self, a: &Attr, v: &Value) -> Vec<Datom> {
        crate::query::search_internal(self, None, Some(a), Some(v), None)
    }

    fn search_a_refs(&self, a: &Attr, v_ref: i64) -> Vec<Datom> {
        crate::query::search_internal(self, None, Some(a), Some(&Value::Ref(v_ref)), None)
    }

    fn apply_datom(&mut self, datom: Datom) {
        let attr = datom.a.as_ref().expect("datom must have attribute");
        let indexing = self.is_indexed(attr);

        if datom.tx > 0 {
            // Adding — use std::mem::take to handle PSS's consuming conj
            let eavt = std::mem::replace(
                &mut self.eavt,
                PersistentSortedSet::empty(comparator_for_index(IndexType::EAVT)),
            );
            self.eavt = eavt.conj(&datom);

            let aevt = std::mem::replace(
                &mut self.aevt,
                PersistentSortedSet::empty(comparator_for_index(IndexType::AEVT)),
            );
            self.aevt = aevt.conj(&datom);

            if indexing {
                let avet = std::mem::replace(
                    &mut self.avet,
                    PersistentSortedSet::empty(comparator_for_index(IndexType::AVET)),
                );
                self.avet = avet.conj(&datom);
            }
            self.advance_max_eid(datom.e);
        } else {
            // Retracting — construct positive version to find and remove
            let pos_datom = Datom::new(datom.e, datom.a.clone(), datom.v.clone(), -datom.tx);
            if self.eavt.contains(&pos_datom) {
                let eavt = std::mem::replace(
                    &mut self.eavt,
                    PersistentSortedSet::empty(comparator_for_index(IndexType::EAVT)),
                );
                self.eavt = eavt.disj(&pos_datom);

                let aevt = std::mem::replace(
                    &mut self.aevt,
                    PersistentSortedSet::empty(comparator_for_index(IndexType::AEVT)),
                );
                self.aevt = aevt.disj(&pos_datom);

                if indexing {
                    let avet = std::mem::replace(
                        &mut self.avet,
                        PersistentSortedSet::empty(comparator_for_index(IndexType::AVET)),
                    );
                    self.avet = avet.disj(&pos_datom);
                }
            }
        }
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

// ---------------------------------------------------------------------------
// wasm_bindgen exports
// ---------------------------------------------------------------------------

#[wasm_bindgen]
impl WasmDataScript {
    /// Create an empty DataScript database with the given schema.
    #[wasm_bindgen(js_name = "emptyDb")]
    pub fn empty_db(schema_js: JsValue) -> WasmDataScript {
        let schema = schema_from_js(&schema_js);
        let rschema = build_rschema(&schema);
        let settings = Settings::new(512);

        WasmDataScript {
            schema,
            rschema,
            eavt: PersistentSortedSet::empty(comparator_for_index(IndexType::EAVT)),
            aevt: PersistentSortedSet::empty(comparator_for_index(IndexType::AEVT)),
            avet: PersistentSortedSet::empty(comparator_for_index(IndexType::AVET)),
            max_eid: E0,
            max_tx: TX0,
            settings,
        }
    }

    /// Add datoms to the database. Each element is a JS object with {e, a, v, tx}.
    /// Positive tx = assert, negative tx = retract.
    /// Returns a new WasmDataScript (persistent/immutable).
    #[wasm_bindgen(js_name = "withDatoms")]
    pub fn with_datoms(mut self, datoms_array: js_sys::Array) -> WasmDataScript {
        let len = datoms_array.length();
        console_log(&format!("withDatoms: {} datoms", len));
        for i in 0..len {
            let datom = datom_from_js(&datoms_array.get(i));
            if i < 10 {
                console_log(&format!(
                    "  withDatoms[{}]: e={} a={:?} v={:?} tx={}",
                    i, datom.e, datom.a, datom.v, datom.tx
                ));
            }
            self = self.with_datom_internal(datom);
        }
        self
    }

    /// Ultra-fast bulk load from a tab/newline-delimited string.
    /// Format per line: `e\tattr\tv_type\tv_data\ttx`
    /// Parses all datoms in pure Rust, sorts per index, builds trees bottom-up.
    /// Zero JS boundary crossings.
    #[wasm_bindgen(js_name = "transactBulkString")]
    pub fn transact_bulk_string(&mut self, data: &str) {
        use persistent_sorted_set::datom::{Attr, Datom, Value};
        use persistent_sorted_set::comparator::{cmp_datoms, IndexType};

        let mut all_datoms: Vec<Datom> = Vec::with_capacity(data.len() / 30);
        let mut max_eid: i64 = self.max_eid;

        for line in data.split('\n') {
            if line.is_empty() { continue; }
            let mut parts = line.splitn(5, '\t');
            let e: i64 = parts.next().unwrap_or("0").parse().unwrap_or(0);
            let a_str = parts.next().unwrap_or("");
            let v_type = parts.next().unwrap_or("s");
            let v_data = parts.next().unwrap_or("");
            let tx: i64 = parts.next().unwrap_or("0").parse().unwrap_or(0);

            let attr = if a_str.is_empty() {
                None
            } else if let Some(idx) = a_str.find('/') {
                Some(Attr::Keyword {
                    ns: Some(a_str[..idx].to_string()),
                    name: a_str[idx + 1..].to_string(),
                })
            } else {
                Some(Attr::Keyword { ns: None, name: a_str.to_string() })
            };

            let val = match v_type {
                "n" => {
                    let n: f64 = v_data.parse().unwrap_or(0.0);
                    if n.fract() == 0.0 && n >= i64::MIN as f64 && n <= i64::MAX as f64 {
                        Value::Long(n as i64)
                    } else {
                        Value::Double(n)
                    }
                }
                "r" => Value::Ref(v_data.parse().unwrap_or(0)),
                "k" => {
                    if let Some(idx) = v_data.find('/') {
                        Value::Keyword(Attr::Keyword {
                            ns: Some(v_data[..idx].to_string()),
                            name: v_data[idx + 1..].to_string(),
                        })
                    } else {
                        Value::Keyword(Attr::Keyword { ns: None, name: v_data.to_string() })
                    }
                }
                "b" => Value::Bool(v_data == "true" || v_data == "1"),
                _ => Value::Str(v_data.to_string()),
            };

            if e > max_eid { max_eid = e; }
            all_datoms.push(Datom::new(e, attr, val, tx));
        }

        self.max_eid = max_eid;

        console_log(&format!(
            "transactBulkString: {} datoms parsed, max_eid={}",
            all_datoms.len(), max_eid
        ));
        for (i, d) in all_datoms.iter().enumerate().take(20) {
            console_log(&format!(
                "  datom[{}]: e={} a={:?} v={:?} tx={}",
                i, d.e, d.a, d.v, d.tx
            ));
        }

        // Build EAVT: sort + bottom-up tree
        let mut eavt_datoms = all_datoms.clone();
        eavt_datoms.sort_by(|a, b| cmp_datoms(IndexType::EAVT, a, b));
        self.eavt = PersistentSortedSet::from_sorted(
            eavt_datoms, comparator_for_index(IndexType::EAVT));

        // Build AEVT: sort + bottom-up tree
        let mut aevt_datoms = all_datoms.clone();
        aevt_datoms.sort_by(|a, b| cmp_datoms(IndexType::AEVT, a, b));
        self.aevt = PersistentSortedSet::from_sorted(
            aevt_datoms, comparator_for_index(IndexType::AEVT));

        // Build AVET: filter indexed attrs, sort + bottom-up tree
        let avet_datoms: Vec<Datom> = all_datoms.into_iter()
            .filter(|d| d.a.as_ref().map_or(false, |a| self.is_indexed(a)))
            .collect();
        let mut avet_sorted = avet_datoms;
        avet_sorted.sort_by(|a, b| cmp_datoms(IndexType::AVET, a, b));
        self.avet = PersistentSortedSet::from_sorted(
            avet_sorted, comparator_for_index(IndexType::AVET));
    }

    /// Count of datoms in the EAVT index.
    pub fn count(&self) -> usize {
        self.eavt.count()
    }

    /// Maximum entity id.
    #[wasm_bindgen(js_name = "maxEid")]
    pub fn max_eid(&self) -> f64 {
        self.max_eid as f64
    }

    /// Maximum transaction id.
    #[wasm_bindgen(js_name = "maxTx")]
    pub fn max_tx(&self) -> f64 {
        self.max_tx as f64
    }

    /// Get the schema as a JS object.
    pub fn schema(&self) -> JsValue {
        schema_to_js(&self.schema)
    }

    /// Search by pattern [e, a, v, tx]. Null/undefined = wildcard.
    /// Returns a JS Array of datom objects.
    pub fn search(
        &self,
        e: JsValue,
        a: JsValue,
        v: JsValue,
        tx: JsValue,
    ) -> js_sys::Array {
        let e_opt = e.as_f64().map(|n| n as i64);
        let a_opt = parse_attr_arg(&a);
        let v_opt = parse_value_arg(&v);
        let tx_opt = tx.as_f64().map(|n| n as i64);

        match (e_opt, a_opt.as_ref(), v_opt.as_ref(), tx_opt) {
            (Some(e), Some(a), Some(v), Some(tx)) => {
                let d = Datom::new(e, Some(a.clone()), v.clone(), tx);
                Self::slice_to_array(&self.eavt, &d, &d)
            }
            (Some(e), Some(a), Some(v), None) => {
                let from = Datom::new(e, Some(a.clone()), v.clone(), TX0);
                let to = Datom::new(e, Some(a.clone()), v.clone(), TXMAX);
                Self::slice_to_array(&self.eavt, &from, &to)
            }
            (Some(e), Some(a), None, None) => {
                let from = Datom::new(e, Some(a.clone()), Value::Nil, TX0);
                let to = Datom::new(e, Some(a.clone()), Value::Nil, TXMAX);
                Self::slice_to_array(&self.eavt, &from, &to)
            }
            (Some(e), Some(a), None, Some(tx)) => {
                let from = Datom::new(e, Some(a.clone()), Value::Nil, TX0);
                let to = Datom::new(e, Some(a.clone()), Value::Nil, TXMAX);
                Self::slice_filter_tx(&self.eavt, &from, &to, tx)
            }
            (Some(e), None, None, None) => {
                let from = Datom::new(e, None, Value::Nil, TX0);
                let to = Datom::new(e, None, Value::Nil, TXMAX);
                Self::slice_to_array(&self.eavt, &from, &to)
            }
            (Some(e), None, Some(v), None) => {
                let from = Datom::new(e, None, Value::Nil, TX0);
                let to = Datom::new(e, None, Value::Nil, TXMAX);
                Self::slice_filter_v(&self.eavt, &from, &to, v)
            }
            (Some(e), None, None, Some(tx)) => {
                let from = Datom::new(e, None, Value::Nil, TX0);
                let to = Datom::new(e, None, Value::Nil, TXMAX);
                Self::slice_filter_tx(&self.eavt, &from, &to, tx)
            }
            (Some(e), None, Some(v), Some(tx)) => {
                let from = Datom::new(e, None, Value::Nil, TX0);
                let to = Datom::new(e, None, Value::Nil, TXMAX);
                Self::slice_filter_v_tx(&self.eavt, &from, &to, v, tx)
            }
            (None, Some(a), Some(v), None) => {
                if self.is_indexed(a) {
                    let from = Datom::new(E0, Some(a.clone()), v.clone(), TX0);
                    let to = Datom::new(EMAX, Some(a.clone()), v.clone(), TXMAX);
                    Self::slice_to_array(&self.avet, &from, &to)
                } else {
                    let from = Datom::new(E0, Some(a.clone()), Value::Nil, TX0);
                    let to = Datom::new(EMAX, Some(a.clone()), Value::Nil, TXMAX);
                    Self::slice_filter_v(&self.aevt, &from, &to, v)
                }
            }
            (None, Some(a), Some(v), Some(tx)) => {
                if self.is_indexed(a) {
                    let from = Datom::new(E0, Some(a.clone()), v.clone(), TX0);
                    let to = Datom::new(EMAX, Some(a.clone()), v.clone(), TXMAX);
                    Self::slice_filter_tx(&self.avet, &from, &to, tx)
                } else {
                    let from = Datom::new(E0, Some(a.clone()), Value::Nil, TX0);
                    let to = Datom::new(EMAX, Some(a.clone()), Value::Nil, TXMAX);
                    Self::slice_filter_v_tx(&self.aevt, &from, &to, v, tx)
                }
            }
            (None, Some(a), None, None) => {
                let from = Datom::new(E0, Some(a.clone()), Value::Nil, TX0);
                let to = Datom::new(EMAX, Some(a.clone()), Value::Nil, TXMAX);
                Self::slice_to_array(&self.aevt, &from, &to)
            }
            (None, Some(a), None, Some(tx)) => {
                let from = Datom::new(E0, Some(a.clone()), Value::Nil, TX0);
                let to = Datom::new(EMAX, Some(a.clone()), Value::Nil, TXMAX);
                Self::slice_filter_tx(&self.aevt, &from, &to, tx)
            }
            (None, None, Some(v), None) => {
                Self::all_filter_v(&self.eavt, v)
            }
            (None, None, Some(v), Some(tx)) => {
                Self::all_filter_v_tx(&self.eavt, v, tx)
            }
            (None, None, None, Some(tx)) => {
                Self::all_filter_tx(&self.eavt, tx)
            }
            (None, None, None, None) => {
                Self::all_to_array(&self.eavt)
            }
        }
    }

    /// Get datoms from a named index with optional from/to bounds.
    /// Pass null for unbounded scan.
    #[wasm_bindgen(js_name = "datomsIndex")]
    pub fn datoms_index(
        &self,
        index: String,
        from_e: JsValue, from_a: JsValue, from_v: JsValue, from_tx: JsValue,
        to_e: JsValue, to_a: JsValue, to_v: JsValue, to_tx: JsValue,
    ) -> js_sys::Array {
        let idx = parse_index_type(&index);
        let pss = match idx {
            IndexType::EAVT => &self.eavt,
            IndexType::AEVT => &self.aevt,
            IndexType::AVET => &self.avet,
        };

        let has_from = !from_e.is_null() && !from_e.is_undefined();
        let has_to = !to_e.is_null() && !to_e.is_undefined();

        match (has_from, has_to) {
            (true, true) => {
                let from = make_datom_from_components(&from_e, &from_a, &from_v, &from_tx);
                let to = make_datom_from_components(&to_e, &to_a, &to_v, &to_tx);
                Self::slice_to_array(pss, &from, &to)
            }
            (true, false) => {
                let from = make_datom_from_components(&from_e, &from_a, &from_v, &from_tx);
                let arr = js_sys::Array::new();
                if let Some(seq) = pss.slice(Some(&from), None) {
                    for key in seq.to_vec() {
                        arr.push(&datom_to_js(&key));
                    }
                }
                arr
            }
            (false, true) => {
                let to = make_datom_from_components(&to_e, &to_a, &to_v, &to_tx);
                let arr = js_sys::Array::new();
                if let Some(seq) = pss.slice(None, Some(&to)) {
                    for key in seq.to_vec() {
                        arr.push(&datom_to_js(&key));
                    }
                }
                arr
            }
            (false, false) => {
                Self::all_to_array(pss)
            }
        }
    }

    /// Resolve WHERE pattern clauses entirely in Rust and return result tuples.
    ///
    /// `patterns` is a JS Array of pattern arrays. Each pattern is `[e, a, v, tx]`
    /// where variables are strings starting with "?" and constants are concrete values.
    /// The string "_" is a wildcard (match but don't bind).
    ///
    /// `find_vars` is a JS Array of variable name strings (e.g. `["?name", "?age"]`).
    ///
    /// Returns a JS Array of result tuples (each tuple is a JS Array of values).
    /// All pattern lookups and joins happen in Rust — no per-clause WASM boundary crossings.
    #[wasm_bindgen(js_name = "queryPatterns")]
    pub fn query_patterns(
        &self,
        patterns: js_sys::Array,
        find_vars: js_sys::Array,
    ) -> js_sys::Array {
        use crate::query::{resolve_patterns, collect_results};
        use persistent_sorted_set::relation::Var;

        // Parse patterns from JS
        let rust_patterns = parse_patterns_js(&patterns);

        // Parse find vars
        let rust_find: Vec<Var> = (0..find_vars.length())
            .filter_map(|i| find_vars.get(i).as_string())
            .collect();

        // Resolve patterns + join entirely in Rust
        let result_rel = resolve_patterns(self, &rust_patterns);
        let result_tuples = collect_results(&result_rel, &rust_find);

        tuples_to_js(&result_tuples)
    }

    /// Resolve a full Datalog query with clauses (including rule calls) and rules.
    ///
    /// `clauses_js` is a JS Array of clause objects. Each clause is one of:
    /// - `{type: "pattern", pattern: [e, a, v, tx]}` — pattern clause
    /// - `{type: "rule", name: "ruleName", args: [...]}` — rule call
    /// - `{type: "predicate", name: "predicateName", args: [...]}` — predicate filter
    /// - `{type: "and", clauses: [...]}` — conjunction
    /// - `{type: "or", branches: [[...], [...]]}` — disjunction (array of clause arrays)
    /// - `{type: "not", clauses: [...]}` — negation
    ///
    /// `rules_js` is a JS Object mapping rule names to arrays of branch objects:
    /// `{"ruleName": [{head: ["?a", "?b"], body: [...clauses...]}]}`
    ///
    /// `find_vars` is a JS Array of variable name strings.
    ///
    /// Returns a JS Array of result tuples.
    #[wasm_bindgen(js_name = "queryClauses")]
    pub fn query_clauses(
        &self,
        clauses_js: js_sys::Array,
        rules_js: JsValue,
        find_vars: js_sys::Array,
    ) -> js_sys::Array {
        use crate::query::resolve_clauses_with_rules;
        use persistent_sorted_set::relation::Var;

        let clauses = parse_clauses_js(&clauses_js);
        let rules = parse_rules_js(&rules_js);
        let rust_find: Vec<Var> = (0..find_vars.length())
            .filter_map(|i| find_vars.get(i).as_string())
            .collect();

        let result_tuples = resolve_clauses_with_rules(self, &clauses, &rules, &rust_find);
        tuples_to_js(&result_tuples)
    }

    /// Execute a Datalog query from an EDN string, with optional rules and inputs.
    ///
    /// `query_edn` is the query as an EDN string:
    /// ```
    /// "[:find ?name ?age :in $ % ?min :where [?e :name ?name] [?e :age ?age] [(>= ?age ?min)]]"
    /// ```
    ///
    /// `rules_edn` is the rules as an EDN string (pass empty string or null for no rules):
    /// ```
    /// "[[(subgroup ?g ?s) [?g :groups ?s]] [(subgroup ?g ?s) [?g :groups ?x] (subgroup ?x ?s)]]"
    /// ```
    ///
    /// `inputs` is a JS Array of `[name, value]` pairs for `:in` variables:
    /// ```
    /// [["?min", 21], ["?uuid", "abc-123"]]
    /// ```
    ///
    /// Returns a JS value shaped according to the `:find` spec:
    /// - `:find ?a ?b` (rel) → `[[v1, v2], [v3, v4], ...]`
    /// - `:find ?a .` (scalar) → `v` or `null`
    /// - `:find [?a ...]` (coll) → `[v1, v2, ...]`
    /// - `:find [?a ?b]` (tuple) → `[v1, v2]` or `null`
    #[wasm_bindgen(js_name = "queryEdn")]
    pub fn query_edn(
        &self,
        query_edn: String,
        rules_edn: JsValue,
        inputs: JsValue,
    ) -> JsValue {
        use crate::query::resolve_clauses_with_rules;
        use persistent_sorted_set::pull;
        use persistent_sorted_set::pull_parser::parse_pull_pattern;
        use persistent_sorted_set::query_parser::{
            parse_query, parse_rules, bind_inputs, FindSpec, FindElement,
        };
        let mut parsed = parse_query(&query_edn);

        // Parse rules
        let rules = if let Some(s) = rules_edn.as_string() {
            if s.is_empty() {
                parsed.rules.clone()
            } else {
                parse_rules(&s)
            }
        } else {
            parsed.rules.clone()
        };

        // Bind input parameters — split into scalar bindings (pattern substitution)
        // and collection bindings (initial relations).
        use persistent_sorted_set::query_parser::{InBinding, build_collection_relations};
        use crate::query::resolve_clauses_with_rules_and_initial;

        let mut scalar_bindings: Vec<(String, Value)> = Vec::new();
        let mut coll_values: std::collections::HashMap<String, Vec<Value>> = std::collections::HashMap::new();

        if !inputs.is_null() && !inputs.is_undefined() {
            let inputs_arr = js_sys::Array::from(&inputs);
            let mut input_idx = 0;

            for binding in &parsed.in_bindings {
                if input_idx >= inputs_arr.length() { break; }
                match binding {
                    InBinding::Scalar(_var) => {
                        let pair = js_sys::Array::from(&inputs_arr.get(input_idx));
                        if pair.length() >= 2 {
                            if let Some(name) = pair.get(0).as_string() {
                                let val = parse_input_value(&pair.get(1));
                                scalar_bindings.push((name, val));
                            }
                        }
                        input_idx += 1;
                    }
                    InBinding::Coll(var) => {
                        let pair = js_sys::Array::from(&inputs_arr.get(input_idx));
                        if pair.length() >= 2 {
                            let val_js = pair.get(1);
                            if js_sys::Array::is_array(&val_js) {
                                let arr = js_sys::Array::from(&val_js);
                                let values: Vec<Value> = (0..arr.length())
                                    .map(|j| parse_input_value(&arr.get(j)))
                                    .collect();
                                coll_values.insert(var.clone(), values);
                            } else {
                                // Single value — treat as 1-element collection
                                coll_values.insert(var.clone(), vec![parse_input_value(&val_js)]);
                            }
                        }
                        input_idx += 1;
                    }
                    InBinding::Tuple(vars) => {
                        let pair = js_sys::Array::from(&inputs_arr.get(input_idx));
                        if pair.length() >= 2 {
                            let val_js = pair.get(1);
                            if js_sys::Array::is_array(&val_js) {
                                let arr = js_sys::Array::from(&val_js);
                                for (j, tv) in vars.iter().enumerate() {
                                    if (j as u32) < arr.length() {
                                        scalar_bindings.push((tv.clone(), parse_input_value(&arr.get(j as u32))));
                                    }
                                }
                            }
                        }
                        input_idx += 1;
                    }
                }
            }

            // If no in_bindings parsed (old-style queries), fall back to treating all as scalars
            if parsed.in_bindings.is_empty() {
                let inputs_arr = js_sys::Array::from(&inputs);
                for i in 0..inputs_arr.length() {
                    let pair = js_sys::Array::from(&inputs_arr.get(i));
                    if pair.length() >= 2 {
                        if let Some(name) = pair.get(0).as_string() {
                            let val = parse_input_value(&pair.get(1));
                            scalar_bindings.push((name, val));
                        }
                    }
                }
            }
        }

        // Apply scalar bindings (pattern substitution)
        if !scalar_bindings.is_empty() {
            let binding_refs: Vec<(&str, Value)> = scalar_bindings
                .iter()
                .map(|(k, v)| (k.as_str(), v.clone()))
                .collect();
            bind_inputs(&mut parsed, &binding_refs);
        }

        // Build initial relations for collection bindings
        let initial_rels = build_collection_relations(&parsed.in_bindings, &coll_values);

        let find_vars = parsed.find.vars();
        let result_tuples = if initial_rels.is_empty() {
            resolve_clauses_with_rules(
                self,
                &parsed.where_clauses,
                &rules,
                &find_vars,
            )
        } else {
            resolve_clauses_with_rules_and_initial(
                self,
                &parsed.where_clauses,
                &rules,
                &find_vars,
                initial_rels,
            )
        };

        // Apply aggregation if any find elements are aggregates
        let result_tuples = if parsed.has_aggregates() {
            persistent_sorted_set::aggregates::aggregate(
                &parsed.find_elements,
                result_tuples,
            )
        } else {
            result_tuples
        };

        let has_pull = parsed.has_pull_in_find();

        // Pre-parse pull patterns if needed
        let pull_patterns: Vec<Option<persistent_sorted_set::pull_parser::PullPattern>> =
            if has_pull {
                parsed.find_elements.iter().map(|fe| {
                    if let FindElement::Pull { pattern_edn, .. } = fe {
                        Some(parse_pull_pattern(&self.schema, &self.rschema, pattern_edn))
                    } else {
                        None
                    }
                }).collect()
            } else {
                vec![]
            };

        // Convert a single tuple element to JS, applying pull if needed
        let element_to_js = |col: usize, val: &Value| -> JsValue {
            if has_pull {
                if let Some(Some(pattern)) = pull_patterns.get(col) {
                    // Value is an entity ID — pull it
                    let eid = match val {
                        Value::Long(n) | Value::Ref(n) => *n,
                        _ => return value_to_js(val),
                    };
                    return match pull::pull(self, pattern, eid) {
                        Some(pr) => pull_result_to_js(&pr),
                        None => JsValue::NULL,
                    };
                }
            }
            value_to_js(val)
        };

        // Shape output according to find spec
        match &parsed.find {
            FindSpec::Rel(_) => {
                let outer = js_sys::Array::new_with_length(result_tuples.len() as u32);
                for (i, tuple) in result_tuples.iter().enumerate() {
                    let inner = js_sys::Array::new_with_length(tuple.len() as u32);
                    for (j, val) in tuple.iter().enumerate() {
                        inner.set(j as u32, element_to_js(j, val));
                    }
                    outer.set(i as u32, inner.into());
                }
                outer.into()
            }
            FindSpec::Scalar(_) => {
                if let Some(tuple) = result_tuples.first() {
                    if let Some(val) = tuple.first() {
                        element_to_js(0, val)
                    } else {
                        JsValue::NULL
                    }
                } else {
                    JsValue::NULL
                }
            }
            FindSpec::Coll(_) => {
                let arr = js_sys::Array::new_with_length(result_tuples.len() as u32);
                for (i, tuple) in result_tuples.iter().enumerate() {
                    if let Some(val) = tuple.first() {
                        arr.set(i as u32, element_to_js(0, val));
                    }
                }
                arr.into()
            }
            FindSpec::Tuple(_) => {
                if let Some(tuple) = result_tuples.first() {
                    let arr = js_sys::Array::new_with_length(tuple.len() as u32);
                    for (j, val) in tuple.iter().enumerate() {
                        arr.set(j as u32, element_to_js(j, val));
                    }
                    arr.into()
                } else {
                    JsValue::NULL
                }
            }
        }
    }

    /// Execute a multi-source Datalog query. `self` is the default source (`$`).
    /// Additional sources are passed as a JS object: `{ "$ws": anotherWasmDataScript }`.
    ///
    /// Used for behave-style queries: `[:find ?x :in $ $ws % :where [$ws ?e :name ?x]]`
    /// Query with multiple database sources.
    /// `self` is the default ($) source. `source_name` + `source_db` provide one
    /// additional source (e.g. "$ws" + workspace DB). Pass empty string/null for
    /// single-source queries (falls back to queryEdn behavior).
    #[wasm_bindgen(js_name = "queryEdnMulti")]
    pub fn query_edn_multi(
        &self,
        query_edn: String,
        rules_edn: JsValue,
        inputs: JsValue,
        source_name: String,
        source_db: Option<WasmDataScript>,
    ) -> JsValue {
        use persistent_sorted_set::query_parser::{
            parse_query, parse_rules, bind_inputs, FindSpec, InBinding,
            build_collection_relations,
        };
        use persistent_sorted_set::relation::{
            resolve_query_with_initial, MultiResolver,
        };

        let mut parsed = parse_query(&query_edn);

        // Parse rules
        let rules = if let Some(s) = rules_edn.as_string() {
            if s.is_empty() { parsed.rules.clone() } else { parse_rules(&s) }
        } else {
            parsed.rules.clone()
        };

        // Build multi-resolver from sources object
        // We need to extract WasmDataScript references from the JS sources object.
        // Since wasm_bindgen doesn't support passing borrowed references easily,
        // we'll resolve all additional sources upfront.
        //
        // For now, the multi-resolver approach works with the PatternResolver trait.
        // The `self` is the default ($) source. Additional sources from JS would
        // need their own WasmDataScript instances — but wasm_bindgen can't pass
        // &WasmDataScript from JS. Instead, we'll create a QueryMultiContext on
        // the JS side that handles source dispatch.
        //
        // Simpler approach: parse the query to find which patterns use which source,
        // then resolve $ws patterns by calling a separate queryEdn on the $ws db
        // from JS. This avoids the ownership problem.
        //
        // For the Rust-only path (native tests), MultiResolver works directly.
        // For WASM, we provide queryEdnMulti as a convenience that the CLJS layer
        // can use by passing additional WasmDataScript instances.

        // Process inputs (same as queryEdn)
        let mut scalar_bindings: Vec<(String, Value)> = Vec::new();
        let mut coll_values: std::collections::HashMap<String, Vec<Value>> = std::collections::HashMap::new();

        if !inputs.is_null() && !inputs.is_undefined() {
            let inputs_arr = js_sys::Array::from(&inputs);
            let mut input_idx = 0;
            for binding in &parsed.in_bindings {
                if input_idx >= inputs_arr.length() { break; }
                match binding {
                    InBinding::Scalar(_) => {
                        let pair = js_sys::Array::from(&inputs_arr.get(input_idx));
                        if pair.length() >= 2 {
                            if let Some(name) = pair.get(0).as_string() {
                                scalar_bindings.push((name, parse_input_value(&pair.get(1))));
                            }
                        }
                        input_idx += 1;
                    }
                    InBinding::Coll(var) => {
                        let pair = js_sys::Array::from(&inputs_arr.get(input_idx));
                        if pair.length() >= 2 {
                            let val_js = pair.get(1);
                            if js_sys::Array::is_array(&val_js) {
                                let arr = js_sys::Array::from(&val_js);
                                let values: Vec<Value> = (0..arr.length())
                                    .map(|j| parse_input_value(&arr.get(j)))
                                    .collect();
                                coll_values.insert(var.clone(), values);
                            }
                        }
                        input_idx += 1;
                    }
                    InBinding::Tuple(vars) => {
                        let pair = js_sys::Array::from(&inputs_arr.get(input_idx));
                        if pair.length() >= 2 {
                            let val_js = pair.get(1);
                            if js_sys::Array::is_array(&val_js) {
                                let arr = js_sys::Array::from(&val_js);
                                for (j, tv) in vars.iter().enumerate() {
                                    if (j as u32) < arr.length() {
                                        scalar_bindings.push((tv.clone(), parse_input_value(&arr.get(j as u32))));
                                    }
                                }
                            }
                        }
                        input_idx += 1;
                    }
                }
            }
            if parsed.in_bindings.is_empty() {
                let inputs_arr = js_sys::Array::from(&inputs);
                for i in 0..inputs_arr.length() {
                    let pair = js_sys::Array::from(&inputs_arr.get(i));
                    if pair.length() >= 2 {
                        if let Some(name) = pair.get(0).as_string() {
                            scalar_bindings.push((name, parse_input_value(&pair.get(1))));
                        }
                    }
                }
            }
        }

        if !scalar_bindings.is_empty() {
            let binding_refs: Vec<(&str, Value)> = scalar_bindings
                .iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
            bind_inputs(&mut parsed, &binding_refs);
        }

        let initial_rels = build_collection_relations(&parsed.in_bindings, &coll_values);

        // Build multi-resolver: self = default ($), optional additional source.
        let mut multi = MultiResolver::new(self);
        if !source_name.is_empty() {
            if let Some(ref db) = source_db {
                multi.add_source(source_name, db);
            }
        }

        let find_vars = parsed.find.vars();
        let result = resolve_query_with_initial(
            &multi, &parsed.where_clauses, &rules, initial_rels,
        );
        let result_tuples = crate::query::collect_results(&result, &find_vars);

        // Apply aggregation
        let result_tuples = if parsed.has_aggregates() {
            persistent_sorted_set::aggregates::aggregate(&parsed.find_elements, result_tuples)
        } else {
            result_tuples
        };

        // Shape output (same as queryEdn)
        match &parsed.find {
            FindSpec::Rel(_) => {
                let outer = js_sys::Array::new_with_length(result_tuples.len() as u32);
                for (i, tuple) in result_tuples.iter().enumerate() {
                    let inner = js_sys::Array::new_with_length(tuple.len() as u32);
                    for (j, val) in tuple.iter().enumerate() {
                        inner.set(j as u32, value_to_js(val));
                    }
                    outer.set(i as u32, inner.into());
                }
                outer.into()
            }
            FindSpec::Scalar(_) => {
                result_tuples.first()
                    .and_then(|t| t.first())
                    .map(|v| value_to_js(v))
                    .unwrap_or(JsValue::NULL)
            }
            FindSpec::Coll(_) => {
                let arr = js_sys::Array::new_with_length(result_tuples.len() as u32);
                for (i, tuple) in result_tuples.iter().enumerate() {
                    if let Some(val) = tuple.first() {
                        arr.set(i as u32, value_to_js(val));
                    }
                }
                arr.into()
            }
            FindSpec::Tuple(_) => {
                if let Some(tuple) = result_tuples.first() {
                    let arr = js_sys::Array::new_with_length(tuple.len() as u32);
                    for (j, val) in tuple.iter().enumerate() {
                        arr.set(j as u32, value_to_js(val));
                    }
                    arr.into()
                } else {
                    JsValue::NULL
                }
            }
        }
    }

    /// Pull an entity by pattern. Returns a JS object (nested map) or null.
    ///
    /// `pattern_edn` is the pull pattern as an EDN string, e.g. `"[:name :age]"`.
    /// `eid` is the entity ID (number) or a lookup ref `[":attr", value]`.
    #[wasm_bindgen(js_name = "pull")]
    pub fn pull_js(&self, pattern_edn: String, eid: JsValue) -> JsValue {
        use persistent_sorted_set::pull;
        use persistent_sorted_set::pull_parser::parse_pull_pattern_edn;

        let pattern = parse_pull_pattern_edn(&self.schema, &self.rschema, &pattern_edn);

        let resolved_eid = resolve_eid_from_js(self, &eid);
        match resolved_eid {
            Some(eid) => match pull::pull(self, &pattern, eid) {
                Some(pr) => pull_result_to_js(&pr),
                None => JsValue::NULL,
            },
            None => JsValue::NULL,
        }
    }

    /// Pull multiple entities by pattern. Returns a JS Array of objects.
    #[wasm_bindgen(js_name = "pullMany")]
    pub fn pull_many_js(&self, pattern_edn: String, eids: js_sys::Array) -> js_sys::Array {
        use persistent_sorted_set::pull;
        use persistent_sorted_set::pull_parser::parse_pull_pattern_edn;

        let pattern = parse_pull_pattern_edn(&self.schema, &self.rschema, &pattern_edn);
        let result = js_sys::Array::new_with_length(eids.length());
        for i in 0..eids.length() {
            let eid_js = eids.get(i);
            let resolved = resolve_eid_from_js(self, &eid_js);
            let val = match resolved {
                Some(eid) => match pull::pull(self, &pattern, eid) {
                    Some(pr) => pull_result_to_js(&pr),
                    None => JsValue::NULL,
                },
                None => JsValue::NULL,
            };
            result.set(i, val);
        }
        result
    }

    /// Resolve an entity ID or lookup ref to a numeric entity ID.
    /// Returns the numeric eid, or `JsValue::NULL` if unresolvable.
    ///
    /// `eid` may be a number (returned as-is) or a lookup ref `[":attr", value]`.
    #[wasm_bindgen(js_name = "entid")]
    pub fn entid_js(&self, eid: JsValue) -> JsValue {
        match resolve_eid_from_js(self, &eid) {
            Some(id) => JsValue::from_f64(id as f64),
            None => JsValue::NULL,
        }
    }

    /// Store the database to unified SQLite storage.
    #[wasm_bindgen(js_name = "storeDb")]
    pub fn store_db(&mut self, db_name: String) {
        // Store each index
        self.eavt.set_storage(Box::new(UnifiedSQLiteStorage::new(
            &db_name,
            self.settings.clone(),
        )));
        let eavt_root = self.eavt.store();

        self.aevt.set_storage(Box::new(UnifiedSQLiteStorage::new(
            &db_name,
            self.settings.clone(),
        )));
        let aevt_root = self.aevt.store();

        self.avet.set_storage(Box::new(UnifiedSQLiteStorage::new(
            &db_name,
            self.settings.clone(),
        )));
        let avet_root = self.avet.store();

        // Store metadata at addr=0
        let schema_blob = serialize_schema(&self.schema);
        let meta = DbMetadata {
            schema_blob,
            max_eid: self.max_eid,
            max_tx: self.max_tx,
            eavt_root,
            aevt_root,
            avet_root,
        };
        let meta_blob = serialize_metadata(&meta);

        let mut meta_storage = UnifiedSQLiteStorage::new(&db_name, self.settings.clone());
        meta_storage.store_metadata(METADATA_ADDR, &meta_blob);
    }

    /// Check whether a stored database exists at the given db_name.
    #[wasm_bindgen(js_name = "hasStoredDb")]
    pub fn has_stored_db(db_name: String) -> bool {
        let settings = Settings::new(512);
        let storage = UnifiedSQLiteStorage::new(&db_name, settings);
        storage.restore_metadata(METADATA_ADDR).is_some()
    }

    /// Restore a database from unified SQLite storage.
    /// Returns null if no stored database exists.
    #[wasm_bindgen(js_name = "restoreDb")]
    pub fn restore_db(db_name: String) -> Option<WasmDataScript> {
        let settings = Settings::new(512);
        let storage = UnifiedSQLiteStorage::new(&db_name, settings.clone());

        let meta_blob = match storage.restore_metadata(METADATA_ADDR) {
            Some(blob) => blob,
            None => return None,
        };
        let meta = deserialize_metadata(&meta_blob);
        let schema = deserialize_schema(&meta.schema_blob);
        let rschema = build_rschema(&schema);

        let eavt = PersistentSortedSet::restore(
            comparator_for_index(IndexType::EAVT),
            meta.eavt_root,
            Box::new(UnifiedSQLiteStorage::new(&db_name, settings.clone())),
            settings.clone(),
        );
        let aevt = PersistentSortedSet::restore(
            comparator_for_index(IndexType::AEVT),
            meta.aevt_root,
            Box::new(UnifiedSQLiteStorage::new(&db_name, settings.clone())),
            settings.clone(),
        );
        let avet = PersistentSortedSet::restore(
            comparator_for_index(IndexType::AVET),
            meta.avet_root,
            Box::new(UnifiedSQLiteStorage::new(&db_name, settings.clone())),
            settings.clone(),
        );

        Some(WasmDataScript {
            schema,
            rschema,
            eavt,
            aevt,
            avet,
            max_eid: meta.max_eid,
            max_tx: meta.max_tx,
            settings,
        })
    }

    /// Restore a DataScript database from legacy EDN format (`.bp7` files).
    /// The `datascript` table must exist with EDN metadata at addr=0.
    /// Returns null if no legacy data found.
    #[wasm_bindgen(js_name = "restoreFromLegacy")]
    pub fn restore_from_legacy(db_name: String) -> Option<WasmDataScript> {
        use crate::legacy_datascript::wasm::LegacyStorage;

        let settings_base = Settings::new(512);
        let meta = {
            let storage = LegacyStorage::new(&db_name, settings_base.clone());
            match storage.read_metadata() {
                Some(m) => m,
                None => return None, // No legacy metadata — not a legacy DB
            }
        };

        let settings = Settings::new(meta.branching_factor);
        let rschema = build_rschema(&meta.schema);

        let eavt = PersistentSortedSet::restore(
            comparator_for_index(IndexType::EAVT),
            meta.eavt_root,
            Box::new(LegacyStorage::new(&db_name, settings.clone())),
            settings.clone(),
        );
        let aevt = PersistentSortedSet::restore(
            comparator_for_index(IndexType::AEVT),
            meta.aevt_root,
            Box::new(LegacyStorage::new(&db_name, settings.clone())),
            settings.clone(),
        );
        let avet = PersistentSortedSet::restore(
            comparator_for_index(IndexType::AVET),
            meta.avet_root,
            Box::new(LegacyStorage::new(&db_name, settings.clone())),
            settings.clone(),
        );

        Some(WasmDataScript {
            schema: meta.schema,
            rschema,
            eavt,
            aevt,
            avet,
            max_eid: meta.max_eid,
            max_tx: meta.max_tx,
            settings,
        })
    }

    /// Check if a legacy (EDN) DataScript database exists at the given db_name.
    #[wasm_bindgen(js_name = "hasLegacyDb")]
    pub fn has_legacy_db(db_name: String) -> bool {
        crate::legacy_datascript::wasm::LegacyStorage::has_legacy_data(&db_name)
    }

    /// Store the database to the legacy EDN format in the `datascript` table.
    /// Creates the table if it doesn't exist. Writes nodes as EDN text.
    #[wasm_bindgen(js_name = "storeToLegacy")]
    pub fn store_to_legacy(&mut self, db_name: String) {
        use crate::legacy_datascript::wasm::LegacyStorage;
        use crate::legacy_datascript::LegacyMetadata;

        let settings = self.settings.clone();

        // Ensure the datascript table exists
        {
            use std::ffi::CString;
            use std::ptr;
            use absurder_sql::connection_pool;
            let pool_key = db_name.trim_end_matches(".db");
            let conn = connection_pool::get_or_create_connection(pool_key, {
                let name = db_name.clone();
                move || {
                    let c_path = CString::new(name.as_str()).map_err(|e| e.to_string())?;
                    let mut db: *mut sqlite_wasm_rs::sqlite3 = ptr::null_mut();
                    let ret = unsafe {
                        sqlite_wasm_rs::sqlite3_open_v2(
                            c_path.as_ptr(),
                            &mut db,
                            sqlite_wasm_rs::SQLITE_OPEN_READWRITE | sqlite_wasm_rs::SQLITE_OPEN_CREATE,
                            ptr::null(),
                        )
                    };
                    if ret != sqlite_wasm_rs::SQLITE_OK {
                        return Err(format!("Failed to open SQLite: {}", name));
                    }
                    Ok(db)
                }
            }).expect("Failed to get connection for legacy store");
            let db = conn.db.get();
            unsafe {
                let c_sql = CString::new(
                    "CREATE TABLE IF NOT EXISTS datascript (addr INTEGER PRIMARY KEY, content TEXT)"
                ).unwrap();
                sqlite_wasm_rs::sqlite3_exec(
                    db, c_sql.as_ptr(), None, ptr::null_mut(), ptr::null_mut()
                );
            }
            connection_pool::release_connection(pool_key);
        }

        // Store each index tree
        self.eavt.set_storage(Box::new(LegacyStorage::new(&db_name, settings.clone())));
        let eavt_root = self.eavt.store();

        self.aevt.set_storage(Box::new(LegacyStorage::new(&db_name, settings.clone())));
        let aevt_root = self.aevt.store();

        self.avet.set_storage(Box::new(LegacyStorage::new(&db_name, settings.clone())));
        let avet_root = self.avet.store();

        // Compute max_addr from the last storage instance
        let mut meta_storage = LegacyStorage::new(&db_name, settings.clone());
        let max_addr = meta_storage.max_addr();

        let meta = LegacyMetadata {
            schema: self.schema.clone(),
            rschema: build_rschema(&self.schema),
            eavt_root,
            aevt_root,
            avet_root,
            max_eid: self.max_eid,
            max_tx: self.max_tx,
            max_addr,
            branching_factor: settings.branching_factor(),
        };

        meta_storage.write_metadata(&meta);
    }

    /// Transact tx-data (EDN string) against this database.
    ///
    /// Mutates the database in place (becomes db-after) and returns a JS object:
    /// ```js
    /// { txData: [{e, a, v, tx}, ...], tempids: {"<tempid>": eid, ...}, currentTx: number }
    /// ```
    ///
    /// Supports: `:db/add`, `:db/retract`, `:db.fn/retractAttribute`,
    /// `:db.fn/retractEntity`, `:db/retractEntity`, map entities, tempids,
    /// lookup refs, upsert via `:db.unique/identity`.
    #[wasm_bindgen(js_name = "transact")]
    pub fn transact_edn(&mut self, tx_data_edn: String) -> JsValue {
        use persistent_sorted_set::transact::{parse_tx_edn, transact};

        let entities = match parse_tx_edn(&tx_data_edn, &self.rschema) {
            Ok(e) => e,
            Err(err) => {
                let obj = js_sys::Object::new();
                js_sys::Reflect::set(&obj, &"error".into(), &JsValue::from_str(&err.to_string())).ok();
                return obj.into();
            }
        };

        let report = match transact(self, entities) {
            Ok(r) => r,
            Err(err) => {
                let obj = js_sys::Object::new();
                js_sys::Reflect::set(&obj, &"error".into(), &JsValue::from_str(&err.to_string())).ok();
                return obj.into();
            }
        };

        // Convert tx_data to JS array of datom objects
        let tx_data_arr = js_sys::Array::new_with_length(report.tx_data.len() as u32);
        for (i, d) in report.tx_data.iter().enumerate() {
            tx_data_arr.set(i as u32, datom_to_js(d));
        }

        // Convert tempids to JS object
        let tempids_obj = js_sys::Object::new();
        for (tid, eid) in &report.tempids {
            let key = tid.to_string();
            js_sys::Reflect::set(&tempids_obj, &JsValue::from_str(&key), &JsValue::from_f64(*eid as f64)).ok();
        }

        // Build result object
        let result = js_sys::Object::new();
        js_sys::Reflect::set(&result, &"txData".into(), &tx_data_arr).ok();
        js_sys::Reflect::set(&result, &"tempids".into(), &tempids_obj).ok();
        js_sys::Reflect::set(&result, &"currentTx".into(), &JsValue::from_f64(report.current_tx as f64)).ok();

        result.into()
    }

    /// Collect garbage: walk all 3 indexes, delete orphan addresses.
    #[wasm_bindgen(js_name = "collectGarbage")]
    pub fn collect_garbage(&self, db_name: String) {
        let storage = UnifiedSQLiteStorage::new(&db_name, self.settings.clone());

        let mut live_addrs = HashSet::new();
        live_addrs.insert(METADATA_ADDR);

        self.eavt
            .walk_addresses(&mut |addr| { live_addrs.insert(addr); true });
        self.aevt
            .walk_addresses(&mut |addr| { live_addrs.insert(addr); true });
        self.avet
            .walk_addresses(&mut |addr| { live_addrs.insert(addr); true });

        let all_addrs = storage.list_addresses();
        let orphans: Vec<i64> = all_addrs
            .into_iter()
            .filter(|addr| !live_addrs.contains(addr))
            .collect();

        if !orphans.is_empty() {
            let mut storage_mut = UnifiedSQLiteStorage::new(&db_name, self.settings.clone());
            storage_mut.delete(&orphans);
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_attr_arg(val: &JsValue) -> Option<Attr> {
    if val.is_null() || val.is_undefined() {
        return None;
    }
    attr_from_js(val)
}

fn parse_value_arg(val: &JsValue) -> Option<Value> {
    if val.is_null() || val.is_undefined() {
        return None;
    }
    let v = value_from_js(val);
    match v {
        Value::Nil => None,
        other => Some(other),
    }
}

fn make_datom_from_components(e: &JsValue, a: &JsValue, v: &JsValue, tx: &JsValue) -> Datom {
    Datom::new(
        e.as_f64().unwrap_or(0.0) as i64,
        parse_attr_arg(a),
        if v.is_null() || v.is_undefined() {
            Value::Nil
        } else {
            value_from_js(v)
        },
        tx.as_f64().unwrap_or(0.0) as i64,
    )
}

// ---------------------------------------------------------------------------
// JS → Rust clause/rule parsing
// ---------------------------------------------------------------------------

use persistent_sorted_set::relation::{Clause, PatternEl, RuleBranch, Rules, Tuple, Var};

/// Parse a single JS value into a PatternEl.
fn pattern_el_from_js(el: &JsValue) -> PatternEl {
    if el.is_null() || el.is_undefined() {
        PatternEl::Blank
    } else if let Some(s) = el.as_string() {
        if s.starts_with('?') {
            PatternEl::Var(s)
        } else if s == "_" {
            PatternEl::Blank
        } else if s.starts_with(':') {
            PatternEl::Const(Value::Keyword(attr_from_keyword_str(&s)))
        } else {
            PatternEl::Const(Value::Str(s))
        }
    } else if let Some(n) = el.as_f64() {
        let n_i64 = n as i64;
        PatternEl::Const(if n.fract() == 0.0 { Value::Long(n_i64) } else { Value::Double(n) })
    } else if let Some(b) = el.as_bool() {
        PatternEl::Const(Value::Bool(b))
    } else {
        PatternEl::Blank
    }
}

/// Parse a JS Array of pattern arrays into Rust patterns.
fn parse_patterns_js(patterns: &js_sys::Array) -> Vec<[PatternEl; 4]> {
    let mut result = Vec::with_capacity(patterns.length() as usize);
    for i in 0..patterns.length() {
        let pat = js_sys::Array::from(&patterns.get(i));
        let mut els = [PatternEl::Blank, PatternEl::Blank, PatternEl::Blank, PatternEl::Blank];
        for j in 0..4.min(pat.length()) {
            els[j as usize] = pattern_el_from_js(&pat.get(j));
        }
        result.push(els);
    }
    result
}

/// Parse a JS Array of PatternEl-like values into a Vec<PatternEl>.
fn parse_args_js(arr: &js_sys::Array) -> Vec<PatternEl> {
    (0..arr.length())
        .map(|i| pattern_el_from_js(&arr.get(i)))
        .collect()
}

/// Parse a single JS clause object into a Rust Clause.
///
/// Expected JS format:
/// - `{type: "pattern", pattern: [e, a, v, tx]}`
/// - `{type: "rule", name: "ruleName", args: [...]}`
/// - `{type: "predicate", name: "predName", args: [...]}`
/// - `{type: "and", clauses: [...]}`
/// - `{type: "or", branches: [[...], [...]]}`
/// - `{type: "not", clauses: [...]}`
fn clause_from_js(val: &JsValue) -> Option<Clause> {
    let type_str = js_sys::Reflect::get(val, &JsValue::from_str("type"))
        .ok()?
        .as_string()?;

    match type_str.as_str() {
        "pattern" => {
            let pat_js = js_sys::Reflect::get(val, &JsValue::from_str("pattern")).ok()?;
            let pat = js_sys::Array::from(&pat_js);
            let mut els = [PatternEl::Blank, PatternEl::Blank, PatternEl::Blank, PatternEl::Blank];
            for j in 0..4.min(pat.length()) {
                els[j as usize] = pattern_el_from_js(&pat.get(j));
            }
            Some(Clause::Pattern { source: None, pattern: els })
        }
        "rule" => {
            let name = js_sys::Reflect::get(val, &JsValue::from_str("name"))
                .ok()?
                .as_string()?;
            let args_js = js_sys::Reflect::get(val, &JsValue::from_str("args")).ok()?;
            let args = parse_args_js(&js_sys::Array::from(&args_js));
            Some(Clause::RuleCall { name, args })
        }
        "predicate" => {
            let name = js_sys::Reflect::get(val, &JsValue::from_str("name"))
                .ok()?
                .as_string()?;
            let args_js = js_sys::Reflect::get(val, &JsValue::from_str("args")).ok()?;
            let args = parse_args_js(&js_sys::Array::from(&args_js));
            Some(Clause::Predicate { name, args })
        }
        "and" => {
            let clauses_js = js_sys::Reflect::get(val, &JsValue::from_str("clauses")).ok()?;
            let clauses = parse_clauses_js(&js_sys::Array::from(&clauses_js));
            Some(Clause::And(clauses))
        }
        "or" => {
            let branches_js = js_sys::Reflect::get(val, &JsValue::from_str("branches")).ok()?;
            let branches_arr = js_sys::Array::from(&branches_js);
            let mut branches = Vec::with_capacity(branches_arr.length() as usize);
            for i in 0..branches_arr.length() {
                let branch_clauses = parse_clauses_js(&js_sys::Array::from(&branches_arr.get(i)));
                branches.push(branch_clauses);
            }
            Some(Clause::Or(branches))
        }
        "not" => {
            let clauses_js = js_sys::Reflect::get(val, &JsValue::from_str("clauses")).ok()?;
            let clauses = parse_clauses_js(&js_sys::Array::from(&clauses_js));
            Some(Clause::Not(clauses))
        }
        _ => None,
    }
}

/// Parse a JS Array of clause objects into Vec<Clause>.
fn parse_clauses_js(arr: &js_sys::Array) -> Vec<Clause> {
    (0..arr.length())
        .filter_map(|i| clause_from_js(&arr.get(i)))
        .collect()
}

/// Parse a JS rules object into Rust Rules.
///
/// Expected JS format:
/// ```js
/// {
///   "ruleName": [
///     { head: ["?a", "?b"], body: [{type: "pattern", ...}] },
///     { head: ["?a", "?b"], body: [{type: "pattern", ...}] }
///   ]
/// }
/// ```
fn parse_rules_js(val: &JsValue) -> Rules {
    let mut rules = Rules::new();
    if val.is_null() || val.is_undefined() {
        return rules;
    }

    let entries = js_sys::Object::entries(&js_sys::Object::from(val.clone()));
    for i in 0..entries.length() {
        let entry = js_sys::Array::from(&entries.get(i));
        let name = match entry.get(0).as_string() {
            Some(s) => s,
            None => continue,
        };
        let branches_js = js_sys::Array::from(&entry.get(1));
        let mut branches = Vec::with_capacity(branches_js.length() as usize);

        for j in 0..branches_js.length() {
            let branch_js = branches_js.get(j);

            let head_js = match js_sys::Reflect::get(&branch_js, &JsValue::from_str("head")).ok() {
                Some(h) => js_sys::Array::from(&h),
                None => continue,
            };
            let head_args: Vec<Var> = (0..head_js.length())
                .filter_map(|k| head_js.get(k).as_string())
                .collect();

            let body_js = match js_sys::Reflect::get(&branch_js, &JsValue::from_str("body")).ok() {
                Some(b) => js_sys::Array::from(&b),
                None => continue,
            };
            let body = parse_clauses_js(&body_js);

            branches.push(RuleBranch { head_args, body });
        }

        if !branches.is_empty() {
            rules.insert(name, branches);
        }
    }
    rules
}

/// Parse a JS value into a datom Value for input binding.
fn parse_input_value(val: &JsValue) -> Value {
    if val.is_null() || val.is_undefined() {
        Value::Nil
    } else if let Some(s) = val.as_string() {
        if s.starts_with(':') {
            Value::Keyword(attr_from_keyword_str(&s))
        } else {
            Value::Str(s)
        }
    } else if let Some(n) = val.as_f64() {
        if n.fract() == 0.0 {
            Value::Long(n as i64)
        } else {
            Value::Double(n)
        }
    } else if let Some(b) = val.as_bool() {
        Value::Bool(b)
    } else {
        Value::Nil
    }
}

/// Convert result tuples to a JS Array of Arrays.
fn tuples_to_js(tuples: &[Tuple]) -> js_sys::Array {
    let outer = js_sys::Array::new_with_length(tuples.len() as u32);
    for (i, tuple) in tuples.iter().enumerate() {
        let inner = js_sys::Array::new_with_length(tuple.len() as u32);
        for (j, val) in tuple.iter().enumerate() {
            inner.set(j as u32, value_to_js(val));
        }
        outer.set(i as u32, inner.into());
    }
    outer
}

/// Convert a PullResult to a JS value (nested objects/arrays/scalars).
fn pull_result_to_js(result: &persistent_sorted_set::pull::PullResult) -> JsValue {
    use persistent_sorted_set::legacy_edn::attr_to_edn;
    use persistent_sorted_set::pull::PullResult;

    match result {
        PullResult::Scalar(v) => value_to_js(v),
        PullResult::Vec(items) => {
            let arr = js_sys::Array::new_with_length(items.len() as u32);
            for (i, item) in items.iter().enumerate() {
                arr.set(i as u32, pull_result_to_js(item));
            }
            arr.into()
        }
        PullResult::Map(entries) => {
            let obj = js_sys::Object::new();
            for (attr, val) in entries {
                let key = JsValue::from_str(&attr_to_edn(attr));
                let _ = js_sys::Reflect::set(&obj, &key, &pull_result_to_js(val));
            }
            obj.into()
        }
    }
}

/// Resolve a JS entity ID to a Rust i64.
/// Handles numbers directly and lookup refs like `[":bp/uuid", "abc"]`.
fn resolve_eid_from_js(db: &WasmDataScript, eid: &JsValue) -> Option<i64> {
    use persistent_sorted_set::pull::PullSource;

    if let Some(n) = eid.as_f64() {
        return Some(n as i64);
    }
    // Check for lookup ref: [":attr", value]
    if js_sys::Array::is_array(eid) {
        let arr = js_sys::Array::from(eid);
        if arr.length() == 2 {
            if let Some(attr_str) = arr.get(0).as_string() {
                if let Some(attr) = attr_from_js(&JsValue::from_str(&attr_str)) {
                    let val = parse_input_value(&arr.get(1));
                    return db.resolve_lookup_ref(&attr, &val);
                }
            }
        }
    }
    None
}
