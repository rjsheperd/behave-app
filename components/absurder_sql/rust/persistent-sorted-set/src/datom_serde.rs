//! Binary serialization for Datom types.
//!
//! Shared by both native (rusqlite) and WASM (sqlite-wasm-rs) targets.
//! Format: [e:i64_le][a_type:u8][a_data..][v_type:u8][v_data..][tx:i64_le]

use crate::datom::{Attr, Datom, Value};

// --- Attribute type tags ---
const ATTR_NONE: u8 = 0;
const ATTR_KEYWORD_NO_NS: u8 = 1;
const ATTR_KEYWORD_WITH_NS: u8 = 2;
const ATTR_STR: u8 = 3;

// --- Value type tags ---
const VAL_NIL: u8 = 0;
const VAL_BOOL: u8 = 1;
const VAL_LONG: u8 = 2;
const VAL_DOUBLE: u8 = 3;
const VAL_STR: u8 = 4;
const VAL_KEYWORD: u8 = 5;
const VAL_REF: u8 = 6;
const VAL_INSTANT: u8 = 7;
const VAL_UUID: u8 = 8;
const VAL_BYTES: u8 = 9;

fn write_str(buf: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    buf.extend_from_slice(bytes);
}

fn read_str(data: &[u8], pos: &mut usize) -> String {
    let len = u32::from_le_bytes(data[*pos..*pos + 4].try_into().unwrap()) as usize;
    *pos += 4;
    let s = std::str::from_utf8(&data[*pos..*pos + len]).unwrap().to_string();
    *pos += len;
    s
}

fn serialize_attr(buf: &mut Vec<u8>, attr: &Option<Attr>) {
    match attr {
        None => buf.push(ATTR_NONE),
        Some(Attr::Keyword { ns: None, name }) => {
            buf.push(ATTR_KEYWORD_NO_NS);
            write_str(buf, name);
        }
        Some(Attr::Keyword { ns: Some(ns), name }) => {
            buf.push(ATTR_KEYWORD_WITH_NS);
            write_str(buf, ns);
            write_str(buf, name);
        }
        Some(Attr::Str(s)) => {
            buf.push(ATTR_STR);
            write_str(buf, s);
        }
    }
}

fn deserialize_attr(data: &[u8], pos: &mut usize) -> Option<Attr> {
    let tag = data[*pos];
    *pos += 1;
    match tag {
        ATTR_NONE => None,
        ATTR_KEYWORD_NO_NS => {
            let name = read_str(data, pos);
            Some(Attr::Keyword { ns: None, name })
        }
        ATTR_KEYWORD_WITH_NS => {
            let ns = read_str(data, pos);
            let name = read_str(data, pos);
            Some(Attr::Keyword { ns: Some(ns), name })
        }
        ATTR_STR => {
            let s = read_str(data, pos);
            Some(Attr::Str(s))
        }
        _ => panic!("unknown attr tag: {}", tag),
    }
}

fn serialize_value(buf: &mut Vec<u8>, v: &Value) {
    match v {
        Value::Nil => buf.push(VAL_NIL),
        Value::Bool(b) => {
            buf.push(VAL_BOOL);
            buf.push(if *b { 1 } else { 0 });
        }
        Value::Long(n) => {
            buf.push(VAL_LONG);
            buf.extend_from_slice(&n.to_le_bytes());
        }
        Value::Double(f) => {
            buf.push(VAL_DOUBLE);
            buf.extend_from_slice(&f.to_le_bytes());
        }
        Value::Str(s) => {
            buf.push(VAL_STR);
            write_str(buf, s);
        }
        Value::Keyword(attr) => {
            buf.push(VAL_KEYWORD);
            // Reuse attr serialization but always Some
            serialize_attr(buf, &Some(attr.clone()));
        }
        Value::Ref(id) => {
            buf.push(VAL_REF);
            buf.extend_from_slice(&id.to_le_bytes());
        }
        Value::Instant(ms) => {
            buf.push(VAL_INSTANT);
            buf.extend_from_slice(&ms.to_le_bytes());
        }
        Value::Uuid(bytes) => {
            buf.push(VAL_UUID);
            buf.extend_from_slice(bytes);
        }
        Value::Bytes(bytes) => {
            buf.push(VAL_BYTES);
            buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            buf.extend_from_slice(bytes);
        }
    }
}

fn deserialize_value(data: &[u8], pos: &mut usize) -> Value {
    let tag = data[*pos];
    *pos += 1;
    match tag {
        VAL_NIL => Value::Nil,
        VAL_BOOL => {
            let b = data[*pos] != 0;
            *pos += 1;
            Value::Bool(b)
        }
        VAL_LONG => {
            let n = i64::from_le_bytes(data[*pos..*pos + 8].try_into().unwrap());
            *pos += 8;
            Value::Long(n)
        }
        VAL_DOUBLE => {
            let f = f64::from_le_bytes(data[*pos..*pos + 8].try_into().unwrap());
            *pos += 8;
            Value::Double(f)
        }
        VAL_STR => {
            let s = read_str(data, pos);
            Value::Str(s)
        }
        VAL_KEYWORD => {
            let attr = deserialize_attr(data, pos)
                .expect("keyword value must have Some attr");
            Value::Keyword(attr)
        }
        VAL_REF => {
            let id = i64::from_le_bytes(data[*pos..*pos + 8].try_into().unwrap());
            *pos += 8;
            Value::Ref(id)
        }
        VAL_INSTANT => {
            let ms = i64::from_le_bytes(data[*pos..*pos + 8].try_into().unwrap());
            *pos += 8;
            Value::Instant(ms)
        }
        VAL_UUID => {
            let bytes: [u8; 16] = data[*pos..*pos + 16].try_into().unwrap();
            *pos += 16;
            Value::Uuid(bytes)
        }
        VAL_BYTES => {
            let len = u32::from_le_bytes(data[*pos..*pos + 4].try_into().unwrap()) as usize;
            *pos += 4;
            let bytes = data[*pos..*pos + len].to_vec();
            *pos += len;
            Value::Bytes(bytes)
        }
        _ => panic!("unknown value tag: {}", tag),
    }
}

/// Serialize a single datom to binary.
pub fn serialize_datom(d: &Datom) -> Vec<u8> {
    let mut buf = Vec::with_capacity(64);
    buf.extend_from_slice(&d.e.to_le_bytes());
    serialize_attr(&mut buf, &d.a);
    serialize_value(&mut buf, &d.v);
    buf.extend_from_slice(&d.tx.to_le_bytes());
    buf
}

/// Deserialize a single datom from binary.
pub fn deserialize_datom(data: &[u8]) -> Datom {
    let mut pos = 0;
    let e = i64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
    pos += 8;
    let a = deserialize_attr(data, &mut pos);
    let v = deserialize_value(data, &mut pos);
    let tx = i64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
    Datom::new(e, a, v, tx)
}

/// Serialize a slice of datoms with length prefixes.
pub fn serialize_keys(datoms: &[Datom]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(datoms.len() * 64);
    for d in datoms {
        let datom_bytes = serialize_datom(d);
        buf.extend_from_slice(&(datom_bytes.len() as u32).to_le_bytes());
        buf.extend_from_slice(&datom_bytes);
    }
    buf
}

/// Deserialize length-prefixed concatenated datoms.
pub fn deserialize_keys(blob: &[u8]) -> Vec<Datom> {
    let mut result = Vec::new();
    let mut pos = 0;
    while pos < blob.len() {
        let len = u32::from_le_bytes(blob[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;
        let datom = deserialize_datom(&blob[pos..pos + len]);
        pos += len;
        result.push(datom);
    }
    result
}

/// Serialize addresses as packed i64 little-endian.
pub fn serialize_addrs(addrs: &[i64]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(addrs.len() * 8);
    for &a in addrs {
        buf.extend_from_slice(&a.to_le_bytes());
    }
    buf
}

/// Deserialize packed i64 little-endian addresses.
pub fn deserialize_addrs(blob: &[u8]) -> Vec<i64> {
    blob.chunks_exact(8)
        .map(|chunk| i64::from_le_bytes(chunk.try_into().unwrap()))
        .collect()
}

// ---------------------------------------------------------------------------
// Schema serialization
// ---------------------------------------------------------------------------

use crate::schema::{AttrSchema, Cardinality, Schema, Unique, ValueType};

/// Bit flags for AttrSchema properties.
const FLAG_INDEX: u8 = 1;
const FLAG_UNIQUE_IDENTITY: u8 = 2;
const FLAG_UNIQUE_VALUE: u8 = 4;
const FLAG_CARDINALITY_MANY: u8 = 8;
const FLAG_VALUE_TYPE_REF: u8 = 16;
const FLAG_IS_COMPONENT: u8 = 32;
const FLAG_HAS_TUPLE_ATTRS: u8 = 64;

fn serialize_attr_schema(buf: &mut Vec<u8>, s: &AttrSchema) {
    let mut flags: u8 = 0;
    if s.index { flags |= FLAG_INDEX; }
    match &s.unique {
        Some(Unique::Identity) => flags |= FLAG_UNIQUE_IDENTITY,
        Some(Unique::Value) => flags |= FLAG_UNIQUE_VALUE,
        None => {}
    }
    if s.cardinality == Cardinality::Many { flags |= FLAG_CARDINALITY_MANY; }
    if s.value_type == Some(ValueType::Ref) { flags |= FLAG_VALUE_TYPE_REF; }
    if s.is_component { flags |= FLAG_IS_COMPONENT; }
    if s.tuple_attrs.is_some() { flags |= FLAG_HAS_TUPLE_ATTRS; }
    buf.push(flags);

    if let Some(ref tuple_attrs) = s.tuple_attrs {
        buf.extend_from_slice(&(tuple_attrs.len() as u32).to_le_bytes());
        for attr in tuple_attrs {
            serialize_attr(buf, &Some(attr.clone()));
        }
    }
}

fn deserialize_attr_schema(data: &[u8], pos: &mut usize) -> AttrSchema {
    let flags = data[*pos];
    *pos += 1;

    let unique = if flags & FLAG_UNIQUE_IDENTITY != 0 {
        Some(Unique::Identity)
    } else if flags & FLAG_UNIQUE_VALUE != 0 {
        Some(Unique::Value)
    } else {
        None
    };

    let tuple_attrs = if flags & FLAG_HAS_TUPLE_ATTRS != 0 {
        let len = u32::from_le_bytes(data[*pos..*pos + 4].try_into().unwrap()) as usize;
        *pos += 4;
        let mut attrs = Vec::with_capacity(len);
        for _ in 0..len {
            attrs.push(deserialize_attr(data, pos).expect("tuple attr must be Some"));
        }
        Some(attrs)
    } else {
        None
    };

    AttrSchema {
        index: flags & FLAG_INDEX != 0,
        unique,
        cardinality: if flags & FLAG_CARDINALITY_MANY != 0 { Cardinality::Many } else { Cardinality::One },
        value_type: if flags & FLAG_VALUE_TYPE_REF != 0 { Some(ValueType::Ref) } else { None },
        is_component: flags & FLAG_IS_COMPONENT != 0,
        tuple_attrs,
    }
}

/// Serialize a Schema (map of Attr → AttrSchema).
pub fn serialize_schema(schema: &Schema) -> Vec<u8> {
    let mut buf = Vec::with_capacity(256);
    buf.extend_from_slice(&(schema.attrs.len() as u32).to_le_bytes());
    for (attr, attr_schema) in &schema.attrs {
        serialize_attr(&mut buf, &Some(attr.clone()));
        serialize_attr_schema(&mut buf, attr_schema);
    }
    buf
}

/// Deserialize a Schema.
pub fn deserialize_schema(data: &[u8]) -> Schema {
    let mut pos = 0;
    let len = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4;
    let mut schema = Schema::default();
    for _ in 0..len {
        let attr = deserialize_attr(data, &mut pos).expect("schema attr must be Some");
        let attr_schema = deserialize_attr_schema(data, &mut pos);
        schema.attrs.insert(attr, attr_schema);
    }
    schema
}

// ---------------------------------------------------------------------------
// DB metadata serialization (stored at addr=0 in pss_nodes)
// ---------------------------------------------------------------------------

/// Metadata for a stored DataScriptDB.
#[derive(Clone, Debug)]
pub struct DbMetadata {
    pub schema_blob: Vec<u8>,
    pub max_eid: i64,
    pub max_tx: i64,
    pub eavt_root: i64,
    pub aevt_root: i64,
    pub avet_root: i64,
}

/// Serialize DB metadata to binary.
pub fn serialize_metadata(meta: &DbMetadata) -> Vec<u8> {
    let mut buf = Vec::with_capacity(meta.schema_blob.len() + 48);
    // Schema blob with length prefix
    buf.extend_from_slice(&(meta.schema_blob.len() as u32).to_le_bytes());
    buf.extend_from_slice(&meta.schema_blob);
    // Fixed fields
    buf.extend_from_slice(&meta.max_eid.to_le_bytes());
    buf.extend_from_slice(&meta.max_tx.to_le_bytes());
    buf.extend_from_slice(&meta.eavt_root.to_le_bytes());
    buf.extend_from_slice(&meta.aevt_root.to_le_bytes());
    buf.extend_from_slice(&meta.avet_root.to_le_bytes());
    buf
}

/// Deserialize DB metadata from binary.
pub fn deserialize_metadata(data: &[u8]) -> DbMetadata {
    let mut pos = 0;
    let schema_len = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4;
    let schema_blob = data[pos..pos + schema_len].to_vec();
    pos += schema_len;
    let max_eid = i64::from_le_bytes(data[pos..pos + 8].try_into().unwrap()); pos += 8;
    let max_tx = i64::from_le_bytes(data[pos..pos + 8].try_into().unwrap()); pos += 8;
    let eavt_root = i64::from_le_bytes(data[pos..pos + 8].try_into().unwrap()); pos += 8;
    let aevt_root = i64::from_le_bytes(data[pos..pos + 8].try_into().unwrap()); pos += 8;
    let avet_root = i64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
    DbMetadata { schema_blob, max_eid, max_tx, eavt_root, aevt_root, avet_root }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip_datom(d: &Datom) -> Datom {
        let bytes = serialize_datom(d);
        deserialize_datom(&bytes)
    }

    #[test]
    fn roundtrip_nil_value() {
        let d = Datom::new(1, None, Value::Nil, 100);
        let d2 = roundtrip_datom(&d);
        assert_eq!(d.e, d2.e);
        assert_eq!(d.a, d2.a);
        assert_eq!(d.v, d2.v);
        assert_eq!(d.tx, d2.tx);
    }

    #[test]
    fn roundtrip_bool() {
        let d = Datom::new(1, Some(Attr::Keyword { ns: None, name: "active".into() }), Value::Bool(true), 100);
        let d2 = roundtrip_datom(&d);
        assert_eq!(d, d2);
    }

    #[test]
    fn roundtrip_long() {
        let d = Datom::new(42, Some(Attr::Keyword { ns: Some("person".into()), name: "age".into() }), Value::Long(30), 200);
        let d2 = roundtrip_datom(&d);
        assert_eq!(d, d2);
    }

    #[test]
    fn roundtrip_double() {
        let d = Datom::new(1, None, Value::Double(3.14159), 1);
        let d2 = roundtrip_datom(&d);
        assert_eq!(d.e, d2.e);
        match d2.v {
            Value::Double(f) => assert!((f - 3.14159).abs() < 1e-10),
            _ => panic!("expected Double"),
        }
    }

    #[test]
    fn roundtrip_str_value() {
        let d = Datom::new(1, Some(Attr::Str("name".into())), Value::Str("Ivan".into()), 100);
        let d2 = roundtrip_datom(&d);
        assert_eq!(d, d2);
    }

    #[test]
    fn roundtrip_keyword_value() {
        let kw = Attr::Keyword { ns: Some("db".into()), name: "ident".into() };
        let d = Datom::new(1, None, Value::Keyword(kw), 1);
        let d2 = roundtrip_datom(&d);
        assert_eq!(d, d2);
    }

    #[test]
    fn roundtrip_ref() {
        let d = Datom::new(1, Some(Attr::Keyword { ns: None, name: "parent".into() }), Value::Ref(42), 100);
        let d2 = roundtrip_datom(&d);
        assert_eq!(d, d2);
    }

    #[test]
    fn roundtrip_instant() {
        let d = Datom::new(1, None, Value::Instant(1710000000000), 1);
        let d2 = roundtrip_datom(&d);
        assert_eq!(d, d2);
    }

    #[test]
    fn roundtrip_uuid() {
        let uuid = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let d = Datom::new(1, None, Value::Uuid(uuid), 1);
        let d2 = roundtrip_datom(&d);
        assert_eq!(d, d2);
    }

    #[test]
    fn roundtrip_bytes() {
        let d = Datom::new(1, None, Value::Bytes(vec![0xFF, 0x00, 0xAB]), 1);
        let d2 = roundtrip_datom(&d);
        assert_eq!(d, d2);
    }

    #[test]
    fn roundtrip_negative_tx() {
        let d = Datom::new(1, Some(Attr::Keyword { ns: None, name: "x".into() }), Value::Long(1), -100);
        let d2 = roundtrip_datom(&d);
        assert_eq!(d, d2);
    }

    #[test]
    fn roundtrip_keys_multiple() {
        let datoms = vec![
            Datom::new(1, Some(Attr::Keyword { ns: Some("person".into()), name: "name".into() }), Value::Str("Alice".into()), 100),
            Datom::new(2, Some(Attr::Keyword { ns: None, name: "age".into() }), Value::Long(30), 100),
            Datom::new(3, None, Value::Nil, 200),
        ];
        let blob = serialize_keys(&datoms);
        let datoms2 = deserialize_keys(&blob);
        assert_eq!(datoms.len(), datoms2.len());
        for (a, b) in datoms.iter().zip(datoms2.iter()) {
            assert_eq!(a, b);
        }
    }

    #[test]
    fn roundtrip_addrs() {
        let addrs = vec![1i64, 42, 100, -5];
        let blob = serialize_addrs(&addrs);
        let addrs2 = deserialize_addrs(&blob);
        assert_eq!(addrs, addrs2);
    }

    #[test]
    fn empty_keys_roundtrip() {
        let blob = serialize_keys(&[]);
        let result = deserialize_keys(&blob);
        assert!(result.is_empty());
    }

    #[test]
    fn empty_addrs_roundtrip() {
        let blob = serialize_addrs(&[]);
        let result = deserialize_addrs(&blob);
        assert!(result.is_empty());
    }

    // --- Schema serialization ---

    use crate::schema::{AttrSchema, Cardinality, Schema, Unique, ValueType, kw, kw_ns};

    #[test]
    fn schema_serialize_empty() {
        let schema = Schema::default();
        let blob = serialize_schema(&schema);
        let schema2 = deserialize_schema(&blob);
        assert_eq!(schema2.attrs.len(), 0);
    }

    #[test]
    fn schema_serialize_all_properties() {
        let mut schema = Schema::default();
        schema.attrs.insert(kw("name"), AttrSchema { index: true, ..Default::default() });
        schema.attrs.insert(kw("email"), AttrSchema {
            unique: Some(Unique::Identity),
            ..Default::default()
        });
        schema.attrs.insert(kw("code"), AttrSchema {
            unique: Some(Unique::Value),
            ..Default::default()
        });
        schema.attrs.insert(kw("aka"), AttrSchema {
            cardinality: Cardinality::Many,
            ..Default::default()
        });
        schema.attrs.insert(kw("parent"), AttrSchema {
            value_type: Some(ValueType::Ref),
            is_component: true,
            ..Default::default()
        });

        let blob = serialize_schema(&schema);
        let schema2 = deserialize_schema(&blob);

        assert_eq!(schema2.attrs.len(), 5);
        assert!(schema2.attrs[&kw("name")].index);
        assert_eq!(schema2.attrs[&kw("email")].unique, Some(Unique::Identity));
        assert_eq!(schema2.attrs[&kw("code")].unique, Some(Unique::Value));
        assert_eq!(schema2.attrs[&kw("aka")].cardinality, Cardinality::Many);
        assert_eq!(schema2.attrs[&kw("parent")].value_type, Some(ValueType::Ref));
        assert!(schema2.attrs[&kw("parent")].is_component);
    }

    #[test]
    fn schema_serialize_namespaced_keywords() {
        let mut schema = Schema::default();
        schema.attrs.insert(kw_ns("person", "name"), AttrSchema { index: true, ..Default::default() });
        schema.attrs.insert(kw_ns("db", "ident"), AttrSchema {
            unique: Some(Unique::Identity),
            ..Default::default()
        });

        let blob = serialize_schema(&schema);
        let schema2 = deserialize_schema(&blob);

        assert_eq!(schema2.attrs.len(), 2);
        assert!(schema2.attrs.contains_key(&kw_ns("person", "name")));
        assert!(schema2.attrs.contains_key(&kw_ns("db", "ident")));
    }

    // --- Metadata serialization ---

    #[test]
    fn metadata_serialize_roundtrip() {
        let mut schema = Schema::default();
        schema.attrs.insert(kw("name"), AttrSchema { index: true, ..Default::default() });
        let schema_blob = serialize_schema(&schema);

        let meta = DbMetadata {
            schema_blob,
            max_eid: 42,
            max_tx: 536870912 + 100,
            eavt_root: 1001,
            aevt_root: 1002,
            avet_root: 1003,
        };

        let blob = serialize_metadata(&meta);
        let meta2 = deserialize_metadata(&blob);

        assert_eq!(meta2.max_eid, 42);
        assert_eq!(meta2.max_tx, 536870912 + 100);
        assert_eq!(meta2.eavt_root, 1001);
        assert_eq!(meta2.aevt_root, 1002);
        assert_eq!(meta2.avet_root, 1003);

        // Schema round-trips through the metadata blob
        let schema2 = deserialize_schema(&meta2.schema_blob);
        assert_eq!(schema2.attrs.len(), 1);
        assert!(schema2.attrs[&kw("name")].index);
    }
}
