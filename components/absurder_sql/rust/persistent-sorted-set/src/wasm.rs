//! wasm-bindgen exports wrapping the inner Rust types.
//! Only compiled for wasm32 targets.

use std::cmp::Ordering;
use std::rc::Rc;

use js_sys;
use wasm_bindgen::prelude::*;

use crate::comparator::{comparator_for_index, parse_index_type, IndexType};
use crate::datom::{Attr, Datom, Value};
use crate::js_storage::JsStorage;
use crate::key::Key;
use crate::node::Comparator;
use crate::seq::Seq as InnerSeq;
use crate::set::PersistentSortedSet;
use crate::settings::Settings;
use crate::storage::IStorage;

// ---------------------------------------------------------------------------
// Datom ↔ JsValue conversion
// ---------------------------------------------------------------------------

/// Extract a Datom from a CLJS Datom object (or plain JS object with e, a, v, tx).
pub fn datom_from_js(val: &JsValue) -> Datom {
    let e = js_sys::Reflect::get(val, &JsValue::from_str("e"))
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0) as i64;

    let a_val = js_sys::Reflect::get(val, &JsValue::from_str("a"))
        .unwrap_or(JsValue::NULL);
    let a = attr_from_js(&a_val);

    let v_val = js_sys::Reflect::get(val, &JsValue::from_str("v"))
        .unwrap_or(JsValue::NULL);
    let v = value_from_js(&v_val);

    let tx = js_sys::Reflect::get(val, &JsValue::from_str("tx"))
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0) as i64;

    let mut datom = Datom::new(e, a, v, tx);
    datom.original_js = Some(val.clone());
    datom
}

/// Convert a Datom to a JS object with e, a, v, tx properties.
/// Returns the original CLJS datom if available (lossless round-trip).
pub fn datom_to_js(d: &Datom) -> JsValue {
    if let Some(ref orig) = d.original_js {
        return orig.clone();
    }
    let obj = js_sys::Object::new();

    js_sys::Reflect::set(&obj, &JsValue::from_str("e"), &JsValue::from_f64(d.e as f64))
        .unwrap();

    let a_js = match &d.a {
        None => JsValue::NULL,
        Some(attr) => attr_to_js(attr),
    };
    js_sys::Reflect::set(&obj, &JsValue::from_str("a"), &a_js).unwrap();

    let v_js = value_to_js(&d.v);
    js_sys::Reflect::set(&obj, &JsValue::from_str("v"), &v_js).unwrap();

    js_sys::Reflect::set(&obj, &JsValue::from_str("tx"), &JsValue::from_f64(d.tx as f64))
        .unwrap();

    obj.into()
}

/// Parse a JS value into an Attr.
/// Handles CLJS keywords (strings like ":ns/name" or "ns/name") and plain strings.
pub fn attr_from_js(val: &JsValue) -> Option<Attr> {
    if val.is_null() || val.is_undefined() {
        return None;
    }

    let s = match val.as_string() {
        Some(s) => s,
        None => {
            // Try to get the keyword's fqn or str representation
            // CLJS keywords have .-fqn property
            let fqn = js_sys::Reflect::get(val, &JsValue::from_str("fqn"))
                .ok()
                .and_then(|v| v.as_string());
            match fqn {
                Some(s) => s,
                None => return None,
            }
        }
    };

    // Strip leading colon if present (CLJS keyword string representation)
    let s = if s.starts_with(':') { &s[1..] } else { &s };

    if let Some(idx) = s.find('/') {
        let ns = &s[..idx];
        let name = &s[idx + 1..];
        Some(Attr::Keyword {
            ns: Some(ns.to_string()),
            name: name.to_string(),
        })
    } else {
        Some(Attr::Keyword {
            ns: None,
            name: s.to_string(),
        })
    }
}

/// Convert an Attr to a JS string (keyword format "ns/name" or "name").
fn attr_to_js(attr: &Attr) -> JsValue {
    match attr {
        Attr::Keyword { ns: Some(ns), name } => {
            JsValue::from_str(&format!(":{}/{}", ns, name))
        }
        Attr::Keyword { ns: None, name } => {
            JsValue::from_str(&format!(":{}", name))
        }
        Attr::Str(s) => JsValue::from_str(s),
    }
}

/// Parse a JS value into a Value.
pub fn value_from_js(val: &JsValue) -> Value {
    if val.is_null() || val.is_undefined() {
        return Value::Nil;
    }

    if let Some(b) = val.as_bool() {
        return Value::Bool(b);
    }

    if let Some(n) = val.as_f64() {
        // Check if it's an integer
        if n.fract() == 0.0 && n >= i64::MIN as f64 && n <= i64::MAX as f64 {
            return Value::Long(n as i64);
        }
        return Value::Double(n);
    }

    if let Some(s) = val.as_string() {
        // Check if it looks like a keyword
        if s.starts_with(':') {
            let inner = &s[1..];
            if let Some(idx) = inner.find('/') {
                return Value::Keyword(Attr::Keyword {
                    ns: Some(inner[..idx].to_string()),
                    name: inner[idx + 1..].to_string(),
                });
            }
            return Value::Keyword(Attr::Keyword {
                ns: None,
                name: inner.to_string(),
            });
        }
        return Value::Str(s);
    }

    // Check for CLJS keyword object (has .-fqn property)
    if let Ok(fqn) = js_sys::Reflect::get(val, &JsValue::from_str("fqn")) {
        if let Some(s) = fqn.as_string() {
            let inner = if s.starts_with(':') { &s[1..] } else { &s };
            if let Some(idx) = inner.find('/') {
                return Value::Keyword(Attr::Keyword {
                    ns: Some(inner[..idx].to_string()),
                    name: inner[idx + 1..].to_string(),
                });
            }
            return Value::Keyword(Attr::Keyword {
                ns: None,
                name: inner.to_string(),
            });
        }
    }

    // Fallback: treat as nil
    Value::Nil
}

/// Convert a Value to a JsValue.
pub fn value_to_js(v: &Value) -> JsValue {
    match v {
        Value::Nil => JsValue::NULL,
        Value::Bool(b) => JsValue::from_bool(*b),
        Value::Long(n) => JsValue::from_f64(*n as f64),
        Value::Double(n) => JsValue::from_f64(*n),
        Value::Str(s) => JsValue::from_str(s),
        Value::Keyword(attr) => attr_to_js(attr),
        Value::Ref(n) => JsValue::from_f64(*n as f64),
        Value::Instant(n) => JsValue::from_f64(*n as f64),
        Value::Uuid(bytes) => {
            // Format as UUID string
            let hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
            let s = format!(
                "{}-{}-{}-{}-{}",
                &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]
            );
            JsValue::from_str(&s)
        }
        Value::Bytes(b) => {
            let arr = js_sys::Uint8Array::new_with_length(b.len() as u32);
            arr.copy_from(b);
            arr.into()
        }
    }
}

// ---------------------------------------------------------------------------
// Helper: build Datom from explicit components
// ---------------------------------------------------------------------------

fn datom_from_components(e: f64, a: &JsValue, v: &JsValue, tx: f64) -> Datom {
    Datom::new(
        e as i64,
        attr_from_js(a),
        value_from_js(v),
        tx as i64,
    )
}

// ---------------------------------------------------------------------------
// Settings parsing
// ---------------------------------------------------------------------------

/// Parse Settings from a JS object with `branchingFactor` and optional `cacheSize`.
fn parse_settings(settings: &JsValue) -> Settings {
    let bf = js_sys::Reflect::get(settings, &JsValue::from_str("branchingFactor"))
        .ok()
        .and_then(|v| v.as_f64())
        .map(|n| n as usize)
        .unwrap_or(512);

    let cache_size = js_sys::Reflect::get(settings, &JsValue::from_str("cacheSize"))
        .ok()
        .and_then(|v| v.as_f64())
        .map(|n| n as usize)
        .unwrap_or(0);

    let s = Settings::new(bf);
    if cache_size > 0 { s.with_cache_size(cache_size) } else { s }
}

// ---------------------------------------------------------------------------
// JS comparator wrapping (backward-compat fallback, slower than IndexType path)
// ---------------------------------------------------------------------------

/// Convert a JS comparator function into a Rust Comparator.
/// This is the slow path: Datom → JsValue → JS function call → Ordering.
fn js_to_comparator(f: js_sys::Function) -> Rc<Comparator> {
    Rc::new(move |a: &Key, b: &Key| {
        let js_a = datom_to_js(a);
        let js_b = datom_to_js(b);
        let result = f.call2(&JsValue::NULL, &js_a, &js_b)
            .expect("JS comparator threw an exception");
        let n = result.as_f64()
            .expect("JS comparator must return a number") as i32;
        match n.cmp(&0) {
            std::cmp::Ordering::Less => Ordering::Less,
            std::cmp::Ordering::Equal => Ordering::Equal,
            std::cmp::Ordering::Greater => Ordering::Greater,
        }
    })
}

// ===========================================================================
// WasmPSS
// ===========================================================================

#[wasm_bindgen]
pub struct WasmPSS {
    inner: PersistentSortedSet,
    index_type: Option<IndexType>,
}

#[wasm_bindgen]
impl WasmPSS {
    // --- New constructors (fast path, pure Rust comparisons) ---

    /// Create an empty set with a Rust-native IndexType comparator.
    /// `index_type` is one of "eavt", "aevt", "avet".
    #[wasm_bindgen(js_name = "emptyWithIndex")]
    pub fn empty_with_index(index_type: String) -> WasmPSS {
        let idx = parse_index_type(&index_type);
        WasmPSS {
            inner: PersistentSortedSet::empty(comparator_for_index(idx)),
            index_type: Some(idx),
        }
    }

    /// Create an empty set with IndexType, storage, and settings.
    #[wasm_bindgen(js_name = "emptyWithIndexAndStorage")]
    pub fn empty_with_index_and_storage(
        index_type: String,
        storage: JsValue,
        settings: JsValue,
    ) -> WasmPSS {
        let idx = parse_index_type(&index_type);
        let cmp = comparator_for_index(idx);
        let rust_settings = parse_settings(&settings);

        if storage.is_undefined() || storage.is_null() {
            WasmPSS {
                inner: PersistentSortedSet::empty_with_settings(cmp, rust_settings),
                index_type: Some(idx),
            }
        } else {
            let js_storage = JsStorage::from_js_object(&storage);
            WasmPSS {
                inner: PersistentSortedSet::with_storage(
                    cmp,
                    Box::new(js_storage),
                    rust_settings,
                ),
                index_type: Some(idx),
            }
        }
    }

    /// Add a datom by its components (fast path, no JS object creation).
    #[wasm_bindgen(js_name = "conjDatom")]
    pub fn conj_datom(&self, e: f64, a: JsValue, v: JsValue, tx: f64) -> WasmPSS {
        let datom = datom_from_components(e, &a, &v, tx);
        WasmPSS {
            inner: self.inner.clone().conj(&datom),
            index_type: self.index_type,
        }
    }

    /// Remove a datom by its components (fast path).
    #[wasm_bindgen(js_name = "disjDatom")]
    pub fn disj_datom(&self, e: f64, a: JsValue, v: JsValue, tx: f64) -> WasmPSS {
        let datom = datom_from_components(e, &a, &v, tx);
        WasmPSS {
            inner: self.inner.clone().disj(&datom),
            index_type: self.index_type,
        }
    }

    /// Slice returning a JS Array of datom objects.
    #[wasm_bindgen(js_name = "datomsSlice")]
    pub fn datoms_slice(
        &self,
        from_e: f64, from_a: JsValue, from_v: JsValue, from_tx: f64,
        to_e: f64, to_a: JsValue, to_v: JsValue, to_tx: f64,
    ) -> js_sys::Array {
        let from = datom_from_components(from_e, &from_a, &from_v, from_tx);
        let to = datom_from_components(to_e, &to_a, &to_v, to_tx);
        let arr = js_sys::Array::new();
        if let Some(seq) = self.inner.slice(Some(&from), Some(&to)) {
            for key in seq.to_vec() {
                arr.push(&datom_to_js(&key));
            }
        }
        arr
    }

    // --- Legacy constructors (backward compat, slower JS comparator path) ---

    /// Create an empty set with the given JS comparator function.
    /// DEPRECATED: Use `emptyWithIndex` for better performance.
    #[wasm_bindgen(js_name = "empty")]
    pub fn empty(cmp: js_sys::Function) -> WasmPSS {
        WasmPSS {
            inner: PersistentSortedSet::empty(js_to_comparator(cmp)),
            index_type: None,
        }
    }

    /// Create an empty set with comparator and storage callbacks.
    /// DEPRECATED: Use `emptyWithIndexAndStorage` for better performance.
    #[wasm_bindgen(js_name = "withComparatorAndStorage")]
    pub fn with_cmp_and_storage(
        cmp: js_sys::Function,
        storage: JsValue,
        settings: JsValue,
    ) -> WasmPSS {
        let rust_settings = parse_settings(&settings);
        let comparator = js_to_comparator(cmp);

        if storage.is_undefined() || storage.is_null() {
            WasmPSS {
                inner: PersistentSortedSet::empty_with_settings(comparator, rust_settings),
                index_type: None,
            }
        } else {
            let js_storage = JsStorage::from_js_object(&storage);
            WasmPSS {
                inner: PersistentSortedSet::with_storage(
                    comparator,
                    Box::new(js_storage),
                    rust_settings,
                ),
                index_type: None,
            }
        }
    }

    /// Create a set from a JS array, comparator, optional storage, and settings.
    #[wasm_bindgen(js_name = "from")]
    pub fn from_array(
        arr: js_sys::Array,
        cmp: js_sys::Function,
        _storage: JsValue,
        _settings: JsValue,
    ) -> WasmPSS {
        let comparator = js_to_comparator(cmp);
        let mut set = PersistentSortedSet::empty(Rc::clone(&comparator));
        for i in 0..arr.length() {
            let key = datom_from_js(&arr.get(i));
            set = set.conj(&key);
        }
        WasmPSS { inner: set, index_type: None }
    }

    /// Restore from storage address (lazy — root not loaded until accessed).
    #[wasm_bindgen(js_name = "restore")]
    pub fn restore(
        cmp: js_sys::Function,
        address: f64,
        storage: JsValue,
        settings: JsValue,
    ) -> WasmPSS {
        let rust_settings = parse_settings(&settings);
        let js_storage = JsStorage::from_js_object(&storage);

        WasmPSS {
            inner: PersistentSortedSet::restore(
                js_to_comparator(cmp),
                address as i64,
                Box::new(js_storage),
                rust_settings,
            ),
            index_type: None,
        }
    }

    /// Restore with IndexType comparator (fast path).
    #[wasm_bindgen(js_name = "restoreWithIndex")]
    pub fn restore_with_index(
        index_type: String,
        address: f64,
        storage: JsValue,
        settings: JsValue,
    ) -> WasmPSS {
        let idx = parse_index_type(&index_type);
        let rust_settings = parse_settings(&settings);
        let js_storage = JsStorage::from_js_object(&storage);

        WasmPSS {
            inner: PersistentSortedSet::restore(
                comparator_for_index(idx),
                address as i64,
                Box::new(js_storage),
                rust_settings,
            ),
            index_type: Some(idx),
        }
    }

    // --- Common methods ---

    /// Add a key (JS datom object). Optional comparator override (ignored).
    pub fn conj(&self, key: JsValue, _cmp: JsValue) -> WasmPSS {
        let datom = datom_from_js(&key);
        WasmPSS {
            inner: self.inner.clone().conj(&datom),
            index_type: self.index_type,
        }
    }

    /// Remove a key (JS datom object). Optional comparator override (ignored).
    pub fn disj(&self, key: JsValue, _cmp: JsValue) -> WasmPSS {
        let datom = datom_from_js(&key);
        WasmPSS {
            inner: self.inner.clone().disj(&datom),
            index_type: self.index_type,
        }
    }

    pub fn contains(&self, key: JsValue) -> bool {
        let datom = datom_from_js(&key);
        self.inner.contains(&datom)
    }

    pub fn count(&self) -> usize {
        self.inner.count()
    }

    pub fn seq(&self) -> Option<WasmSeq> {
        self.inner.seq().map(|s| WasmSeq { inner: s })
    }

    /// Forward slice [from, to] inclusive.
    pub fn slice(&self, from: JsValue, to: JsValue, _cmp: JsValue) -> Option<WasmSeq> {
        let from_opt = if from.is_undefined() || from.is_null() {
            None
        } else {
            Some(datom_from_js(&from))
        };
        let to_opt = if to.is_undefined() || to.is_null() {
            None
        } else {
            Some(datom_from_js(&to))
        };
        self.inner
            .slice(from_opt.as_ref(), to_opt.as_ref())
            .map(|s| WasmSeq { inner: s })
    }

    /// Reverse slice [from, to] inclusive.
    pub fn rslice(&self, from: JsValue, to: JsValue, _cmp: JsValue) -> Option<WasmSeq> {
        let from_opt = if from.is_undefined() || from.is_null() {
            None
        } else {
            Some(datom_from_js(&from))
        };
        let to_opt = if to.is_undefined() || to.is_null() {
            None
        } else {
            Some(datom_from_js(&to))
        };
        self.inner
            .rslice(from_opt.as_ref(), to_opt.as_ref())
            .map(|s| WasmSeq { inner: s })
    }

    /// Store tree to storage, returns root address.
    pub fn store(&mut self, storage: JsValue) -> JsValue {
        if self.inner.storage().is_none() && !storage.is_undefined() && !storage.is_null() {
            let js_storage = JsStorage::from_js_object(&storage);
            self.inner.set_storage(Box::new(js_storage));
        }
        let addr = self.inner.store();
        JsValue::from_f64(addr as f64)
    }

    /// Walk all addresses, calling callback(addr) for each.
    #[wasm_bindgen(js_name = "walkAddresses")]
    pub fn walk_addresses(&self, callback: js_sys::Function) {
        self.inner.walk_addresses(&mut |addr| {
            let result = callback.call1(&JsValue::NULL, &JsValue::from_f64(addr as f64));
            result.ok().and_then(|v| v.as_bool()).unwrap_or(true)
        });
    }

    pub fn equals(&self, _other: &WasmPSS) -> bool {
        false
    }

    #[wasm_bindgen(js_name = "hashCode")]
    pub fn hash_code(&self) -> i32 {
        0
    }

    #[wasm_bindgen(js_name = "toArray")]
    pub fn to_array(&self) -> js_sys::Array {
        let arr = js_sys::Array::new();
        let keys = self.inner.to_vec();
        for k in keys {
            arr.push(&datom_to_js(&k));
        }
        arr
    }

    /// Branching factor from settings.
    #[wasm_bindgen(js_name = "branchingFactor")]
    pub fn branching_factor(&self) -> usize {
        self.inner.settings().branching_factor()
    }
}

// ---------------------------------------------------------------------------
// Rust-only constructors (not wasm_bindgen) for use by datascript-rs
// ---------------------------------------------------------------------------

impl WasmPSS {
    /// Create an empty PSS with the given IndexType and storage backend.
    /// Called from datascript-rs to wire up UnifiedSQLiteStorage.
    pub fn new_with_storage(
        index_type: IndexType,
        storage: Box<dyn IStorage>,
        settings: Settings,
    ) -> WasmPSS {
        let cmp = comparator_for_index(index_type);
        WasmPSS {
            inner: PersistentSortedSet::with_storage(cmp, storage, settings),
            index_type: Some(index_type),
        }
    }

    /// Restore a PSS from storage by root address.
    /// Called from datascript-rs to wire up UnifiedSQLiteStorage.
    pub fn new_restored(
        index_type: IndexType,
        address: i64,
        storage: Box<dyn IStorage>,
        settings: Settings,
    ) -> WasmPSS {
        WasmPSS {
            inner: PersistentSortedSet::restore(
                comparator_for_index(index_type),
                address,
                storage,
                settings,
            ),
            index_type: Some(index_type),
        }
    }
}

// ===========================================================================
// WasmSeq
// ===========================================================================

#[wasm_bindgen]
pub struct WasmSeq {
    inner: InnerSeq,
}

#[wasm_bindgen]
impl WasmSeq {
    /// Return the current datom as a JS object.
    pub fn first(&self) -> JsValue {
        datom_to_js(&self.inner.first())
    }

    pub fn next(&self) -> Option<WasmSeq> {
        self.inner.next().map(|s| WasmSeq { inner: s })
    }

    /// Seek forward/backward to a key.
    pub fn seek(&self, to: JsValue, cmp: JsValue) -> Option<WasmSeq> {
        let to_datom = datom_from_js(&to);

        let cmp_override: Option<Box<dyn Fn(&Key, &Key) -> Ordering>> =
            if cmp.is_undefined() || cmp.is_null() {
                None
            } else {
                let f: js_sys::Function = cmp.into();
                Some(Box::new(move |a: &Key, b: &Key| {
                    let js_a = datom_to_js(a);
                    let js_b = datom_to_js(b);
                    let result = f.call2(&JsValue::NULL, &js_a, &js_b).unwrap();
                    let n = result.as_f64().unwrap() as i32;
                    match n.cmp(&0) {
                        std::cmp::Ordering::Less => Ordering::Less,
                        std::cmp::Ordering::Equal => Ordering::Equal,
                        std::cmp::Ordering::Greater => Ordering::Greater,
                    }
                }))
            };

        let cmp_ref = cmp_override
            .as_ref()
            .map(|b| b.as_ref() as &dyn Fn(&Key, &Key) -> Ordering);

        self.inner.seek(&to_datom, cmp_ref).map(|s| WasmSeq { inner: s })
    }

    #[wasm_bindgen(js_name = "toArray")]
    pub fn to_array(&self) -> js_sys::Array {
        let arr = js_sys::Array::new();
        let keys = self.inner.to_vec();
        for k in keys {
            arr.push(&datom_to_js(&k));
        }
        arr
    }
}
