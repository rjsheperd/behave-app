//! wasm-bindgen exports wrapping the inner Rust types.
//! Only compiled for wasm32 targets.

use std::cmp::Ordering;
use std::rc::Rc;

use js_sys;
use wasm_bindgen::prelude::*;

use crate::js_storage::JsStorage;
use crate::node::Comparator;
use crate::seq::Seq as InnerSeq;
use crate::set::PersistentSortedSet;
use crate::settings::Settings;

/// Convert a JS comparator function into a Rust Comparator.
fn js_to_comparator(f: js_sys::Function) -> Rc<Comparator> {
    Rc::new(move |a: &JsValue, b: &JsValue| {
        let result = f.call2(&JsValue::NULL, a, b)
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

#[wasm_bindgen]
pub struct WasmPSS {
    inner: PersistentSortedSet,
}

#[wasm_bindgen]
impl WasmPSS {
    /// Install the panic hook for better error messages.
    #[wasm_bindgen(js_name = "setPanicHook")]
    pub fn set_panic_hook() {
        console_error_panic_hook::set_once();
    }

    /// Create an empty set with the given JS comparator function.
    #[wasm_bindgen(js_name = "empty")]
    pub fn empty(cmp: js_sys::Function) -> WasmPSS {
        WasmPSS {
            inner: PersistentSortedSet::empty(js_to_comparator(cmp)),
        }
    }

    /// Create an empty set with comparator and storage callbacks.
    /// `storage` should be an object with `store` and `restore` functions.
    /// `settings` should be an object with `branchingFactor` number.
    #[wasm_bindgen(js_name = "withComparatorAndStorage")]
    pub fn with_cmp_and_storage(
        cmp: js_sys::Function,
        storage: JsValue,
        settings: JsValue,
    ) -> WasmPSS {
        let bf = js_sys::Reflect::get(&settings, &JsValue::from_str("branchingFactor"))
            .ok()
            .and_then(|v| v.as_f64())
            .map(|n| n as usize)
            .unwrap_or(512);

        let rust_settings = Settings::new(bf);
        let comparator = js_to_comparator(cmp);

        if storage.is_undefined() || storage.is_null() {
            WasmPSS {
                inner: PersistentSortedSet::empty_with_settings(comparator, rust_settings),
            }
        } else {
            let js_storage = JsStorage::from_js_object(&storage);
            WasmPSS {
                inner: PersistentSortedSet::with_storage(
                    comparator,
                    Box::new(js_storage),
                    rust_settings,
                ),
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
            let key = arr.get(i);
            set = set.conj(&key);
        }
        WasmPSS { inner: set }
    }

    /// Restore from storage address (lazy — root not loaded until accessed).
    #[wasm_bindgen(js_name = "restore")]
    pub fn restore(
        cmp: js_sys::Function,
        address: f64,
        storage: JsValue,
        settings: JsValue,
    ) -> WasmPSS {
        let bf = js_sys::Reflect::get(&settings, &JsValue::from_str("branchingFactor"))
            .ok()
            .and_then(|v| v.as_f64())
            .map(|n| n as usize)
            .unwrap_or(512);

        let rust_settings = Settings::new(bf);
        let js_storage = JsStorage::from_js_object(&storage);

        WasmPSS {
            inner: PersistentSortedSet::restore(
                js_to_comparator(cmp),
                address as i64,
                Box::new(js_storage),
                rust_settings,
            ),
        }
    }

    /// Add a key. Optional comparator override (pass undefined to use default).
    pub fn conj(&self, key: JsValue, _cmp: JsValue) -> WasmPSS {
        // Clone the inner set (cheap - Rc sharing) then conj
        WasmPSS {
            inner: self.inner.clone().conj(&key),
        }
    }

    /// Remove a key. Optional comparator override.
    pub fn disj(&self, key: JsValue, _cmp: JsValue) -> WasmPSS {
        WasmPSS {
            inner: self.inner.clone().disj(&key),
        }
    }

    pub fn contains(&self, key: JsValue) -> bool {
        self.inner.contains(&key)
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
            Some(from)
        };
        let to_opt = if to.is_undefined() || to.is_null() {
            None
        } else {
            Some(to)
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
            Some(from)
        };
        let to_opt = if to.is_undefined() || to.is_null() {
            None
        } else {
            Some(to)
        };
        self.inner
            .rslice(from_opt.as_ref(), to_opt.as_ref())
            .map(|s| WasmSeq { inner: s })
    }

    /// Store tree to storage, returns root address.
    /// If the set doesn't already have storage configured, the passed-in
    /// storage adapter object is used to set it up.
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
        // Structural equality would require iterating both sets
        false
    }

    #[wasm_bindgen(js_name = "hashCode")]
    pub fn hash_code(&self) -> i32 {
        0 // placeholder
    }

    #[wasm_bindgen(js_name = "toArray")]
    pub fn to_array(&self) -> js_sys::Array {
        let arr = js_sys::Array::new();
        let keys = self.inner.to_vec();
        for k in keys {
            arr.push(&k);
        }
        arr
    }

    /// Branching factor from settings.
    #[wasm_bindgen(js_name = "branchingFactor")]
    pub fn branching_factor(&self) -> usize {
        self.inner.settings().branching_factor()
    }
}

#[wasm_bindgen]
pub struct WasmSeq {
    inner: InnerSeq,
}

#[wasm_bindgen]
impl WasmSeq {
    pub fn first(&self) -> JsValue {
        self.inner.first()
    }

    pub fn next(&self) -> Option<WasmSeq> {
        self.inner.next().map(|s| WasmSeq { inner: s })
    }

    /// Seek forward/backward to a key.
    pub fn seek(&self, to: JsValue, cmp: JsValue) -> Option<WasmSeq> {
        let cmp_override: Option<Box<dyn Fn(&JsValue, &JsValue) -> Ordering>> =
            if cmp.is_undefined() || cmp.is_null() {
                None
            } else {
                let f: js_sys::Function = cmp.into();
                Some(Box::new(move |a: &JsValue, b: &JsValue| {
                    let result = f.call2(&JsValue::NULL, a, b).unwrap();
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
            .map(|b| b.as_ref() as &dyn Fn(&JsValue, &JsValue) -> Ordering);

        self.inner.seek(&to, cmp_ref).map(|s| WasmSeq { inner: s })
    }

    #[wasm_bindgen(js_name = "toArray")]
    pub fn to_array(&self) -> js_sys::Array {
        let arr = js_sys::Array::new();
        let keys = self.inner.to_vec();
        for k in keys {
            arr.push(&k);
        }
        arr
    }
}
