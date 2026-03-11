//! JS-callback storage bridge for Phase 2.
//! Implements IStorage by calling back to JS functions for store/restore.
//! Only compiled for wasm32 targets.

use std::rc::Rc;

use js_sys;
use wasm_bindgen::prelude::*;

use crate::key::Key;
use crate::node::Node;
use crate::settings::Settings;
use crate::storage::IStorage;

/// Storage that delegates to JS callback functions.
/// Keeps a reference to the original JS object so methods are called with
/// the correct `this` binding.
pub struct JsStorage {
    obj: JsValue,
    store_fn: js_sys::Function,
    restore_fn: js_sys::Function,
    accessed_fn: Option<js_sys::Function>,
    settings: Settings,
}

impl JsStorage {
    /// Create from a JS object with `store`, `restore`, and optional `accessed` methods.
    pub fn from_js_object(obj: &JsValue) -> Self {
        let store_fn: js_sys::Function =
            js_sys::Reflect::get(obj, &JsValue::from_str("store"))
                .expect("storage object must have 'store' method")
                .into();

        let restore_fn: js_sys::Function =
            js_sys::Reflect::get(obj, &JsValue::from_str("restore"))
                .expect("storage object must have 'restore' method")
                .into();

        let accessed_fn = js_sys::Reflect::get(obj, &JsValue::from_str("accessed"))
            .ok()
            .filter(|v| v.is_function())
            .map(|v| v.into());

        let bf = js_sys::Reflect::get(obj, &JsValue::from_str("branchingFactor"))
            .ok()
            .and_then(|v| v.as_f64())
            .map(|n| n as usize)
            .unwrap_or(512);

        Self {
            obj: obj.clone(),
            store_fn,
            restore_fn,
            accessed_fn,
            settings: Settings::new(bf),
        }
    }
}

impl IStorage for JsStorage {
    fn store(&mut self, node: &Node) -> i64 {
        // Serialize node to a JS object:
        // { level: number, keys: Array<JsValue>, addresses?: Array<number> }
        let obj = js_sys::Object::new();
        let level = node.level();

        js_sys::Reflect::set(
            &obj,
            &JsValue::from_str("level"),
            &JsValue::from_f64(level as f64),
        )
        .unwrap();

        let keys_arr = js_sys::Array::new();
        for k in node.keys() {
            keys_arr.push(k);
        }
        js_sys::Reflect::set(&obj, &JsValue::from_str("keys"), &keys_arr).unwrap();

        if let Node::Branch(b) = node {
            let addrs_arr = js_sys::Array::new();
            for i in 0..b.len() {
                let addr = b.address(i).unwrap_or(-1);
                addrs_arr.push(&JsValue::from_f64(addr as f64));
            }
            js_sys::Reflect::set(&obj, &JsValue::from_str("addresses"), &addrs_arr).unwrap();
        }

        let result = self.store_fn.call1(&self.obj, &obj).unwrap();
        result.as_f64().unwrap() as i64
    }

    fn restore(&self, address: i64) -> Rc<Node> {
        let result = self
            .restore_fn
            .call1(&self.obj, &JsValue::from_f64(address as f64))
            .unwrap();

        // Expect: { level: number, keys: Array<JsValue>, addresses?: Array<number> }
        let level = js_sys::Reflect::get(&result, &JsValue::from_str("level"))
            .unwrap()
            .as_f64()
            .unwrap() as u32;

        let keys_val = js_sys::Reflect::get(&result, &JsValue::from_str("keys")).unwrap();
        let keys_arr: js_sys::Array = keys_val.into();
        let mut keys: Vec<Key> = Vec::with_capacity(keys_arr.length() as usize);
        for i in 0..keys_arr.length() {
            keys.push(keys_arr.get(i));
        }

        let addresses: Option<Vec<i64>> = js_sys::Reflect::get(&result, &JsValue::from_str("addresses"))
            .ok()
            .filter(|v| !v.is_undefined() && !v.is_null())
            .map(|v| {
                let arr: js_sys::Array = v.into();
                (0..arr.length())
                    .map(|i| arr.get(i).as_f64().unwrap() as i64)
                    .collect()
            });

        let node = Node::restore(level, keys, addresses, &self.settings);
        Rc::new(node)
    }

    fn accessed(&self, address: i64) {
        if let Some(f) = &self.accessed_fn {
            let _ = f.call1(&self.obj, &JsValue::from_f64(address as f64));
        }
    }

    fn list_addresses(&self) -> Vec<i64> {
        vec![] // Not needed for JS bridge — GC handled on CLJS side
    }

    fn delete(&mut self, _addresses: &[i64]) {
        // Handled by CLJS side
    }
}
