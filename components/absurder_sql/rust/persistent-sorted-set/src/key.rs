//! Key type abstraction for the persistent sorted set.
//!
//! On native targets, keys are i64 (for testing and simple use cases).
//! On wasm32 targets, keys are Datom — first-class datom knowledge in Rust,
//! no WASM boundary crossings for comparisons.

#[cfg(target_arch = "wasm32")]
pub type Key = crate::datom::Datom;

#[cfg(not(target_arch = "wasm32"))]
pub type Key = i64;
