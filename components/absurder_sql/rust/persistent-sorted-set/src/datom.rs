//! Core Datom types for DataScript indexes.
//!
//! These types give Rust first-class knowledge of datoms so that comparisons,
//! storage serialization, and index slicing can happen entirely in Rust
//! without crossing the WASM boundary.

/// Attribute — keyword (ns/name) or plain string.
/// Matches DataScript's keyword attributes (e.g. :db/id, :person/name).
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum Attr {
    Keyword { ns: Option<String>, name: String },
    Str(String),
}

impl Ord for Attr {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        match (self, other) {
            (Attr::Keyword { ns: ns1, name: n1 }, Attr::Keyword { ns: ns2, name: n2 }) => {
                // Keywords: compare namespace first (None < Some), then name
                match (ns1, ns2) {
                    (None, Some(_)) => Ordering::Less,
                    (Some(_), None) => Ordering::Greater,
                    (None, None) => n1.cmp(n2),
                    (Some(a), Some(b)) => a.cmp(b).then(n1.cmp(n2)),
                }
            }
            (Attr::Str(a), Attr::Str(b)) => a.cmp(b),
            // Keywords sort before strings (matches CLJS class-compare)
            (Attr::Keyword { .. }, Attr::Str(_)) => Ordering::Less,
            (Attr::Str(_), Attr::Keyword { .. }) => Ordering::Greater,
        }
    }
}

impl PartialOrd for Attr {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Fixed discriminant for cross-type value ordering.
/// Matches the CLJS `class-compare` order.
fn value_type_rank(v: &Value) -> u8 {
    match v {
        Value::Nil => 0,
        Value::Bool(_) => 1,
        Value::Long(_) => 2,
        Value::Double(_) => 3,
        Value::Str(_) => 4,
        Value::Keyword(_) => 5,
        Value::Ref(_) => 6,
        Value::Instant(_) => 7,
        Value::Uuid(_) => 8,
        Value::Bytes(_) => 9,
    }
}

/// Datom value covering all types DataScript's value-compare handles.
#[derive(Clone, Debug)]
pub enum Value {
    /// Sentinel for boundary/wildcard datoms.
    /// `value_cmp(Nil, _) => Equal` and `value_cmp(_, Nil) => Equal`.
    Nil,
    Bool(bool),
    Long(i64),
    Double(f64),
    Str(String),
    Keyword(Attr),
    /// Entity reference
    Ref(i64),
    /// Epoch millis
    Instant(i64),
    Uuid([u8; 16]),
    Bytes(Vec<u8>),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
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
}

impl Eq for Value {}

/// A single datom: [entity, attribute, value, transaction].
#[derive(Clone, Debug)]
pub struct Datom {
    pub e: i64,
    /// None = wildcard (boundary datom). Comparator returns Equal for None attrs.
    pub a: Option<Attr>,
    /// Value::Nil = wildcard. Comparator returns Equal for Nil values.
    pub v: Value,
    /// Transaction id. Negative means retraction (high bit encodes added/retracted).
    pub tx: i64,
    /// Original CLJS datom object, preserved for lossless round-trip.
    #[cfg(target_arch = "wasm32")]
    pub original_js: Option<wasm_bindgen::JsValue>,
}

impl PartialEq for Datom {
    fn eq(&self, other: &Self) -> bool {
        self.e == other.e && self.a == other.a && self.v == other.v && self.tx == other.tx
    }
}

impl Datom {
    pub fn new(e: i64, a: Option<Attr>, v: Value, tx: i64) -> Self {
        Self {
            e,
            a,
            v,
            tx,
            #[cfg(target_arch = "wasm32")]
            original_js: None,
        }
    }

    /// Transaction id without the added/retracted flag.
    pub fn tx_id(&self) -> i64 {
        if self.tx < 0 { -self.tx } else { self.tx }
    }
}

/// Helper to get the value type rank (public for comparator module).
pub(crate) fn type_rank(v: &Value) -> u8 {
    value_type_rank(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attr_keyword_ordering() {
        let a1 = Attr::Keyword { ns: None, name: "age".into() };
        let a2 = Attr::Keyword { ns: None, name: "name".into() };
        let a3 = Attr::Keyword { ns: Some("person".into()), name: "age".into() };
        let a4 = Attr::Keyword { ns: Some("person".into()), name: "name".into() };
        let a5 = Attr::Keyword { ns: Some("db".into()), name: "id".into() };

        // No-ns keywords sort by name
        assert!(a1 < a2);
        // No-ns < some-ns
        assert!(a1 < a3);
        assert!(a2 < a3);
        // Same ns, different name
        assert!(a3 < a4);
        // Different ns
        assert!(a5 < a3); // "db" < "person"
    }

    #[test]
    fn attr_keyword_before_str() {
        let kw = Attr::Keyword { ns: None, name: "foo".into() };
        let s = Attr::Str("foo".into());
        assert!(kw < s);
    }

    #[test]
    fn value_equality() {
        assert_eq!(Value::Long(42), Value::Long(42));
        assert_ne!(Value::Long(42), Value::Long(43));
        assert_ne!(Value::Long(42), Value::Str("42".into()));
    }

    #[test]
    fn datom_tx_id() {
        let d1 = Datom::new(1, None, Value::Nil, 100);
        assert_eq!(d1.tx_id(), 100);

        let d2 = Datom::new(1, None, Value::Nil, -100);
        assert_eq!(d2.tx_id(), 100);
    }
}
