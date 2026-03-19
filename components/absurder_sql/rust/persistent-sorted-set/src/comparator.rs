//! Datom comparators for EAVT, AEVT, and AVET index orderings.
//!
//! Three static comparator functions — no closures, no dynamic dispatch,
//! no WASM boundary crossings. These replicate the behavior of DataScript's
//! `cmp-datoms-eavt`, `cmp-datoms-aevt`, and `cmp-datoms-avet`.

use std::cmp::Ordering;

use crate::datom::{type_rank, Attr, Datom, Value};

/// Which index ordering to use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexType {
    EAVT,
    AEVT,
    AVET,
}

/// Short-circuit chained comparisons. Returns the first non-Equal result.
macro_rules! combine_cmp {
    ($first:expr $(, $rest:expr)*) => {{
        let ord = $first;
        if ord != Ordering::Equal {
            ord
        } else {
            combine_cmp!($($rest),*)
        }
    }};
    () => { Ordering::Equal };
}

/// Compare two datoms according to the given index ordering.
pub fn cmp_datoms(idx: IndexType, d1: &Datom, d2: &Datom) -> Ordering {
    match idx {
        IndexType::EAVT => combine_cmp!(
            d1.e.cmp(&d2.e),
            attr_cmp(&d1.a, &d2.a),
            value_cmp(&d1.v, &d2.v),
            tx_cmp(d1, d2)
        ),
        IndexType::AEVT => combine_cmp!(
            attr_cmp(&d1.a, &d2.a),
            d1.e.cmp(&d2.e),
            value_cmp(&d1.v, &d2.v),
            tx_cmp(d1, d2)
        ),
        IndexType::AVET => combine_cmp!(
            attr_cmp(&d1.a, &d2.a),
            value_cmp(&d1.v, &d2.v),
            d1.e.cmp(&d2.e),
            tx_cmp(d1, d2)
        ),
    }
}

/// Compare attributes. Returns Equal if either is None (wildcard/boundary).
fn attr_cmp(a: &Option<Attr>, b: &Option<Attr>) -> Ordering {
    match (a, b) {
        (None, _) | (_, None) => Ordering::Equal,
        (Some(a), Some(b)) => a.cmp(b),
    }
}

/// Compare values. Returns Equal if either is Nil (wildcard/boundary).
/// Same-type uses natural ordering; cross-type uses discriminant ordering.
pub fn value_cmp(a: &Value, b: &Value) -> Ordering {
    match (a, b) {
        (Value::Nil, _) | (_, Value::Nil) => Ordering::Equal,

        (Value::Bool(a), Value::Bool(b)) => a.cmp(b),

        // Numeric: Long/Ref are interchangeable (DataScript treats refs as plain ints)
        (Value::Long(a) | Value::Ref(a), Value::Long(b) | Value::Ref(b)) => a.cmp(b),
        (Value::Double(a), Value::Double(b)) => f64_cmp(*a, *b),
        (Value::Long(a) | Value::Ref(a), Value::Double(b)) => f64_cmp(*a as f64, *b),
        (Value::Double(a), Value::Long(b) | Value::Ref(b)) => f64_cmp(*a, *b as f64),

        (Value::Str(a), Value::Str(b)) => a.cmp(b),

        (Value::Keyword(a), Value::Keyword(b)) => a.cmp(b),

        (Value::Instant(a), Value::Instant(b)) => a.cmp(b),

        (Value::Uuid(a), Value::Uuid(b)) => a.cmp(b),

        (Value::Bytes(a), Value::Bytes(b)) => a.cmp(b),

        // Cross-type: order by discriminant
        _ => type_rank(a).cmp(&type_rank(b)),
    }
}

/// Compare transaction components. DataScript compares the absolute tx value
/// (the sign encodes added/retracted, not ordering).
fn tx_cmp(d1: &Datom, d2: &Datom) -> Ordering {
    d1.tx_id().cmp(&d2.tx_id())
}

/// Total-order f64 comparison (NaN sorts last, -0.0 == +0.0).
fn f64_cmp(a: f64, b: f64) -> Ordering {
    a.partial_cmp(&b).unwrap_or_else(|| {
        // At least one is NaN
        match (a.is_nan(), b.is_nan()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater, // NaN sorts last
            (false, true) => Ordering::Less,
            _ => unreachable!(),
        }
    })
}

/// "Quick" comparator variants that skip nil/None guards.
/// Use these during insertion/lookup where all fields are populated.
pub fn cmp_datoms_quick(idx: IndexType, d1: &Datom, d2: &Datom) -> Ordering {
    match idx {
        IndexType::EAVT => combine_cmp!(
            d1.e.cmp(&d2.e),
            attr_cmp_quick(&d1.a, &d2.a),
            value_cmp_quick(&d1.v, &d2.v),
            tx_cmp(d1, d2)
        ),
        IndexType::AEVT => combine_cmp!(
            attr_cmp_quick(&d1.a, &d2.a),
            d1.e.cmp(&d2.e),
            value_cmp_quick(&d1.v, &d2.v),
            tx_cmp(d1, d2)
        ),
        IndexType::AVET => combine_cmp!(
            attr_cmp_quick(&d1.a, &d2.a),
            value_cmp_quick(&d1.v, &d2.v),
            d1.e.cmp(&d2.e),
            tx_cmp(d1, d2)
        ),
    }
}

/// Compare attributes without nil guard — panics if either is None.
fn attr_cmp_quick(a: &Option<Attr>, b: &Option<Attr>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => a.cmp(b),
        _ => panic!("attr_cmp_quick called with None attribute"),
    }
}

/// Compare values without nil guard — panics if either is Nil.
fn value_cmp_quick(a: &Value, b: &Value) -> Ordering {
    match (a, b) {
        (Value::Nil, _) | (_, Value::Nil) => {
            panic!("value_cmp_quick called with Nil value")
        }
        _ => value_cmp(a, b),
    }
}

/// Parse an index type string ("eavt", "aevt", "avet") into an IndexType.
pub fn parse_index_type(s: &str) -> IndexType {
    match s.to_lowercase().as_str() {
        "eavt" => IndexType::EAVT,
        "aevt" => IndexType::AEVT,
        "avet" => IndexType::AVET,
        _ => panic!("Unknown index type: {}", s),
    }
}

/// Create a comparator closure from an IndexType.
/// This is the fast path — no WASM boundary crossings, pure Rust comparison.
#[cfg(target_arch = "wasm32")]
pub fn comparator_for_index(idx: IndexType) -> std::rc::Rc<crate::node::Comparator> {
    std::rc::Rc::new(move |a: &crate::datom::Datom, b: &crate::datom::Datom| {
        cmp_datoms(idx, a, b)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::{Attr, Datom, Value};

    fn kw(name: &str) -> Option<Attr> {
        Some(Attr::Keyword { ns: None, name: name.into() })
    }

    fn kw_ns(ns: &str, name: &str) -> Option<Attr> {
        Some(Attr::Keyword { ns: Some(ns.into()), name: name.into() })
    }

    fn datom(e: i64, a: Option<Attr>, v: Value, tx: i64) -> Datom {
        Datom::new(e, a, v, tx)
    }

    // --- EAVT ordering ---

    #[test]
    fn eavt_orders_by_entity_first() {
        let d1 = datom(1, kw("name"), Value::Str("Alice".into()), 100);
        let d2 = datom(2, kw("name"), Value::Str("Alice".into()), 100);
        assert_eq!(cmp_datoms(IndexType::EAVT, &d1, &d2), Ordering::Less);
        assert_eq!(cmp_datoms(IndexType::EAVT, &d2, &d1), Ordering::Greater);
    }

    #[test]
    fn eavt_same_entity_orders_by_attr() {
        let d1 = datom(1, kw("age"), Value::Long(30), 100);
        let d2 = datom(1, kw("name"), Value::Str("Alice".into()), 100);
        assert_eq!(cmp_datoms(IndexType::EAVT, &d1, &d2), Ordering::Less);
    }

    #[test]
    fn eavt_same_entity_attr_orders_by_value() {
        let d1 = datom(1, kw("age"), Value::Long(25), 100);
        let d2 = datom(1, kw("age"), Value::Long(30), 100);
        assert_eq!(cmp_datoms(IndexType::EAVT, &d1, &d2), Ordering::Less);
    }

    #[test]
    fn eavt_same_eav_orders_by_tx() {
        let d1 = datom(1, kw("age"), Value::Long(30), 100);
        let d2 = datom(1, kw("age"), Value::Long(30), 200);
        assert_eq!(cmp_datoms(IndexType::EAVT, &d1, &d2), Ordering::Less);
    }

    #[test]
    fn eavt_identical_datoms_are_equal() {
        let d1 = datom(1, kw("age"), Value::Long(30), 100);
        let d2 = datom(1, kw("age"), Value::Long(30), 100);
        assert_eq!(cmp_datoms(IndexType::EAVT, &d1, &d2), Ordering::Equal);
    }

    // --- AEVT ordering ---

    #[test]
    fn aevt_orders_by_attr_first() {
        let d1 = datom(2, kw("age"), Value::Long(30), 100);
        let d2 = datom(1, kw("name"), Value::Str("Alice".into()), 100);
        assert_eq!(cmp_datoms(IndexType::AEVT, &d1, &d2), Ordering::Less);
    }

    #[test]
    fn aevt_same_attr_orders_by_entity() {
        let d1 = datom(1, kw("name"), Value::Str("Alice".into()), 100);
        let d2 = datom(2, kw("name"), Value::Str("Bob".into()), 100);
        assert_eq!(cmp_datoms(IndexType::AEVT, &d1, &d2), Ordering::Less);
    }

    // --- AVET ordering ---

    #[test]
    fn avet_orders_by_attr_first() {
        let d1 = datom(1, kw("age"), Value::Long(30), 100);
        let d2 = datom(1, kw("name"), Value::Str("Bob".into()), 100);
        assert_eq!(cmp_datoms(IndexType::AVET, &d1, &d2), Ordering::Less);
    }

    #[test]
    fn avet_same_attr_orders_by_value_then_entity() {
        let d1 = datom(2, kw("name"), Value::Str("Alice".into()), 100);
        let d2 = datom(1, kw("name"), Value::Str("Bob".into()), 100);
        // "Alice" < "Bob"
        assert_eq!(cmp_datoms(IndexType::AVET, &d1, &d2), Ordering::Less);

        // Same value, different entity
        let d3 = datom(1, kw("name"), Value::Str("Alice".into()), 100);
        let d4 = datom(2, kw("name"), Value::Str("Alice".into()), 100);
        assert_eq!(cmp_datoms(IndexType::AVET, &d3, &d4), Ordering::Less);
    }

    // --- Wildcard/boundary datom semantics ---

    #[test]
    fn nil_attr_matches_any() {
        let d1 = datom(1, None, Value::Nil, 100);
        let d2 = datom(1, kw("name"), Value::Str("Alice".into()), 100);
        // None attr => Equal for attr component
        assert_eq!(cmp_datoms(IndexType::EAVT, &d1, &d2), Ordering::Equal);
    }

    #[test]
    fn nil_value_matches_any() {
        let d1 = datom(1, kw("age"), Value::Nil, 100);
        let d2 = datom(1, kw("age"), Value::Long(30), 100);
        assert_eq!(cmp_datoms(IndexType::EAVT, &d1, &d2), Ordering::Equal);
    }

    #[test]
    fn boundary_datom_all_wildcards() {
        let boundary = datom(5, None, Value::Nil, 0);
        let real = datom(5, kw_ns("person", "name"), Value::Str("Alice".into()), 200);
        // e matches, a is wildcard, v is wildcard, tx: 0 < 200
        // But since a is wildcard => Equal, so we continue to v (also wildcard => Equal),
        // then tx: 0 < 200 => Less
        assert_eq!(cmp_datoms(IndexType::EAVT, &boundary, &real), Ordering::Less);
    }

    // --- Value comparison edge cases ---

    #[test]
    fn cross_type_value_ordering() {
        assert_eq!(value_cmp(&Value::Bool(true), &Value::Long(1)), Ordering::Less);
        assert_eq!(value_cmp(&Value::Long(1), &Value::Str("1".into())), Ordering::Less);
        assert_eq!(value_cmp(&Value::Str("x".into()), &Value::Keyword(Attr::Keyword {
            ns: None, name: "x".into()
        })), Ordering::Less);
    }

    #[test]
    fn long_double_cross_comparison() {
        assert_eq!(value_cmp(&Value::Long(42), &Value::Double(42.0)), Ordering::Equal);
        assert_eq!(value_cmp(&Value::Long(42), &Value::Double(42.5)), Ordering::Less);
        assert_eq!(value_cmp(&Value::Double(41.9), &Value::Long(42)), Ordering::Less);
    }

    #[test]
    fn nan_sorts_last() {
        assert_eq!(value_cmp(&Value::Double(f64::NAN), &Value::Double(1.0)), Ordering::Greater);
        assert_eq!(value_cmp(&Value::Double(1.0), &Value::Double(f64::NAN)), Ordering::Less);
        assert_eq!(value_cmp(&Value::Double(f64::NAN), &Value::Double(f64::NAN)), Ordering::Equal);
    }

    #[test]
    fn bool_ordering() {
        assert_eq!(value_cmp(&Value::Bool(false), &Value::Bool(true)), Ordering::Less);
        assert_eq!(value_cmp(&Value::Bool(true), &Value::Bool(true)), Ordering::Equal);
    }

    #[test]
    fn uuid_ordering() {
        let u1 = [0u8; 16];
        let mut u2 = [0u8; 16];
        u2[15] = 1;
        assert_eq!(value_cmp(&Value::Uuid(u1), &Value::Uuid(u2)), Ordering::Less);
    }

    #[test]
    fn bytes_ordering() {
        assert_eq!(
            value_cmp(&Value::Bytes(vec![1, 2]), &Value::Bytes(vec![1, 3])),
            Ordering::Less
        );
    }

    #[test]
    fn namespaced_keyword_ordering() {
        let d1 = datom(1, kw_ns("db", "id"), Value::Long(1), 100);
        let d2 = datom(1, kw_ns("person", "name"), Value::Str("Alice".into()), 100);
        // "db" < "person"
        assert_eq!(cmp_datoms(IndexType::EAVT, &d1, &d2), Ordering::Less);
    }

    #[test]
    fn ref_value_ordering() {
        assert_eq!(value_cmp(&Value::Ref(1), &Value::Ref(2)), Ordering::Less);
        assert_eq!(value_cmp(&Value::Ref(5), &Value::Ref(5)), Ordering::Equal);
    }

    #[test]
    fn instant_ordering() {
        assert_eq!(value_cmp(&Value::Instant(1000), &Value::Instant(2000)), Ordering::Less);
    }

    // --- Negative tx (retraction) ---

    #[test]
    fn retracted_tx_uses_absolute_value() {
        let d1 = datom(1, kw("age"), Value::Long(30), -100);
        let d2 = datom(1, kw("age"), Value::Long(30), 100);
        assert_eq!(cmp_datoms(IndexType::EAVT, &d1, &d2), Ordering::Equal);

        let d3 = datom(1, kw("age"), Value::Long(30), -200);
        assert_eq!(cmp_datoms(IndexType::EAVT, &d1, &d3), Ordering::Less);
    }

    // --- Quick variant ---

    #[test]
    fn quick_variant_matches_normal() {
        let d1 = datom(1, kw("age"), Value::Long(25), 100);
        let d2 = datom(2, kw("name"), Value::Str("Bob".into()), 200);

        assert_eq!(
            cmp_datoms(IndexType::EAVT, &d1, &d2),
            cmp_datoms_quick(IndexType::EAVT, &d1, &d2)
        );
        assert_eq!(
            cmp_datoms(IndexType::AEVT, &d1, &d2),
            cmp_datoms_quick(IndexType::AEVT, &d1, &d2)
        );
        assert_eq!(
            cmp_datoms(IndexType::AVET, &d1, &d2),
            cmp_datoms_quick(IndexType::AVET, &d1, &d2)
        );
    }

    #[test]
    #[should_panic(expected = "attr_cmp_quick called with None")]
    fn quick_variant_panics_on_none_attr() {
        let d1 = datom(1, None, Value::Long(1), 100);
        let d2 = datom(1, kw("age"), Value::Long(1), 100);
        cmp_datoms_quick(IndexType::EAVT, &d1, &d2);
    }

    // --- Comprehensive multi-index sorting ---

    #[test]
    fn sorting_multiple_datoms_eavt() {
        let mut datoms = vec![
            datom(3, kw("age"), Value::Long(25), 100),
            datom(1, kw("name"), Value::Str("Alice".into()), 100),
            datom(1, kw("age"), Value::Long(30), 100),
            datom(2, kw("name"), Value::Str("Bob".into()), 200),
            datom(1, kw("age"), Value::Long(30), 200),
        ];
        datoms.sort_by(|a, b| cmp_datoms(IndexType::EAVT, a, b));

        // Expected EAVT order: e=1/age/30/100, e=1/age/30/200, e=1/name/Alice/100,
        //                      e=2/name/Bob/200, e=3/age/25/100
        assert_eq!(datoms[0].e, 1);
        assert_eq!(datoms[0].a, kw("age"));
        assert_eq!(datoms[0].tx, 100);

        assert_eq!(datoms[1].e, 1);
        assert_eq!(datoms[1].a, kw("age"));
        assert_eq!(datoms[1].tx, 200);

        assert_eq!(datoms[2].e, 1);
        assert_eq!(datoms[2].a, kw("name"));

        assert_eq!(datoms[3].e, 2);
        assert_eq!(datoms[4].e, 3);
    }

    #[test]
    fn sorting_multiple_datoms_aevt() {
        let mut datoms = vec![
            datom(1, kw("name"), Value::Str("Alice".into()), 100),
            datom(1, kw("age"), Value::Long(30), 100),
            datom(2, kw("age"), Value::Long(25), 200),
            datom(2, kw("name"), Value::Str("Bob".into()), 200),
        ];
        datoms.sort_by(|a, b| cmp_datoms(IndexType::AEVT, a, b));

        // AEVT: all "age" before "name", within same attr order by entity
        assert_eq!(datoms[0].a, kw("age"));
        assert_eq!(datoms[0].e, 1);
        assert_eq!(datoms[1].a, kw("age"));
        assert_eq!(datoms[1].e, 2);
        assert_eq!(datoms[2].a, kw("name"));
        assert_eq!(datoms[2].e, 1);
        assert_eq!(datoms[3].a, kw("name"));
        assert_eq!(datoms[3].e, 2);
    }

    #[test]
    fn sorting_multiple_datoms_avet() {
        let mut datoms = vec![
            datom(1, kw("name"), Value::Str("Bob".into()), 100),
            datom(2, kw("name"), Value::Str("Alice".into()), 200),
            datom(3, kw("age"), Value::Long(30), 100),
            datom(4, kw("age"), Value::Long(25), 200),
        ];
        datoms.sort_by(|a, b| cmp_datoms(IndexType::AVET, a, b));

        // AVET: "age" before "name"; within same attr, order by value then entity
        assert_eq!(datoms[0].a, kw("age"));
        assert_eq!(datoms[0].v, Value::Long(25)); // 25 < 30
        assert_eq!(datoms[0].e, 4);

        assert_eq!(datoms[1].a, kw("age"));
        assert_eq!(datoms[1].v, Value::Long(30));
        assert_eq!(datoms[1].e, 3);

        assert_eq!(datoms[2].a, kw("name"));
        assert_eq!(datoms[2].v, Value::Str("Alice".into())); // "Alice" < "Bob"
        assert_eq!(datoms[2].e, 2);

        assert_eq!(datoms[3].a, kw("name"));
        assert_eq!(datoms[3].v, Value::Str("Bob".into()));
        assert_eq!(datoms[3].e, 1);
    }
}
