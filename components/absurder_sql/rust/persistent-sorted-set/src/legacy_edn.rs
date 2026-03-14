//! EDN serde for legacy `.bp7` DataScript format.
//!
//! Platform-independent functions for parsing and generating EDN text
//! used in the `datascript` table of production `.bp7` files.

use edn::parser::Parser;
use edn::Value as EdnValue;

use crate::datom::{Attr, Datom, Value};
use crate::schema::{
    AttrSchema, Cardinality, ReverseSchema, Schema, Unique, ValueType, build_rschema,
};

// ---------------------------------------------------------------------------
// EDN parsing helpers
// ---------------------------------------------------------------------------

pub fn parse_edn(s: &str) -> EdnValue {
    let mut parser = Parser::new(s);
    parser.read().expect("failed to parse EDN").expect("empty EDN")
}

/// Check if an EDN Keyword string matches a given name (ignoring namespace).
pub fn kw_matches(kw: &str, name: &str) -> bool {
    let stripped = kw.strip_prefix(':').unwrap_or(kw);
    if let Some((_ns, local)) = stripped.rsplit_once('/') {
        local == name
    } else {
        stripped == name
    }
}

pub fn edn_get_int(map: &EdnValue, key: &str) -> i64 {
    if let EdnValue::Map(ref m) = map {
        for (k, v) in m.iter() {
            if let EdnValue::Keyword(ref kw) = k {
                if kw_matches(kw, key) {
                    return match v {
                        EdnValue::Integer(n) => *n,
                        _ => panic!("expected integer for key {}, got {:?}", key, v),
                    };
                }
            }
        }
    }
    panic!("key {} not found in EDN map", key);
}

pub fn edn_get<'a>(map: &'a EdnValue, key: &str) -> Option<&'a EdnValue> {
    if let EdnValue::Map(ref m) = map {
        for (k, v) in m.iter() {
            if let EdnValue::Keyword(ref kw) = k {
                if kw_matches(kw, key) {
                    return Some(v);
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Attr ↔ EDN
// ---------------------------------------------------------------------------

pub fn attr_from_edn_keyword(kw: &str) -> Attr {
    let stripped = kw.strip_prefix(':').unwrap_or(kw);
    if let Some(idx) = stripped.find('/') {
        Attr::Keyword {
            ns: Some(stripped[..idx].to_string()),
            name: stripped[idx + 1..].to_string(),
        }
    } else {
        Attr::Keyword {
            ns: None,
            name: stripped.to_string(),
        }
    }
}

pub fn attr_to_edn(attr: &Attr) -> String {
    match attr {
        Attr::Keyword { ns: Some(ns), name } => format!(":{}/{}", ns, name),
        Attr::Keyword { ns: None, name } => format!(":{}", name),
        Attr::Str(s) => format!("\"{}\"", s),
    }
}

// ---------------------------------------------------------------------------
// Value ↔ EDN
// ---------------------------------------------------------------------------

pub fn value_from_edn(v: &EdnValue, is_ref: bool) -> Value {
    match v {
        EdnValue::Nil => Value::Nil,
        EdnValue::Boolean(b) => Value::Bool(*b),
        EdnValue::Integer(n) => {
            if is_ref { Value::Ref(*n) } else { Value::Long(*n) }
        }
        EdnValue::Float(f) => Value::Double(f.into_inner()),
        EdnValue::String(s) => Value::Str(s.clone()),
        EdnValue::Keyword(kw) => Value::Keyword(attr_from_edn_keyword(kw)),
        _ => Value::Str(format!("{:?}", v)),
    }
}

pub fn value_to_edn(v: &Value) -> String {
    match v {
        Value::Nil => "nil".to_string(),
        Value::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        Value::Long(n) => n.to_string(),
        Value::Double(f) => {
            let s = f.to_string();
            if s.contains('.') { s } else { format!("{}.0", s) }
        }
        Value::Str(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
        Value::Keyword(attr) => attr_to_edn(attr),
        Value::Ref(n) => n.to_string(),
        Value::Instant(ms) => format!("#inst \"{}\"", ms),
        Value::Uuid(bytes) => {
            let hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
            format!(
                "#uuid \"{}-{}-{}-{}-{}\"",
                &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]
            )
        }
        Value::Bytes(b) => format!("{:?}", b),
    }
}

// ---------------------------------------------------------------------------
// Datom ↔ EDN
// ---------------------------------------------------------------------------

pub fn datom_from_edn(vec: &[EdnValue], rschema: &ReverseSchema) -> Datom {
    assert!(vec.len() == 4, "datom must have 4 fields, got {}", vec.len());

    let e = match &vec[0] {
        EdnValue::Integer(n) => *n,
        _ => panic!("datom entity must be integer, got {:?}", vec[0]),
    };

    let a = match &vec[1] {
        EdnValue::Keyword(kw) => attr_from_edn_keyword(kw),
        _ => panic!("datom attribute must be keyword, got {:?}", vec[1]),
    };

    let is_ref = rschema.is_ref(&a);
    let v = value_from_edn(&vec[2], is_ref);

    let tx = match &vec[3] {
        EdnValue::Integer(n) => *n,
        _ => panic!("datom tx must be integer, got {:?}", vec[3]),
    };

    Datom::new(e, Some(a), v, tx)
}

pub fn datom_to_edn(d: &Datom) -> String {
    let a_str = match &d.a {
        Some(attr) => attr_to_edn(attr),
        None => "nil".to_string(),
    };
    format!("[{} {} {} {}]", d.e, a_str, value_to_edn(&d.v), d.tx)
}

// ---------------------------------------------------------------------------
// Node ↔ EDN
// ---------------------------------------------------------------------------

pub fn node_from_edn(
    edn: &EdnValue,
    rschema: &ReverseSchema,
) -> (u32, Vec<Datom>, Option<Vec<i64>>) {
    let level = edn_get_int(edn, "level") as u32;

    let keys_val = edn_get(edn, "keys").expect("node must have :keys");
    let keys: Vec<Datom> = if let EdnValue::Vector(ref arr) = keys_val {
        arr.iter()
            .map(|datom| {
                if let EdnValue::Vector(ref fields) = datom {
                    datom_from_edn(fields, rschema)
                } else {
                    panic!("expected datom vector, got {:?}", datom);
                }
            })
            .collect()
    } else {
        panic!("expected :keys to be a vector");
    };

    let addresses = if level > 0 {
        let addrs_val = edn_get(edn, "addresses").expect("branch must have :addresses");
        if let EdnValue::Vector(ref arr) = addrs_val {
            Some(
                arr.iter()
                    .map(|v| match v {
                        EdnValue::Integer(n) => *n,
                        _ => panic!("expected integer address"),
                    })
                    .collect(),
            )
        } else {
            panic!("expected :addresses to be a vector");
        }
    } else {
        None
    };

    (level, keys, addresses)
}

pub fn node_to_edn(level: u32, keys: &[Datom], addresses: Option<&[i64]>) -> String {
    let keys_str: Vec<String> = keys.iter().map(|d| datom_to_edn(d)).collect();
    let keys_edn = format!("[{}]", keys_str.join(" "));

    if let Some(addrs) = addresses {
        let addrs_str: Vec<String> = addrs.iter().map(|a| a.to_string()).collect();
        format!(
            "{{:level {}, :keys {}, :addresses [{}]}}",
            level,
            keys_edn,
            addrs_str.join(" ")
        )
    } else {
        format!("{{:level {}, :keys {}}}", level, keys_edn)
    }
}

// ---------------------------------------------------------------------------
// Schema ↔ EDN
// ---------------------------------------------------------------------------

pub fn schema_from_edn(edn: &EdnValue) -> Schema {
    let mut schema = Schema::default();

    let map = match edn {
        EdnValue::Map(m) => m,
        _ => return schema,
    };

    for (k, v) in map.iter() {
        let attr = match k {
            EdnValue::Keyword(kw) => attr_from_edn_keyword(kw),
            _ => continue,
        };

        let props = match v {
            EdnValue::Map(m) => m,
            _ => continue,
        };

        let mut attr_schema = AttrSchema::default();

        for (pk, pv) in props.iter() {
            if let EdnValue::Keyword(ref prop_kw) = pk {
                // edn crate stores keywords WITHOUT the leading colon
                match prop_kw.as_str() {
                    "db/index" => {
                        if matches!(pv, EdnValue::Boolean(true)) {
                            attr_schema.index = true;
                        }
                    }
                    "db/valueType" => {
                        if let EdnValue::Keyword(ref vt) = pv {
                            if vt == "db.type/ref" {
                                attr_schema.value_type = Some(ValueType::Ref);
                            }
                        }
                    }
                    "db/unique" => {
                        if let EdnValue::Keyword(ref u) = pv {
                            match u.as_str() {
                                "db.unique/identity" => {
                                    attr_schema.unique = Some(Unique::Identity);
                                }
                                "db.unique/value" => {
                                    attr_schema.unique = Some(Unique::Value);
                                }
                                _ => {}
                            }
                        }
                    }
                    "db/cardinality" => {
                        if let EdnValue::Keyword(ref c) = pv {
                            if c == "db.cardinality/many" {
                                attr_schema.cardinality = Cardinality::Many;
                            }
                        }
                    }
                    "db/isComponent" => {
                        if matches!(pv, EdnValue::Boolean(true)) {
                            attr_schema.is_component = true;
                        }
                    }
                    _ => {}
                }
            }
        }

        schema.attrs.insert(attr, attr_schema);
    }

    schema
}

pub fn schema_to_edn(schema: &Schema) -> String {
    if schema.attrs.is_empty() {
        return "{}".to_string();
    }

    let mut entries = Vec::new();
    for (attr, s) in &schema.attrs {
        let mut props = Vec::new();
        if s.index {
            props.push(":db/index true".to_string());
        }
        match &s.unique {
            Some(Unique::Identity) => {
                props.push(":db/unique :db.unique/identity".to_string());
            }
            Some(Unique::Value) => {
                props.push(":db/unique :db.unique/value".to_string());
            }
            None => {}
        }
        if s.cardinality == Cardinality::Many {
            props.push(":db/cardinality :db.cardinality/many".to_string());
        }
        if s.value_type == Some(ValueType::Ref) {
            props.push(":db/valueType :db.type/ref".to_string());
        }
        if s.is_component {
            props.push(":db/isComponent true".to_string());
        }

        entries.push(format!("{} {{{}}}", attr_to_edn(attr), props.join(", ")));
    }

    format!("{{{}}}", entries.join(", "))
}

// ---------------------------------------------------------------------------
// Metadata
// ---------------------------------------------------------------------------

pub struct LegacyMetadata {
    pub schema: Schema,
    pub rschema: ReverseSchema,
    pub eavt_root: i64,
    pub aevt_root: i64,
    pub avet_root: i64,
    pub max_eid: i64,
    pub max_tx: i64,
    pub max_addr: i64,
    pub branching_factor: usize,
}

pub fn metadata_from_edn(edn: &EdnValue) -> LegacyMetadata {
    let schema_val = edn_get(edn, "schema").expect("metadata must have :schema");
    let schema = schema_from_edn(schema_val);
    let rschema = build_rschema(&schema);

    LegacyMetadata {
        schema,
        rschema,
        eavt_root: edn_get_int(edn, "eavt"),
        aevt_root: edn_get_int(edn, "aevt"),
        avet_root: edn_get_int(edn, "avet"),
        max_eid: edn_get_int(edn, "max-eid"),
        max_tx: edn_get_int(edn, "max-tx"),
        max_addr: edn_get_int(edn, "max-addr"),
        branching_factor: edn_get_int(edn, "branching-factor") as usize,
    }
}

pub fn metadata_to_edn(meta: &LegacyMetadata) -> String {
    let schema_edn = schema_to_edn(&meta.schema);
    format!(
        "{{:schema {}, :eavt {}, :aevt {}, :avet {}, :max-eid {}, :max-tx {}, :max-addr {}, :branching-factor {}}}",
        schema_edn,
        meta.eavt_root,
        meta.aevt_root,
        meta.avet_root,
        meta.max_eid,
        meta.max_tx,
        meta.max_addr,
        meta.branching_factor,
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::kw;

    fn test_rschema() -> ReverseSchema {
        let mut schema = Schema::default();
        schema.attrs.insert(
            Attr::Keyword { ns: Some("person".into()), name: "name".into() },
            AttrSchema { index: true, ..Default::default() },
        );
        schema.attrs.insert(
            Attr::Keyword { ns: Some("person".into()), name: "parent".into() },
            AttrSchema {
                value_type: Some(ValueType::Ref),
                ..Default::default()
            },
        );
        schema.attrs.insert(
            Attr::Keyword { ns: Some("worksheet".into()), name: "created".into() },
            AttrSchema::default(),
        );
        build_rschema(&schema)
    }

    // --- attr_from_edn_keyword ---

    #[test]
    fn attr_from_edn_keyword_namespaced() {
        let attr = attr_from_edn_keyword(":person/name");
        assert_eq!(
            attr,
            Attr::Keyword { ns: Some("person".into()), name: "name".into() }
        );
    }

    #[test]
    fn attr_from_edn_keyword_simple() {
        let attr = attr_from_edn_keyword(":age");
        assert_eq!(attr, Attr::Keyword { ns: None, name: "age".into() });
    }

    #[test]
    fn attr_from_edn_keyword_no_colon() {
        let attr = attr_from_edn_keyword("person/name");
        assert_eq!(
            attr,
            Attr::Keyword { ns: Some("person".into()), name: "name".into() }
        );
    }

    // --- value_from_edn ---

    #[test]
    fn value_from_edn_long() {
        let v = value_from_edn(&EdnValue::Integer(42), false);
        assert_eq!(v, Value::Long(42));
    }

    #[test]
    fn value_from_edn_ref() {
        let v = value_from_edn(&EdnValue::Integer(42), true);
        assert_eq!(v, Value::Ref(42));
    }

    #[test]
    fn value_from_edn_string() {
        let v = value_from_edn(&EdnValue::String("hello".into()), false);
        assert_eq!(v, Value::Str("hello".into()));
    }

    #[test]
    fn value_from_edn_bool() {
        let v = value_from_edn(&EdnValue::Boolean(true), false);
        assert_eq!(v, Value::Bool(true));
    }

    #[test]
    fn value_from_edn_keyword() {
        let v = value_from_edn(&EdnValue::Keyword(":fuel/timber".into()), false);
        assert_eq!(
            v,
            Value::Keyword(Attr::Keyword { ns: Some("fuel".into()), name: "timber".into() })
        );
    }

    #[test]
    fn value_from_edn_float() {
        let edn = parse_edn("3.14");
        let v = value_from_edn(&edn, false);
        match v {
            Value::Double(f) => assert!((f - 3.14).abs() < 1e-10),
            _ => panic!("expected Double, got {:?}", v),
        }
    }

    // --- datom_from_edn ---

    #[test]
    fn datom_from_edn_basic() {
        let rschema = test_rschema();
        let edn = parse_edn("[1 :person/name \"Alice\" 536870913]");
        if let EdnValue::Vector(ref fields) = edn {
            let d = datom_from_edn(fields, &rschema);
            assert_eq!(d.e, 1);
            assert_eq!(
                d.a,
                Some(Attr::Keyword { ns: Some("person".into()), name: "name".into() })
            );
            assert_eq!(d.v, Value::Str("Alice".into()));
            assert_eq!(d.tx, 536870913);
        } else {
            panic!("expected vector");
        }
    }

    #[test]
    fn datom_from_edn_ref_value() {
        let rschema = test_rschema();
        let edn = parse_edn("[1 :person/parent 2 536870913]");
        if let EdnValue::Vector(ref fields) = edn {
            let d = datom_from_edn(fields, &rschema);
            assert_eq!(d.v, Value::Ref(2));
        } else {
            panic!("expected vector");
        }
    }

    #[test]
    fn datom_from_edn_non_ref_integer() {
        let rschema = test_rschema();
        let edn = parse_edn("[1 :worksheet/created 1710000000 536870913]");
        if let EdnValue::Vector(ref fields) = edn {
            let d = datom_from_edn(fields, &rschema);
            assert_eq!(d.v, Value::Long(1710000000));
        } else {
            panic!("expected vector");
        }
    }

    // --- datom_to_edn round-trip ---

    #[test]
    fn datom_edn_roundtrip() {
        let d = Datom::new(
            1,
            Some(Attr::Keyword { ns: Some("person".into()), name: "name".into() }),
            Value::Str("Alice".into()),
            536870913,
        );
        let edn_str = datom_to_edn(&d);
        assert_eq!(edn_str, "[1 :person/name \"Alice\" 536870913]");

        let rschema = test_rschema();
        let parsed = parse_edn(&edn_str);
        if let EdnValue::Vector(ref fields) = parsed {
            let d2 = datom_from_edn(fields, &rschema);
            assert_eq!(d, d2);
        }
    }

    // --- node_to_edn / node_from_edn ---

    #[test]
    fn leaf_node_roundtrip() {
        let d1 = Datom::new(1, Some(kw("name")), Value::Str("Alice".into()), 536870913);
        let d2 = Datom::new(2, Some(kw("name")), Value::Str("Bob".into()), 536870913);
        let keys = vec![d1, d2];

        let edn_str = node_to_edn(0, &keys, None);
        assert!(edn_str.contains(":level 0"));
        assert!(edn_str.contains(":keys"));
        assert!(!edn_str.contains(":addresses"));

        let parsed = parse_edn(&edn_str);
        let rschema = build_rschema(&Schema::default());
        let (level, keys2, addrs) = node_from_edn(&parsed, &rschema);
        assert_eq!(level, 0);
        assert_eq!(keys2.len(), 2);
        assert!(addrs.is_none());
    }

    #[test]
    fn branch_node_roundtrip() {
        let d1 = Datom::new(10, Some(kw("name")), Value::Str("X".into()), 536870913);
        let keys = vec![d1];
        let addrs = vec![100i64, 200, 300];

        let edn_str = node_to_edn(2, &keys, Some(&addrs));
        assert!(edn_str.contains(":level 2"));
        assert!(edn_str.contains(":addresses"));

        let parsed = parse_edn(&edn_str);
        let rschema = build_rschema(&Schema::default());
        let (level, keys2, addrs2) = node_from_edn(&parsed, &rschema);
        assert_eq!(level, 2);
        assert_eq!(keys2.len(), 1);
        assert_eq!(addrs2.unwrap(), vec![100, 200, 300]);
    }

    // --- schema_from_edn / schema_to_edn ---

    #[test]
    fn schema_edn_roundtrip() {
        let edn_str = r#"{:name {:db/index true}, :parent {:db/valueType :db.type/ref}, :email {:db/unique :db.unique/identity}, :aka {:db/cardinality :db.cardinality/many}}"#;
        let parsed = parse_edn(edn_str);
        let schema = schema_from_edn(&parsed);

        assert!(schema.attrs[&kw("name")].index);
        assert_eq!(schema.attrs[&kw("parent")].value_type, Some(ValueType::Ref));
        assert_eq!(schema.attrs[&kw("email")].unique, Some(Unique::Identity));
        assert_eq!(schema.attrs[&kw("aka")].cardinality, Cardinality::Many);

        let out = schema_to_edn(&schema);
        let reparsed = parse_edn(&out);
        let schema2 = schema_from_edn(&reparsed);
        assert_eq!(schema2.attrs.len(), schema.attrs.len());
        assert!(schema2.attrs[&kw("name")].index);
    }

    #[test]
    fn schema_from_edn_with_component() {
        let edn_str = r#"{:child {:db/valueType :db.type/ref, :db/isComponent true}}"#;
        let parsed = parse_edn(edn_str);
        let schema = schema_from_edn(&parsed);

        assert!(schema.attrs[&kw("child")].is_component);
        assert_eq!(schema.attrs[&kw("child")].value_type, Some(ValueType::Ref));
    }

    // --- metadata_from_edn / metadata_to_edn ---

    #[test]
    fn metadata_edn_roundtrip() {
        let meta_str = r#"{:schema {:name {:db/index true}}, :eavt 1000, :aevt 2000, :avet 3000, :max-eid 500, :max-tx 536870913, :max-addr 5000, :branching-factor 512}"#;
        let parsed = parse_edn(meta_str);
        let meta = metadata_from_edn(&parsed);

        assert_eq!(meta.eavt_root, 1000);
        assert_eq!(meta.aevt_root, 2000);
        assert_eq!(meta.avet_root, 3000);
        assert_eq!(meta.max_eid, 500);
        assert_eq!(meta.max_tx, 536870913);
        assert_eq!(meta.max_addr, 5000);
        assert_eq!(meta.branching_factor, 512);
        assert!(meta.schema.attrs[&kw("name")].index);

        let out = metadata_to_edn(&meta);
        let reparsed = parse_edn(&out);
        let meta2 = metadata_from_edn(&reparsed);
        assert_eq!(meta2.eavt_root, 1000);
        assert_eq!(meta2.max_eid, 500);
        assert_eq!(meta2.branching_factor, 512);
    }

    // --- Real bp7 metadata ---

    #[test]
    fn parse_real_bp7_metadata() {
        let meta_str = r#"{:schema {:db/ident {:db/unique :db.unique/identity}}, :eavt 1081805, :aevt 1081824, :avet 1081831, :max-eid 1741, :max-tx 536870913, :max-addr 1081831, :branching-factor 512}"#;
        let parsed = parse_edn(meta_str);
        let meta = metadata_from_edn(&parsed);

        assert_eq!(meta.eavt_root, 1081805);
        assert_eq!(meta.aevt_root, 1081824);
        assert_eq!(meta.avet_root, 1081831);
        assert_eq!(meta.max_eid, 1741);
        assert_eq!(meta.max_tx, 536870913);
        assert_eq!(meta.branching_factor, 512);

        let db_ident = Attr::Keyword { ns: Some("db".into()), name: "ident".into() };
        assert!(meta.rschema.unique_identity.contains(&db_ident));
        assert!(meta.rschema.is_indexed(&db_ident));
    }

    // --- Integration with real bp7 data ---

    #[test]
    fn parse_real_bp7_leaf_node() {
        // A simplified real node from behave-test.bp7
        let node_str = r#"{:level 0, :keys [[1 :db/ident :worksheet 536870913] [2 :db/ident :run 536870913]]}"#;
        let parsed = parse_edn(node_str);

        let mut schema = Schema::default();
        schema.attrs.insert(
            Attr::Keyword { ns: Some("db".into()), name: "ident".into() },
            AttrSchema { unique: Some(Unique::Identity), ..Default::default() },
        );
        let rschema = build_rschema(&schema);

        let (level, keys, addrs) = node_from_edn(&parsed, &rschema);
        assert_eq!(level, 0);
        assert_eq!(keys.len(), 2);
        assert!(addrs.is_none());
        assert_eq!(keys[0].e, 1);
        assert_eq!(
            keys[0].a,
            Some(Attr::Keyword { ns: Some("db".into()), name: "ident".into() })
        );
        // :db/ident is in unique_identity, not in ref_attrs, so this is a keyword value
        assert_eq!(
            keys[0].v,
            Value::Keyword(Attr::Keyword { ns: None, name: "worksheet".into() })
        );
    }

    // ===================================================================
    // Edge cases: kw_matches
    // ===================================================================

    #[test]
    fn kw_matches_plain_name() {
        assert!(kw_matches("name", "name"));
        assert!(!kw_matches("name", "age"));
    }

    #[test]
    fn kw_matches_with_colon() {
        assert!(kw_matches(":name", "name"));
        assert!(kw_matches(":db/ident", "ident"));
    }

    #[test]
    fn kw_matches_namespaced() {
        assert!(kw_matches("db/ident", "ident"));
        assert!(kw_matches(":person/name", "name"));
        assert!(!kw_matches(":person/name", "person"));
    }

    #[test]
    fn kw_matches_hyphenated() {
        assert!(kw_matches(":max-eid", "max-eid"));
        assert!(kw_matches("branching-factor", "branching-factor"));
    }

    // ===================================================================
    // Edge cases: edn_get_int
    // ===================================================================

    #[test]
    #[should_panic(expected = "not found")]
    fn edn_get_int_missing_key() {
        let edn = parse_edn("{:a 1}");
        edn_get_int(&edn, "missing");
    }

    #[test]
    #[should_panic(expected = "expected integer")]
    fn edn_get_int_wrong_type() {
        let edn = parse_edn("{:a \"not-an-int\"}");
        edn_get_int(&edn, "a");
    }

    #[test]
    fn edn_get_int_negative() {
        let edn = parse_edn("{:n -42}");
        assert_eq!(edn_get_int(&edn, "n"), -42);
    }

    #[test]
    fn edn_get_int_zero() {
        let edn = parse_edn("{:n 0}");
        assert_eq!(edn_get_int(&edn, "n"), 0);
    }

    // ===================================================================
    // Edge cases: edn_get
    // ===================================================================

    #[test]
    fn edn_get_returns_none_for_missing() {
        let edn = parse_edn("{:a 1}");
        assert!(edn_get(&edn, "b").is_none());
    }

    #[test]
    fn edn_get_returns_none_for_non_map() {
        let edn = parse_edn("[1 2 3]");
        assert!(edn_get(&edn, "a").is_none());
    }

    // ===================================================================
    // Edge cases: attr_from_edn_keyword
    // ===================================================================

    #[test]
    fn attr_from_edn_keyword_deeply_nested_ns() {
        let attr = attr_from_edn_keyword(":a.b.c/name");
        assert_eq!(
            attr,
            Attr::Keyword { ns: Some("a.b.c".into()), name: "name".into() }
        );
    }

    #[test]
    fn attr_from_edn_keyword_single_char() {
        let attr = attr_from_edn_keyword(":x");
        assert_eq!(attr, Attr::Keyword { ns: None, name: "x".into() });
    }

    // ===================================================================
    // Edge cases: attr_to_edn round-trip
    // ===================================================================

    #[test]
    fn attr_to_edn_simple() {
        let attr = Attr::Keyword { ns: None, name: "age".into() };
        assert_eq!(attr_to_edn(&attr), ":age");
    }

    #[test]
    fn attr_to_edn_namespaced() {
        let attr = Attr::Keyword { ns: Some("person".into()), name: "name".into() };
        assert_eq!(attr_to_edn(&attr), ":person/name");
    }

    #[test]
    fn attr_to_edn_str_variant() {
        let attr = Attr::Str("raw-string".into());
        assert_eq!(attr_to_edn(&attr), "\"raw-string\"");
    }

    #[test]
    fn attr_edn_roundtrip_namespaced() {
        let original = Attr::Keyword { ns: Some("my.ns".into()), name: "field".into() };
        let edn_str = attr_to_edn(&original);
        let parsed = attr_from_edn_keyword(&edn_str);
        assert_eq!(original, parsed);
    }

    #[test]
    fn attr_edn_roundtrip_simple() {
        let original = Attr::Keyword { ns: None, name: "field".into() };
        let edn_str = attr_to_edn(&original);
        let parsed = attr_from_edn_keyword(&edn_str);
        assert_eq!(original, parsed);
    }

    // ===================================================================
    // Edge cases: value_to_edn for all types
    // ===================================================================

    #[test]
    fn value_to_edn_nil() {
        assert_eq!(value_to_edn(&Value::Nil), "nil");
    }

    #[test]
    fn value_to_edn_bool_true() {
        assert_eq!(value_to_edn(&Value::Bool(true)), "true");
    }

    #[test]
    fn value_to_edn_bool_false() {
        assert_eq!(value_to_edn(&Value::Bool(false)), "false");
    }

    #[test]
    fn value_to_edn_long() {
        assert_eq!(value_to_edn(&Value::Long(42)), "42");
        assert_eq!(value_to_edn(&Value::Long(-1)), "-1");
        assert_eq!(value_to_edn(&Value::Long(0)), "0");
    }

    #[test]
    fn value_to_edn_double() {
        let s = value_to_edn(&Value::Double(3.14));
        assert!(s.contains("3.14"), "expected 3.14 in '{}'", s);
    }

    #[test]
    fn value_to_edn_double_whole_number() {
        // A double like 42.0 should still have a decimal point
        let s = value_to_edn(&Value::Double(42.0));
        assert!(s.contains('.'), "whole-number double should contain '.': '{}'", s);
    }

    #[test]
    fn value_to_edn_str_simple() {
        assert_eq!(value_to_edn(&Value::Str("hello".into())), "\"hello\"");
    }

    #[test]
    fn value_to_edn_str_with_quotes() {
        let s = value_to_edn(&Value::Str("say \"hi\"".into()));
        assert_eq!(s, "\"say \\\"hi\\\"\"");
    }

    #[test]
    fn value_to_edn_str_with_backslash() {
        let s = value_to_edn(&Value::Str("path\\file".into()));
        assert_eq!(s, "\"path\\\\file\"");
    }

    #[test]
    fn value_to_edn_keyword_simple() {
        let v = Value::Keyword(Attr::Keyword { ns: None, name: "status".into() });
        assert_eq!(value_to_edn(&v), ":status");
    }

    #[test]
    fn value_to_edn_keyword_namespaced() {
        let v = Value::Keyword(Attr::Keyword {
            ns: Some("fuel".into()),
            name: "timber".into(),
        });
        assert_eq!(value_to_edn(&v), ":fuel/timber");
    }

    #[test]
    fn value_to_edn_ref() {
        assert_eq!(value_to_edn(&Value::Ref(42)), "42");
    }

    #[test]
    fn value_to_edn_instant() {
        let s = value_to_edn(&Value::Instant(1710000000000));
        assert!(s.starts_with("#inst"));
    }

    #[test]
    fn value_to_edn_uuid() {
        let bytes = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
            0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10,
        ];
        let s = value_to_edn(&Value::Uuid(bytes));
        assert!(s.starts_with("#uuid"));
        assert!(s.contains("01020304-0506-0708-090a-0b0c0d0e0f10"));
    }

    #[test]
    fn value_to_edn_bytes() {
        let s = value_to_edn(&Value::Bytes(vec![0xFF, 0x00]));
        // Bytes format is debug-like, not parseable EDN
        assert!(s.contains("255") || s.contains("0xff") || s.contains("FF"));
    }

    // ===================================================================
    // value_from_edn → value_to_edn round-trip for parseable types
    // ===================================================================

    #[test]
    fn value_roundtrip_long() {
        let original = Value::Long(12345);
        let edn_str = value_to_edn(&original);
        let parsed = parse_edn(&edn_str);
        let result = value_from_edn(&parsed, false);
        assert_eq!(result, original);
    }

    #[test]
    fn value_roundtrip_negative_long() {
        let original = Value::Long(-999);
        let edn_str = value_to_edn(&original);
        let parsed = parse_edn(&edn_str);
        let result = value_from_edn(&parsed, false);
        assert_eq!(result, original);
    }

    #[test]
    fn value_roundtrip_string() {
        let original = Value::Str("hello world".into());
        let edn_str = value_to_edn(&original);
        let parsed = parse_edn(&edn_str);
        let result = value_from_edn(&parsed, false);
        assert_eq!(result, original);
    }

    #[test]
    fn value_roundtrip_bool() {
        for b in [true, false] {
            let original = Value::Bool(b);
            let edn_str = value_to_edn(&original);
            let parsed = parse_edn(&edn_str);
            let result = value_from_edn(&parsed, false);
            assert_eq!(result, original);
        }
    }

    #[test]
    fn value_roundtrip_keyword() {
        let original = Value::Keyword(Attr::Keyword {
            ns: Some("ns".into()),
            name: "kw".into(),
        });
        let edn_str = value_to_edn(&original);
        let parsed = parse_edn(&edn_str);
        let result = value_from_edn(&parsed, false);
        assert_eq!(result, original);
    }

    #[test]
    fn value_roundtrip_double() {
        let original = Value::Double(2.718);
        let edn_str = value_to_edn(&original);
        let parsed = parse_edn(&edn_str);
        let result = value_from_edn(&parsed, false);
        match result {
            Value::Double(f) => assert!((f - 2.718).abs() < 1e-10),
            _ => panic!("expected Double, got {:?}", result),
        }
    }

    // ===================================================================
    // Edge cases: datom_from_edn / datom_to_edn
    // ===================================================================

    #[test]
    fn datom_with_boolean_value() {
        let rschema = build_rschema(&Schema::default());
        let edn = parse_edn("[1 :active true 536870913]");
        if let EdnValue::Vector(ref fields) = edn {
            let d = datom_from_edn(fields, &rschema);
            assert_eq!(d.v, Value::Bool(true));
            // Round-trip
            let out = datom_to_edn(&d);
            assert!(out.contains("true"));
        }
    }

    #[test]
    fn datom_with_keyword_value() {
        let rschema = build_rschema(&Schema::default());
        let edn = parse_edn("[1 :type :worksheet 536870913]");
        if let EdnValue::Vector(ref fields) = edn {
            let d = datom_from_edn(fields, &rschema);
            assert_eq!(
                d.v,
                Value::Keyword(Attr::Keyword { ns: None, name: "worksheet".into() })
            );
        }
    }

    #[test]
    fn datom_with_float_value() {
        let rschema = build_rschema(&Schema::default());
        let edn = parse_edn("[1 :weight 72.5 536870913]");
        if let EdnValue::Vector(ref fields) = edn {
            let d = datom_from_edn(fields, &rschema);
            match d.v {
                Value::Double(f) => assert!((f - 72.5).abs() < 1e-10),
                _ => panic!("expected Double"),
            }
        }
    }

    #[test]
    fn datom_with_nil_value() {
        let rschema = build_rschema(&Schema::default());
        let edn = parse_edn("[1 :field nil 536870913]");
        if let EdnValue::Vector(ref fields) = edn {
            let d = datom_from_edn(fields, &rschema);
            assert_eq!(d.v, Value::Nil);
        }
    }

    #[test]
    fn datom_with_namespaced_keyword_value() {
        let rschema = build_rschema(&Schema::default());
        let edn = parse_edn("[1 :fuel/type :fuel/timber 536870913]");
        if let EdnValue::Vector(ref fields) = edn {
            let d = datom_from_edn(fields, &rschema);
            assert_eq!(
                d.v,
                Value::Keyword(Attr::Keyword {
                    ns: Some("fuel".into()),
                    name: "timber".into(),
                })
            );
        }
    }

    #[test]
    fn datom_to_edn_with_ref_value() {
        let d = Datom::new(1, Some(kw("parent")), Value::Ref(42), 536870913);
        let edn_str = datom_to_edn(&d);
        // Ref(42) serializes as just "42" — indistinguishable from Long without schema
        assert_eq!(edn_str, "[1 :parent 42 536870913]");
    }

    #[test]
    fn datom_to_edn_with_nil_attr() {
        let d = Datom::new(1, None, Value::Long(1), 100);
        let edn_str = datom_to_edn(&d);
        assert_eq!(edn_str, "[1 nil 1 100]");
    }

    #[test]
    #[should_panic(expected = "datom must have 4 fields")]
    fn datom_from_edn_wrong_field_count() {
        let rschema = build_rschema(&Schema::default());
        let edn = parse_edn("[1 :name]"); // only 2 fields
        if let EdnValue::Vector(ref fields) = edn {
            datom_from_edn(fields, &rschema);
        }
    }

    #[test]
    #[should_panic(expected = "datom entity must be integer")]
    fn datom_from_edn_non_integer_entity() {
        let rschema = build_rschema(&Schema::default());
        let edn = parse_edn("[\"bad\" :name \"x\" 1]");
        if let EdnValue::Vector(ref fields) = edn {
            datom_from_edn(fields, &rschema);
        }
    }

    // ===================================================================
    // Edge cases: node_to_edn / node_from_edn
    // ===================================================================

    #[test]
    fn empty_leaf_node() {
        let edn_str = node_to_edn(0, &[], None);
        let parsed = parse_edn(&edn_str);
        let rschema = build_rschema(&Schema::default());
        let (level, keys, addrs) = node_from_edn(&parsed, &rschema);
        assert_eq!(level, 0);
        assert!(keys.is_empty());
        assert!(addrs.is_none());
    }

    #[test]
    fn node_with_many_datoms() {
        let datoms: Vec<Datom> = (1..=50)
            .map(|i| Datom::new(i, Some(kw("name")), Value::Str(format!("e{}", i)), 536870913))
            .collect();
        let edn_str = node_to_edn(0, &datoms, None);
        let parsed = parse_edn(&edn_str);
        let rschema = build_rschema(&Schema::default());
        let (_, keys, _) = node_from_edn(&parsed, &rschema);
        assert_eq!(keys.len(), 50);
        assert_eq!(keys[0].e, 1);
        assert_eq!(keys[49].e, 50);
    }

    #[test]
    fn branch_node_with_single_child() {
        let d = Datom::new(5, Some(kw("x")), Value::Long(1), 100);
        let edn_str = node_to_edn(1, &[d], Some(&[42]));
        let parsed = parse_edn(&edn_str);
        let rschema = build_rschema(&Schema::default());
        let (level, keys, addrs) = node_from_edn(&parsed, &rschema);
        assert_eq!(level, 1);
        assert_eq!(keys.len(), 1);
        assert_eq!(addrs.unwrap(), vec![42]);
    }

    // ===================================================================
    // Edge cases: schema_from_edn / schema_to_edn
    // ===================================================================

    #[test]
    fn schema_empty_roundtrip() {
        let edn_str = "{}";
        let parsed = parse_edn(edn_str);
        let schema = schema_from_edn(&parsed);
        assert!(schema.attrs.is_empty());

        let out = schema_to_edn(&schema);
        assert_eq!(out, "{}");
    }

    #[test]
    fn schema_unique_value_roundtrip() {
        let edn_str = r#"{:code {:db/unique :db.unique/value}}"#;
        let parsed = parse_edn(edn_str);
        let schema = schema_from_edn(&parsed);
        assert_eq!(schema.attrs[&kw("code")].unique, Some(Unique::Value));

        let out = schema_to_edn(&schema);
        let reparsed = parse_edn(&out);
        let schema2 = schema_from_edn(&reparsed);
        assert_eq!(schema2.attrs[&kw("code")].unique, Some(Unique::Value));
    }

    #[test]
    fn schema_all_properties_combined() {
        let edn_str = r#"{:child {:db/valueType :db.type/ref, :db/isComponent true, :db/cardinality :db.cardinality/many, :db/index true}}"#;
        let parsed = parse_edn(edn_str);
        let schema = schema_from_edn(&parsed);
        let a = &schema.attrs[&kw("child")];
        assert!(a.index);
        assert!(a.is_component);
        assert_eq!(a.value_type, Some(ValueType::Ref));
        assert_eq!(a.cardinality, Cardinality::Many);
    }

    #[test]
    fn schema_from_edn_ignores_unknown_properties() {
        let edn_str = r#"{:name {:db/index true, :db/doc "A name field", :db/fulltext true}}"#;
        let parsed = parse_edn(edn_str);
        let schema = schema_from_edn(&parsed);
        // Should parse :db/index, silently ignore :db/doc and :db/fulltext
        assert!(schema.attrs[&kw("name")].index);
    }

    #[test]
    fn schema_with_namespaced_attrs() {
        let edn_str = r#"{:person/name {:db/index true}, :person/age {}}"#;
        let parsed = parse_edn(edn_str);
        let schema = schema_from_edn(&parsed);
        let ns_attr = Attr::Keyword {
            ns: Some("person".into()),
            name: "name".into(),
        };
        assert!(schema.attrs[&ns_attr].index);
        assert_eq!(schema.attrs.len(), 2);
    }

    #[test]
    fn schema_from_edn_non_map_value_skipped() {
        // If an attr maps to a non-map value, it should be skipped
        let edn_str = r#"{:name true}"#;
        let parsed = parse_edn(edn_str);
        let schema = schema_from_edn(&parsed);
        assert!(schema.attrs.is_empty());
    }

    // ===================================================================
    // Edge cases: metadata
    // ===================================================================

    #[test]
    fn metadata_preserves_all_fields() {
        let mut schema = Schema::default();
        schema.attrs.insert(kw("name"), AttrSchema { index: true, ..Default::default() });
        schema.attrs.insert(kw("parent"), AttrSchema {
            value_type: Some(ValueType::Ref),
            is_component: true,
            ..Default::default()
        });
        let rschema = build_rschema(&schema);

        let meta = LegacyMetadata {
            schema: schema.clone(),
            rschema,
            eavt_root: 999,
            aevt_root: 888,
            avet_root: 777,
            max_eid: 100,
            max_tx: 536870913,
            max_addr: 1500,
            branching_factor: 256,
        };

        let edn_str = metadata_to_edn(&meta);
        let parsed = parse_edn(&edn_str);
        let meta2 = metadata_from_edn(&parsed);

        assert_eq!(meta2.eavt_root, 999);
        assert_eq!(meta2.aevt_root, 888);
        assert_eq!(meta2.avet_root, 777);
        assert_eq!(meta2.max_eid, 100);
        assert_eq!(meta2.max_tx, 536870913);
        assert_eq!(meta2.max_addr, 1500);
        assert_eq!(meta2.branching_factor, 256);
        assert!(meta2.schema.attrs[&kw("name")].index);
        assert!(meta2.schema.attrs[&kw("parent")].is_component);
        assert_eq!(meta2.schema.attrs[&kw("parent")].value_type, Some(ValueType::Ref));
        // rschema is rebuilt from schema
        assert!(meta2.rschema.is_indexed(&kw("name")));
        assert!(meta2.rschema.is_ref(&kw("parent")));
        assert!(meta2.rschema.is_component(&kw("parent")));
    }

    // ===================================================================
    // Ref vs Long disambiguation
    // ===================================================================

    #[test]
    fn ref_disambiguation_same_integer_different_schema() {
        // Same integer value 42 should become Ref or Long depending on schema
        let mut schema = Schema::default();
        schema.attrs.insert(kw("parent"), AttrSchema {
            value_type: Some(ValueType::Ref),
            ..Default::default()
        });
        schema.attrs.insert(kw("age"), AttrSchema::default());
        let rschema = build_rschema(&schema);

        let ref_edn = parse_edn("[1 :parent 42 100]");
        let long_edn = parse_edn("[1 :age 42 100]");

        if let (EdnValue::Vector(ref r), EdnValue::Vector(ref l)) = (&ref_edn, &long_edn) {
            let ref_datom = datom_from_edn(r, &rschema);
            let long_datom = datom_from_edn(l, &rschema);
            assert_eq!(ref_datom.v, Value::Ref(42));
            assert_eq!(long_datom.v, Value::Long(42));
        }
    }

    // ===================================================================
    // Mixed-type node: datoms with diverse value types
    // ===================================================================

    #[test]
    fn node_with_mixed_value_types() {
        let mut schema = Schema::default();
        schema.attrs.insert(kw("parent"), AttrSchema {
            value_type: Some(ValueType::Ref),
            ..Default::default()
        });
        let rschema = build_rschema(&schema);

        let datoms = vec![
            Datom::new(1, Some(kw("name")), Value::Str("Alice".into()), 100),
            Datom::new(1, Some(kw("age")), Value::Long(30), 100),
            Datom::new(1, Some(kw("active")), Value::Bool(true), 100),
            Datom::new(1, Some(kw("parent")), Value::Ref(2), 100),
            Datom::new(1, Some(kw("type")), Value::Keyword(kw("person")), 100),
        ];

        let edn_str = node_to_edn(0, &datoms, None);
        let parsed = parse_edn(&edn_str);
        let (_, keys, _) = node_from_edn(&parsed, &rschema);

        assert_eq!(keys.len(), 5);
        assert_eq!(keys[0].v, Value::Str("Alice".into()));
        assert_eq!(keys[1].v, Value::Long(30));
        assert_eq!(keys[2].v, Value::Bool(true));
        // parent is a ref attr — integer 2 should be Ref(2)
        assert_eq!(keys[3].v, Value::Ref(2));
        assert_eq!(
            keys[4].v,
            Value::Keyword(Attr::Keyword { ns: None, name: "person".into() })
        );
    }
}
