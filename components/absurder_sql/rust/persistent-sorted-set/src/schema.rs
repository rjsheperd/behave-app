//! Schema and ReverseSchema types for DataScript.
//!
//! Mirrors the CLJS `db.cljc` schema system:
//! - `Schema`: maps attribute → properties (`:db/index`, `:db/valueType`, etc.)
//! - `ReverseSchema`: maps property → set of attributes (for fast lookups)

use std::collections::{HashMap, HashSet};
use crate::datom::Attr;

/// Schema properties for a single attribute.
#[derive(Clone, Debug, Default)]
pub struct AttrSchema {
    pub index: bool,
    pub unique: Option<Unique>,
    pub cardinality: Cardinality,
    pub value_type: Option<ValueType>,
    pub is_component: bool,
    pub tuple_attrs: Option<Vec<Attr>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unique {
    Identity,
    Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cardinality {
    One,
    Many,
}

impl Default for Cardinality {
    fn default() -> Self {
        Cardinality::One
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValueType {
    Ref,
}

/// Full schema: attribute → properties.
#[derive(Clone, Debug, Default)]
pub struct Schema {
    pub attrs: HashMap<Attr, AttrSchema>,
}

/// Reverse schema: property → set of attributes.
/// Mirrors the CLJS `rschema` structure from `db.cljc`.
#[derive(Clone, Debug, Default)]
pub struct ReverseSchema {
    pub indexed_attrs: HashSet<Attr>,
    pub unique_attrs: HashSet<Attr>,
    pub unique_identity: HashSet<Attr>,
    pub unique_value: HashSet<Attr>,
    pub ref_attrs: HashSet<Attr>,
    pub multival_attrs: HashSet<Attr>,
    pub component_attrs: HashSet<Attr>,
    pub tuple_attrs: HashSet<Attr>,
}

impl ReverseSchema {
    /// Matches CLJS `indexing?` — true if attr has `:db/index` in rschema.
    pub fn is_indexed(&self, attr: &Attr) -> bool {
        self.indexed_attrs.contains(attr)
    }

    pub fn is_ref(&self, attr: &Attr) -> bool {
        self.ref_attrs.contains(attr)
    }

    pub fn is_multival(&self, attr: &Attr) -> bool {
        self.multival_attrs.contains(attr)
    }

    pub fn is_component(&self, attr: &Attr) -> bool {
        self.component_attrs.contains(attr)
    }
}

/// The implicit schema that DataScript always merges in.
/// Matches CLJS: `{:db/ident {:db/unique :db.unique/identity}}`
fn implicit_schema() -> HashMap<Attr, AttrSchema> {
    let mut m = HashMap::new();
    m.insert(
        Attr::Keyword { ns: Some("db".into()), name: "ident".into() },
        AttrSchema {
            unique: Some(Unique::Identity),
            ..Default::default()
        },
    );
    m
}

/// Build a `ReverseSchema` from a `Schema`, matching CLJS `attr->properties` logic.
///
/// The rules (from `db.cljc:886-896`):
/// - `:db.unique/identity` → `[:db/unique :db.unique/identity :db/index]`
/// - `:db.unique/value`    → `[:db/unique :db.unique/value :db/index]`
/// - `:db.cardinality/many` → `[:db.cardinality/many]`
/// - `:db.type/ref`        → `[:db.type/ref :db/index]`
/// - `:db/isComponent true` → `[:db/isComponent]`
/// - `:db/index true`      → `[:db/index]`
/// - `:db/tupleAttrs`      → `[:db.type/tuple :db/index]`
pub fn build_rschema(schema: &Schema) -> ReverseSchema {
    let merged = {
        let mut m = implicit_schema();
        for (attr, attr_schema) in &schema.attrs {
            m.insert(attr.clone(), attr_schema.clone());
        }
        m
    };

    let mut rs = ReverseSchema::default();

    for (attr, attr_schema) in &merged {
        // :db/index true
        if attr_schema.index {
            rs.indexed_attrs.insert(attr.clone());
        }

        // :db/unique
        match &attr_schema.unique {
            Some(Unique::Identity) => {
                rs.unique_attrs.insert(attr.clone());
                rs.unique_identity.insert(attr.clone());
                rs.indexed_attrs.insert(attr.clone());
            }
            Some(Unique::Value) => {
                rs.unique_attrs.insert(attr.clone());
                rs.unique_value.insert(attr.clone());
                rs.indexed_attrs.insert(attr.clone());
            }
            None => {}
        }

        // :db/cardinality :db.cardinality/many
        if attr_schema.cardinality == Cardinality::Many {
            rs.multival_attrs.insert(attr.clone());
        }

        // :db/valueType :db.type/ref
        if attr_schema.value_type == Some(ValueType::Ref) {
            rs.ref_attrs.insert(attr.clone());
            rs.indexed_attrs.insert(attr.clone());
        }

        // :db/isComponent true
        if attr_schema.is_component {
            rs.component_attrs.insert(attr.clone());
        }

        // :db/tupleAttrs
        if attr_schema.tuple_attrs.is_some() {
            rs.tuple_attrs.insert(attr.clone());
            rs.indexed_attrs.insert(attr.clone());
        }
    }

    rs
}

/// Helper to create an `Attr::Keyword` with no namespace.
pub fn kw(name: &str) -> Attr {
    Attr::Keyword { ns: None, name: name.into() }
}

/// Helper to create an `Attr::Keyword` with a namespace.
pub fn kw_ns(ns: &str, name: &str) -> Attr {
    Attr::Keyword { ns: Some(ns.into()), name: name.into() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db_ident() -> Attr {
        Attr::Keyword { ns: Some("db".into()), name: "ident".into() }
    }

    #[test]
    fn schema_parse_empty() {
        let schema = Schema::default();
        let rs = build_rschema(&schema);
        // Only :db/ident from implicit schema
        assert_eq!(rs.indexed_attrs.len(), 1);
        assert!(rs.indexed_attrs.contains(&db_ident()));
        assert_eq!(rs.ref_attrs.len(), 0);
        assert_eq!(rs.multival_attrs.len(), 0);
        assert_eq!(rs.component_attrs.len(), 0);
    }

    #[test]
    fn schema_index_flag() {
        let mut schema = Schema::default();
        schema.attrs.insert(kw("name"), AttrSchema { index: true, ..Default::default() });
        schema.attrs.insert(kw("age"), AttrSchema::default());
        let rs = build_rschema(&schema);

        assert!(rs.is_indexed(&kw("name")));
        assert!(!rs.is_indexed(&kw("age")));
    }

    #[test]
    fn schema_ref_implies_indexed() {
        let mut schema = Schema::default();
        schema.attrs.insert(kw("parent"), AttrSchema {
            value_type: Some(ValueType::Ref),
            ..Default::default()
        });
        let rs = build_rschema(&schema);

        assert!(rs.ref_attrs.contains(&kw("parent")));
        assert!(rs.indexed_attrs.contains(&kw("parent")));
    }

    #[test]
    fn schema_unique_identity_implies_indexed() {
        let mut schema = Schema::default();
        schema.attrs.insert(kw("email"), AttrSchema {
            unique: Some(Unique::Identity),
            ..Default::default()
        });
        let rs = build_rschema(&schema);

        assert!(rs.unique_identity.contains(&kw("email")));
        assert!(rs.unique_attrs.contains(&kw("email")));
        assert!(rs.indexed_attrs.contains(&kw("email")));
    }

    #[test]
    fn schema_unique_value_implies_indexed() {
        let mut schema = Schema::default();
        schema.attrs.insert(kw("code"), AttrSchema {
            unique: Some(Unique::Value),
            ..Default::default()
        });
        let rs = build_rschema(&schema);

        assert!(rs.unique_value.contains(&kw("code")));
        assert!(rs.unique_attrs.contains(&kw("code")));
        assert!(rs.indexed_attrs.contains(&kw("code")));
    }

    #[test]
    fn schema_cardinality_many() {
        let mut schema = Schema::default();
        schema.attrs.insert(kw("aka"), AttrSchema {
            cardinality: Cardinality::Many,
            ..Default::default()
        });
        let rs = build_rschema(&schema);

        assert!(rs.multival_attrs.contains(&kw("aka")));
        assert!(!rs.indexed_attrs.contains(&kw("aka")));
    }

    #[test]
    fn schema_is_component() {
        let mut schema = Schema::default();
        schema.attrs.insert(kw("child"), AttrSchema {
            is_component: true,
            value_type: Some(ValueType::Ref),
            ..Default::default()
        });
        let rs = build_rschema(&schema);

        assert!(rs.component_attrs.contains(&kw("child")));
        // Ref also implies indexed
        assert!(rs.ref_attrs.contains(&kw("child")));
        assert!(rs.indexed_attrs.contains(&kw("child")));
    }

    #[test]
    fn schema_complex_multi_attr() {
        let mut schema = Schema::default();
        schema.attrs.insert(kw("name"), AttrSchema { index: true, ..Default::default() });
        schema.attrs.insert(kw("email"), AttrSchema { unique: Some(Unique::Identity), ..Default::default() });
        schema.attrs.insert(kw("parent"), AttrSchema { value_type: Some(ValueType::Ref), ..Default::default() });
        schema.attrs.insert(kw("aka"), AttrSchema { cardinality: Cardinality::Many, ..Default::default() });
        schema.attrs.insert(kw("age"), AttrSchema::default());
        schema.attrs.insert(kw("child"), AttrSchema {
            value_type: Some(ValueType::Ref),
            is_component: true,
            ..Default::default()
        });
        let rs = build_rschema(&schema);

        // Indexed: name (explicit), email (unique), parent (ref), child (ref) + db/ident (implicit)
        assert!(rs.is_indexed(&kw("name")));
        assert!(rs.is_indexed(&kw("email")));
        assert!(rs.is_indexed(&kw("parent")));
        assert!(rs.is_indexed(&kw("child")));
        assert!(!rs.is_indexed(&kw("aka")));
        assert!(!rs.is_indexed(&kw("age")));

        assert!(rs.ref_attrs.contains(&kw("parent")));
        assert!(rs.ref_attrs.contains(&kw("child")));
        assert!(!rs.ref_attrs.contains(&kw("name")));

        assert!(rs.multival_attrs.contains(&kw("aka")));
        assert!(rs.component_attrs.contains(&kw("child")));
    }

    #[test]
    fn schema_implicit_db_ident() {
        // Even with empty user schema, :db/ident is in unique_identity + indexed
        let schema = Schema::default();
        let rs = build_rschema(&schema);

        assert!(rs.unique_identity.contains(&db_ident()));
        assert!(rs.indexed_attrs.contains(&db_ident()));
    }

    #[test]
    fn schema_indexing_predicate() {
        let mut schema = Schema::default();
        schema.attrs.insert(kw("name"), AttrSchema { index: true, ..Default::default() });
        schema.attrs.insert(kw("parent"), AttrSchema { value_type: Some(ValueType::Ref), ..Default::default() });
        schema.attrs.insert(kw("email"), AttrSchema { unique: Some(Unique::Identity), ..Default::default() });
        schema.attrs.insert(kw("age"), AttrSchema::default());
        schema.attrs.insert(kw("aka"), AttrSchema { cardinality: Cardinality::Many, ..Default::default() });
        let rs = build_rschema(&schema);

        // Indexed
        assert!(rs.is_indexed(&kw("name")));
        assert!(rs.is_indexed(&kw("parent")));
        assert!(rs.is_indexed(&kw("email")));

        // Not indexed
        assert!(!rs.is_indexed(&kw("age")));
        assert!(!rs.is_indexed(&kw("aka")));
        assert!(!rs.is_indexed(&kw("nonexistent")));
    }
}
