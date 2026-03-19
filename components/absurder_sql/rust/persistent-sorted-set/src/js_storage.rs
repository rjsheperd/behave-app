//! JS-callback storage bridge.
//! Implements IStorage by calling back to JS functions for store/restore.
//! Only compiled for wasm32 targets.
//!
//! Keys are serialized as JS arrays of [e, a, v, tx] for each datom.

use std::rc::Rc;

use js_sys;
use wasm_bindgen::prelude::*;

use crate::key::Key;
use crate::node::Node;
use crate::settings::Settings;
use crate::storage::IStorage;
use crate::wasm::{datom_from_js, datom_to_js};

/// Storage that delegates to JS callback functions.
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
        let obj = js_sys::Object::new();
        let level = node.level();

        js_sys::Reflect::set(
            &obj,
            &JsValue::from_str("level"),
            &JsValue::from_f64(level as f64),
        )
        .unwrap();

        // Serialize datom keys as JS array of datom objects
        let keys_arr = js_sys::Array::new();
        for k in node.keys() {
            keys_arr.push(&datom_to_js(k));
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
        let result = match self
            .restore_fn
            .call1(&self.obj, &JsValue::from_f64(address as f64))
        {
            Ok(v) => v,
            Err(_e) => {
                // JS restore callback threw — return an empty leaf to avoid panic
                return Rc::new(Node::restore(0, vec![], None, &self.settings));
            }
        };

        let level = js_sys::Reflect::get(&result, &JsValue::from_str("level"))
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as u32;

        let keys_val = js_sys::Reflect::get(&result, &JsValue::from_str("keys"))
            .unwrap_or(JsValue::from(js_sys::Array::new()));
        let keys_arr: js_sys::Array = keys_val.into();
        let mut keys: Vec<Key> = Vec::with_capacity(keys_arr.length() as usize);
        for i in 0..keys_arr.length() {
            keys.push(datom_from_js(&keys_arr.get(i)));
        }

        let addresses: Option<Vec<i64>> = js_sys::Reflect::get(&result, &JsValue::from_str("addresses"))
            .ok()
            .filter(|v| !v.is_undefined() && !v.is_null())
            .map(|v| {
                let arr: js_sys::Array = v.into();
                (0..arr.length())
                    .map(|i| {
                        let val = arr.get(i);
                        val.as_f64().unwrap_or_else(|| {
                            // CLJS edn/read-string may produce non-f64 values
                            // (e.g., BigInt or string) — coerce via JS Number()
                            js_sys::Number::from(val).value_of()
                        }) as i64
                    })
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
        vec![]
    }

    fn delete(&mut self, _addresses: &[i64]) {
    }
}
