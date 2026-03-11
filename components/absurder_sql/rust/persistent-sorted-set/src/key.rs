//! Key type abstraction for the persistent sorted set.
//!
//! On native targets, keys are i64.
//! On wasm32 targets, keys are JsValue to support arbitrary JS/CLJS objects.

#[cfg(target_arch = "wasm32")]
pub type Key = wasm_bindgen::JsValue;

#[cfg(not(target_arch = "wasm32"))]
pub type Key = i64;
