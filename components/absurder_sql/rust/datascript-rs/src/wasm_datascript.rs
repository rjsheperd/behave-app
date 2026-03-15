//! WasmDataScript — Rust DataScript database with PersistentSortedSet indexes.
//!
//! Combines schema routing logic (from db.rs) with real PSS indexes using
//! Datom keys. Provides store/restore/GC via UnifiedSQLiteStorage.

use std::collections::HashSet;

use js_sys;
use wasm_bindgen::prelude::*;

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
        for i in 0..datoms_array.length() {
            let datom = datom_from_js(&datoms_array.get(i));
            self = self.with_datom_internal(datom);
        }
        self
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
        use crate::query::{PatternEl, resolve_patterns, collect_results};
        use persistent_sorted_set::relation::Var;

        // Parse patterns from JS
        let mut rust_patterns: Vec<[PatternEl; 4]> = Vec::with_capacity(patterns.length() as usize);
        for i in 0..patterns.length() {
            let pat = js_sys::Array::from(&patterns.get(i));
            let mut els = [PatternEl::Blank, PatternEl::Blank, PatternEl::Blank, PatternEl::Blank];
            for j in 0..4.min(pat.length()) {
                let el = pat.get(j);
                els[j as usize] = if el.is_null() || el.is_undefined() {
                    PatternEl::Blank
                } else if let Some(s) = el.as_string() {
                    if s.starts_with('?') {
                        PatternEl::Var(s)
                    } else if s == "_" {
                        PatternEl::Blank
                    } else if s.starts_with(':') {
                        // Keyword attr or value
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
                };
            }
            rust_patterns.push(els);
        }

        // Parse find vars
        let rust_find: Vec<Var> = (0..find_vars.length())
            .filter_map(|i| find_vars.get(i).as_string())
            .collect();

        // Resolve patterns + join entirely in Rust
        let result_rel = resolve_patterns(self, &rust_patterns);
        let result_tuples = collect_results(&result_rel, &rust_find);

        // Convert to JS
        let outer = js_sys::Array::new_with_length(result_tuples.len() as u32);
        for (i, tuple) in result_tuples.iter().enumerate() {
            let inner = js_sys::Array::new_with_length(tuple.len() as u32);
            for (j, val) in tuple.iter().enumerate() {
                inner.set(j as u32, value_to_js(val));
            }
            outer.set(i as u32, inner.into());
        }
        outer
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
            storage.read_metadata()
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
