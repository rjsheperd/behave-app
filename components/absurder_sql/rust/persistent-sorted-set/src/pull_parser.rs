//! Pull pattern parser for DataScript pull expressions.
//!
//! Parses EDN pull patterns like `[:name :age {:friends [:name]}]` into
//! `PullPattern` structs that the pull executor can walk.

use edn::parser::Parser;
use edn::Value as EdnValue;

use crate::datom::Attr;
use crate::schema::{ReverseSchema, Schema};

/// A single attribute in a pull pattern.
#[derive(Clone, Debug)]
pub struct PullAttr {
    /// The canonical attribute to look up in the DB.
    pub name: Attr,
    /// The output key (usually same as `name`, differs for reverse refs).
    pub as_name: Attr,
    /// True for reverse references (`:foo/_bar`).
    pub reverse: bool,
    /// True if `:db.cardinality/many`.
    pub multival: bool,
    /// True if `:db.type/ref`.
    pub ref_type: bool,
    /// True if `:db/isComponent`.
    pub component: bool,
    /// Nested pattern for ref attributes (join).
    pub pattern: Option<PullPattern>,
}

/// A parsed pull pattern.
#[derive(Clone, Debug)]
pub struct PullPattern {
    /// Forward attributes, sorted by keyword.
    pub attrs: Vec<PullAttr>,
    /// Reverse attributes, sorted by keyword.
    pub reverse_attrs: Vec<PullAttr>,
    /// True if the pattern includes `*`.
    pub wildcard: bool,
}

// ---------------------------------------------------------------------------
// EDN keyword → Attr conversion (reused from query_parser)
// ---------------------------------------------------------------------------

fn keyword_to_attr(kw: &str) -> Attr {
    let inner = kw.strip_prefix(':').unwrap_or(kw);
    if let Some((ns, name)) = inner.split_once('/') {
        Attr::Keyword {
            ns: Some(ns.to_string()),
            name: name.to_string(),
        }
    } else {
        Attr::Keyword {
            ns: None,
            name: inner.to_string(),
        }
    }
}

/// Parse a reverse-ref keyword like `:variable/_group-variables`.
/// Returns `(forward_attr, as_attr)` where:
/// - `forward_attr` is the actual DB attribute (`:variable/group-variables`)
/// - `as_attr` is the output key (`:variable/_group-variables`)
fn parse_reverse_attr(kw: &str) -> Option<(Attr, Attr)> {
    let inner = kw.strip_prefix(':').unwrap_or(kw);
    if let Some((ns, name)) = inner.split_once('/') {
        if let Some(stripped) = name.strip_prefix('_') {
            let forward = Attr::Keyword {
                ns: Some(ns.to_string()),
                name: stripped.to_string(),
            };
            let as_name = Attr::Keyword {
                ns: Some(ns.to_string()),
                name: name.to_string(), // keeps the _
            };
            return Some((forward, as_name));
        }
    }
    // Non-namespaced reverse ref like `:_parent`
    if let Some(stripped) = inner.strip_prefix('_') {
        let forward = Attr::Keyword {
            ns: None,
            name: stripped.to_string(),
        };
        let as_name = Attr::Keyword {
            ns: None,
            name: inner.to_string(), // keeps the _
        };
        return Some((forward, as_name));
    }
    None
}

fn is_reverse_kw(kw: &str) -> bool {
    let inner = kw.strip_prefix(':').unwrap_or(kw);
    if let Some((_, name)) = inner.split_once('/') {
        name.starts_with('_')
    } else {
        inner.starts_with('_')
    }
}

// ---------------------------------------------------------------------------
// Pattern parsing
// ---------------------------------------------------------------------------

/// Parse a pull pattern from an EDN string.
pub fn parse_pull_pattern_edn(
    schema: &Schema,
    rschema: &ReverseSchema,
    edn_str: &str,
) -> PullPattern {
    let cleaned = crate::query_parser::strip_edn_comments(edn_str);
    let mut parser = Parser::new(&cleaned);
    let val = parser
        .read()
        .expect("failed to parse pull pattern EDN")
        .expect("empty pull pattern EDN");
    parse_pull_pattern(schema, rschema, &val)
}

/// Parse a pull pattern from an already-parsed EDN value.
pub fn parse_pull_pattern(
    schema: &Schema,
    rschema: &ReverseSchema,
    edn: &EdnValue,
) -> PullPattern {
    // Accept both vector pattern `[:attr ...]` and bare map `{:ref [...]}`.
    // A bare map is treated as a single-element pattern `[{:ref [...]}]`.
    if let EdnValue::Map(_) = edn {
        let wrapped = EdnValue::Vector(vec![edn.clone()]);
        return parse_pull_pattern(schema, rschema, &wrapped);
    }

    let elems = match edn {
        EdnValue::Vector(v) => v.as_slice(),
        _ => panic!("pull pattern must be a vector, got {:?}", edn),
    };

    let mut attrs = Vec::new();
    let mut reverse_attrs = Vec::new();
    let mut wildcard = false;

    for elem in elems {
        match elem {
            // Wildcard: * or "*"
            EdnValue::Symbol(s) if s == "*" => {
                wildcard = true;
            }
            EdnValue::String(s) if s == "*" => {
                wildcard = true;
            }

            // Keyword attribute: :name, :person/age, :variable/_group-vars
            EdnValue::Keyword(kw) => {
                let pull_attr = parse_keyword_attr(kw, schema, rschema, None);
                if pull_attr.reverse {
                    reverse_attrs.push(pull_attr);
                } else {
                    attrs.push(pull_attr);
                }
            }

            // Map spec: {:ref-attr [...pattern...]} or {:ref-attr ...}
            EdnValue::Map(pairs) => {
                for (k, v) in pairs {
                    if let EdnValue::Keyword(kw) = k {
                        let nested = match v {
                            EdnValue::Vector(_) => {
                                Some(parse_pull_pattern(schema, rschema, v))
                            }
                            // Recursion markers — not implemented, treat as wildcard pattern
                            EdnValue::Symbol(s) if s == "..." => {
                                Some(PullPattern {
                                    attrs: vec![],
                                    reverse_attrs: vec![],
                                    wildcard: true,
                                })
                            }
                            EdnValue::Integer(_) => {
                                Some(PullPattern {
                                    attrs: vec![],
                                    reverse_attrs: vec![],
                                    wildcard: true,
                                })
                            }
                            _ => None,
                        };
                        let pull_attr = parse_keyword_attr(kw, schema, rschema, nested);
                        if pull_attr.reverse {
                            reverse_attrs.push(pull_attr);
                        } else {
                            attrs.push(pull_attr);
                        }
                    }
                }
            }

            _ => {} // skip unrecognized elements
        }
    }

    // Add :db/id if wildcard
    if wildcard {
        let db_id = Attr::Keyword {
            ns: Some("db".into()),
            name: "id".into(),
        };
        let has_db_id = attrs.iter().any(|a| a.name == db_id);
        if !has_db_id {
            attrs.push(PullAttr {
                name: db_id.clone(),
                as_name: db_id,
                reverse: false,
                multival: false,
                ref_type: false,
                component: false,
                pattern: None,
            });
        }
    }

    // Sort for merge-join against sorted datoms
    attrs.sort_by(|a, b| a.name.cmp(&b.name));
    reverse_attrs.sort_by(|a, b| a.name.cmp(&b.name));

    PullPattern {
        attrs,
        reverse_attrs,
        wildcard,
    }
}

fn parse_keyword_attr(
    kw: &str,
    schema: &Schema,
    rschema: &ReverseSchema,
    nested_pattern: Option<PullPattern>,
) -> PullAttr {
    if is_reverse_kw(kw) {
        let (forward, as_name) = parse_reverse_attr(kw)
            .expect("invalid reverse ref keyword");
        let component = rschema.is_component(&forward) || schema.attrs.get(&forward).map_or(false, |s| s.is_component);
        PullAttr {
            name: forward,
            as_name,
            reverse: true,
            multival: true, // reverse refs always produce collections
            ref_type: true,
            component,
            pattern: nested_pattern,
        }
    } else {
        let attr = keyword_to_attr(kw);
        let multival = rschema.is_multival(&attr);
        let ref_type = rschema.is_ref(&attr);
        let component = rschema.is_component(&attr);
        PullAttr {
            name: attr.clone(),
            as_name: attr,
            reverse: false,
            multival,
            ref_type,
            component,
            pattern: nested_pattern,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{AttrSchema, Cardinality, Schema, ValueType, kw, kw_ns};

    fn test_schema() -> (Schema, ReverseSchema) {
        let mut schema = Schema::default();
        schema.attrs.insert(kw("name"), AttrSchema { index: true, ..Default::default() });
        schema.attrs.insert(kw("age"), AttrSchema::default());
        schema.attrs.insert(kw("aka"), AttrSchema { cardinality: Cardinality::Many, ..Default::default() });
        schema.attrs.insert(kw("parent"), AttrSchema {
            value_type: Some(ValueType::Ref),
            ..Default::default()
        });
        schema.attrs.insert(kw("children"), AttrSchema {
            value_type: Some(ValueType::Ref),
            cardinality: Cardinality::Many,
            ..Default::default()
        });
        schema.attrs.insert(kw_ns("ws", "inputs"), AttrSchema {
            value_type: Some(ValueType::Ref),
            cardinality: Cardinality::Many,
            is_component: true,
            ..Default::default()
        });
        let rschema = crate::schema::build_rschema(&schema);
        (schema, rschema)
    }

    #[test]
    fn parse_simple_attrs() {
        let (s, rs) = test_schema();
        let p = parse_pull_pattern_edn(&s, &rs, "[:name :age]");
        assert_eq!(p.attrs.len(), 2);
        assert!(!p.wildcard);
        assert!(p.reverse_attrs.is_empty());
        assert_eq!(p.attrs[0].name, kw("age"));
        assert_eq!(p.attrs[1].name, kw("name"));
    }

    #[test]
    fn parse_wildcard() {
        let (s, rs) = test_schema();
        let p = parse_pull_pattern_edn(&s, &rs, "[*]");
        assert!(p.wildcard);
        // Should auto-add :db/id
        assert!(p.attrs.iter().any(|a| a.name == kw_ns("db", "id")));
    }

    #[test]
    fn parse_nested_join() {
        let (s, rs) = test_schema();
        let p = parse_pull_pattern_edn(&s, &rs, "[:name {:parent [:name :age]}]");
        assert_eq!(p.attrs.len(), 2);
        let parent_attr = p.attrs.iter().find(|a| a.name == kw("parent")).unwrap();
        assert!(parent_attr.ref_type);
        assert!(parent_attr.pattern.is_some());
        let nested = parent_attr.pattern.as_ref().unwrap();
        assert_eq!(nested.attrs.len(), 2);
    }

    #[test]
    fn parse_reverse_ref() {
        let (s, rs) = test_schema();
        let p = parse_pull_pattern_edn(&s, &rs, "[{:_parent [:name]}]");
        assert!(p.attrs.is_empty());
        assert_eq!(p.reverse_attrs.len(), 1);
        let rev = &p.reverse_attrs[0];
        assert!(rev.reverse);
        assert_eq!(rev.name, kw("parent")); // forward attr
        assert!(rev.pattern.is_some());
    }

    #[test]
    fn parse_namespaced_reverse_ref() {
        let (s, rs) = test_schema();
        let p = parse_pull_pattern_edn(&s, &rs, "[{:ws/_inputs [:name]}]");
        assert_eq!(p.reverse_attrs.len(), 1);
        let rev = &p.reverse_attrs[0];
        assert!(rev.reverse);
        assert_eq!(rev.name, kw_ns("ws", "inputs"));
    }

    #[test]
    fn parse_multival_attr() {
        let (s, rs) = test_schema();
        let p = parse_pull_pattern_edn(&s, &rs, "[:aka]");
        assert_eq!(p.attrs.len(), 1);
        assert!(p.attrs[0].multival);
    }

    #[test]
    fn parse_wildcard_with_nested() {
        let (s, rs) = test_schema();
        let p = parse_pull_pattern_edn(&s, &rs, "[* {:children [:name]}]");
        assert!(p.wildcard);
        let children = p.attrs.iter().find(|a| a.name == kw("children")).unwrap();
        assert!(children.pattern.is_some());
    }

    #[test]
    fn parse_db_id_explicit() {
        let (s, rs) = test_schema();
        let p = parse_pull_pattern_edn(&s, &rs, "[:db/id :name]");
        assert!(!p.wildcard);
        assert!(p.attrs.iter().any(|a| a.name == kw_ns("db", "id")));
    }
}
