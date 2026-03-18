//! Behave app integration tests.
//!
//! Exercises the real query patterns from the behave application against the
//! Rust DataScript engine. Each test mirrors an actual query from the codebase
//! (with file:line references) to verify end-to-end parity.
//!
//! Two databases are used:
//!   - VMS DB ($):  vocabulary/model/schema — modules, submodules, groups,
//!                  group-variables, variables, domains, units
//!   - Worksheet DB ($ws): user workspace — worksheets, input-groups, inputs,
//!                         outputs, result-tables

use std::collections::HashMap;

use persistent_sorted_set::datom::{Datom, Value};
use persistent_sorted_set::db::{DataScriptDB, TX0};
use persistent_sorted_set::query_parser::{
    bind_inputs, build_collection_relations, parse_query, parse_rules,
};
use persistent_sorted_set::relation::{
    project, resolve_query, resolve_query_with_initial,
    MultiResolver, Rules,
};
use persistent_sorted_set::schema::{
    kw, kw_ns, AttrSchema, Cardinality, Schema, Unique, ValueType,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn d(e: i64, ns: &str, name: &str, v: Value, tx: i64) -> Datom {
    Datom::new(e, Some(kw_ns(ns, name)), v, TX0 + tx)
}

fn s(val: &str) -> Value { Value::Str(val.into()) }
fn r(val: i64) -> Value { Value::Ref(val) }
fn n(val: i64) -> Value { Value::Long(val) }
fn b(val: bool) -> Value { Value::Bool(val) }
fn kv(name: &str) -> Value { Value::Keyword(kw(name)) }

/// Run EDN query with scalar inputs against a single DB.
fn run_q(
    db: &DataScriptDB,
    query_edn: &str,
    rules_edn: &str,
    inputs: &[(&str, Value)],
) -> Vec<Vec<Value>> {
    let mut q = parse_query(query_edn);
    let rules = if rules_edn.is_empty() {
        Rules::new()
    } else {
        parse_rules(rules_edn)
    };
    bind_inputs(&mut q, inputs);
    let result = resolve_query(db, &q.where_clauses, &rules);
    let projected = project(&result, &q.find.vars());
    projected.tuples
}

/// Run EDN query with collection inputs against a single DB.
fn run_q_coll(
    db: &DataScriptDB,
    query_edn: &str,
    rules_edn: &str,
    scalar_inputs: &[(&str, Value)],
    coll_inputs: &[(&str, Vec<Value>)],
) -> Vec<Vec<Value>> {
    let mut q = parse_query(query_edn);
    let rules = if rules_edn.is_empty() {
        Rules::new()
    } else {
        parse_rules(rules_edn)
    };
    bind_inputs(&mut q, scalar_inputs);
    let coll_map: HashMap<String, Vec<Value>> = coll_inputs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    let initial_rels = build_collection_relations(&q.in_bindings, &coll_map);
    let result = resolve_query_with_initial(db, &q.where_clauses, &rules, initial_rels);
    let projected = project(&result, &q.find.vars());
    projected.tuples
}

/// Run EDN query against a MultiResolver (multi-source).
fn run_q_multi<'a>(
    resolver: &MultiResolver<'a>,
    query_edn: &str,
    rules_edn: &str,
    scalar_inputs: &[(&str, Value)],
    coll_inputs: &[(&str, Vec<Value>)],
) -> Vec<Vec<Value>> {
    let mut q = parse_query(query_edn);
    let rules = if rules_edn.is_empty() {
        Rules::new()
    } else {
        parse_rules(rules_edn)
    };
    bind_inputs(&mut q, scalar_inputs);
    let coll_map: HashMap<String, Vec<Value>> = coll_inputs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    let initial_rels = build_collection_relations(&q.in_bindings, &coll_map);
    let result = resolve_query_with_initial(resolver, &q.where_clauses, &rules, initial_rels);
    let projected = project(&result, &q.find.vars());
    projected.tuples
}

/// Extract string values from column `col`.
fn strs(tuples: &[Vec<Value>], col: usize) -> Vec<String> {
    let mut v: Vec<String> = tuples
        .iter()
        .filter_map(|t| match &t[col] {
            Value::Str(s) => Some(s.clone()),
            _ => None,
        })
        .collect();
    v.sort();
    v
}

/// Extract i64 values from column `col`.
fn longs(tuples: &[Vec<Value>], col: usize) -> Vec<i64> {
    let mut v: Vec<i64> = tuples
        .iter()
        .filter_map(|t| match &t[col] {
            Value::Long(n) => Some(*n),
            _ => None,
        })
        .collect();
    v.sort();
    v
}

// ---------------------------------------------------------------------------
// Schema
// ---------------------------------------------------------------------------

fn vms_schema() -> Schema {
    let mut s = Schema::default();
    let ref_many = AttrSchema {
        value_type: Some(ValueType::Ref),
        cardinality: Cardinality::Many,
        ..Default::default()
    };
    let ref_one = AttrSchema {
        value_type: Some(ValueType::Ref),
        ..Default::default()
    };
    let indexed = AttrSchema { index: true, ..Default::default() };
    let uuid_attr = AttrSchema {
        index: true,
        unique: Some(Unique::Identity),
        ..Default::default()
    };

    // bp
    s.attrs.insert(kw_ns("bp", "uuid"), uuid_attr.clone());

    // application
    s.attrs.insert(kw_ns("application", "name"), indexed.clone());
    s.attrs.insert(kw_ns("application", "modules"), ref_many.clone());

    // module
    s.attrs.insert(kw_ns("module", "name"), indexed.clone());
    s.attrs.insert(kw_ns("module", "submodules"), ref_many.clone());
    s.attrs.insert(kw_ns("module", "diagrams"), ref_many.clone());
    s.attrs.insert(kw_ns("module", "search-tables"), ref_many.clone());

    // submodule
    s.attrs.insert(kw_ns("submodule", "name"), indexed.clone());
    s.attrs.insert(kw_ns("submodule", "groups"), ref_many.clone());
    s.attrs.insert(kw_ns("submodule", "io"), indexed.clone());

    // group
    s.attrs.insert(kw_ns("group", "name"), indexed.clone());
    s.attrs.insert(kw_ns("group", "children"), ref_many.clone());
    s.attrs.insert(kw_ns("group", "group-variables"), ref_many.clone());
    s.attrs.insert(kw_ns("group", "translation-key"), indexed.clone());

    // group-variable
    s.attrs.insert(kw_ns("group-variable", "translation-key"), indexed.clone());
    s.attrs.insert(kw_ns("group-variable", "hide-result?"), Default::default());
    s.attrs.insert(kw_ns("group-variable", "hide-graph?"), Default::default());
    s.attrs.insert(kw_ns("group-variable", "hide-csv?"), Default::default());
    s.attrs.insert(kw_ns("group-variable", "discrete-multiple?"), Default::default());
    s.attrs.insert(kw_ns("group-variable", "conditionally-set?"), Default::default());
    s.attrs.insert(kw_ns("group-variable", "direction"), indexed.clone());
    s.attrs.insert(kw_ns("group-variable", "cpp-parameter"), Default::default());
    s.attrs.insert(kw_ns("group-variable", "cpp-class"), Default::default());

    // variable
    s.attrs.insert(kw_ns("variable", "name"), indexed.clone());
    s.attrs.insert(kw_ns("variable", "group-variables"), ref_many.clone());
    s.attrs.insert(kw_ns("variable", "kind"), indexed.clone());
    s.attrs.insert(kw_ns("variable", "native-unit-uuid"), Default::default());
    s.attrs.insert(kw_ns("variable", "domain-uuid"), Default::default());

    // domain
    s.attrs.insert(kw_ns("domain", "name"), indexed.clone());
    s.attrs.insert(kw_ns("domain", "native-unit-uuid"), Default::default());
    s.attrs.insert(kw_ns("domain", "dimension-uuid"), Default::default());
    s.attrs.insert(kw_ns("domain", "english-unit-uuid"), Default::default());
    s.attrs.insert(kw_ns("domain", "metric-unit-uuid"), Default::default());
    s.attrs.insert(kw_ns("domain", "decimals"), Default::default());
    s.attrs.insert(kw_ns("domain-set", "name"), indexed.clone());
    s.attrs.insert(kw_ns("domain-set", "domains"), ref_many.clone());

    // link
    s.attrs.insert(kw_ns("link", "source"), ref_one.clone());
    s.attrs.insert(kw_ns("link", "destination"), ref_one.clone());

    // diagram
    s.attrs.insert(kw_ns("diagram", "group-variable"), ref_one.clone());
    s.attrs.insert(kw_ns("diagram", "input-group-variables"), ref_many.clone());
    s.attrs.insert(kw_ns("diagram", "output-group-variables"), ref_many.clone());

    // language
    s.attrs.insert(kw_ns("language", "shortcode"), uuid_attr.clone());
    s.attrs.insert(kw_ns("language", "name"), indexed.clone());
    s.attrs.insert(kw_ns("language", "translation"), ref_many.clone());

    // translation
    s.attrs.insert(kw_ns("translation", "key"), indexed.clone());
    s.attrs.insert(kw_ns("translation", "translation"), indexed.clone());

    // cpp
    s.attrs.insert(kw_ns("cpp.class", "name"), indexed.clone());

    // subtool-variable
    s.attrs.insert(kw_ns("subtool-variable", "cpp-class-uuid"), Default::default());
    s.attrs.insert(kw_ns("subtool-variable", "io"), indexed.clone());
    s.attrs.insert(kw_ns("subtool", "variables"), ref_many.clone());

    s
}

fn ws_schema() -> Schema {
    let mut s = Schema::default();
    let ref_many = AttrSchema {
        value_type: Some(ValueType::Ref),
        cardinality: Cardinality::Many,
        ..Default::default()
    };
    let ref_one = AttrSchema {
        value_type: Some(ValueType::Ref),
        ..Default::default()
    };
    let indexed = AttrSchema { index: true, ..Default::default() };

    // worksheet
    s.attrs.insert(kw_ns("worksheet", "uuid"), AttrSchema {
        index: true,
        unique: Some(Unique::Identity),
        ..Default::default()
    });
    s.attrs.insert(kw_ns("worksheet", "created"), indexed.clone());
    s.attrs.insert(kw_ns("worksheet", "modules"), AttrSchema {
        cardinality: Cardinality::Many,
        ..Default::default()
    });
    s.attrs.insert(kw_ns("worksheet", "input-groups"), ref_many.clone());
    s.attrs.insert(kw_ns("worksheet", "outputs"), ref_many.clone());
    s.attrs.insert(kw_ns("worksheet", "result-table"), ref_one.clone());

    // input-group
    s.attrs.insert(kw_ns("input-group", "group-uuid"), indexed.clone());
    s.attrs.insert(kw_ns("input-group", "repeat-id"), indexed.clone());
    s.attrs.insert(kw_ns("input-group", "inputs"), ref_many.clone());

    // input
    s.attrs.insert(kw_ns("input", "group-variable-uuid"), indexed.clone());
    s.attrs.insert(kw_ns("input", "value"), Default::default());
    s.attrs.insert(kw_ns("input", "units"), Default::default());
    s.attrs.insert(kw_ns("input", "units-uuid"), Default::default());

    // output
    s.attrs.insert(kw_ns("output", "group-variable-uuid"), indexed.clone());
    s.attrs.insert(kw_ns("output", "enabled?"), Default::default());

    // result-table
    s.attrs.insert(kw_ns("result-table", "headers"), ref_many.clone());
    s.attrs.insert(kw_ns("result-table", "rows"), ref_many.clone());

    // result-header
    s.attrs.insert(kw_ns("result-header", "group-variable-uuid"), indexed.clone());
    s.attrs.insert(kw_ns("result-header", "repeat-id"), indexed.clone());
    s.attrs.insert(kw_ns("result-header", "units"), Default::default());

    // result-row
    s.attrs.insert(kw_ns("result-row", "id"), indexed.clone());
    s.attrs.insert(kw_ns("result-row", "cells"), ref_many.clone());

    // result-cell
    s.attrs.insert(kw_ns("result-cell", "header"), ref_one.clone());
    s.attrs.insert(kw_ns("result-cell", "value"), Default::default());

    s
}

// ---------------------------------------------------------------------------
// Test data
// ---------------------------------------------------------------------------

/// VMS database: BehavePlus app → Surface module → Weighted submodule
///   Groups: FuelModel (30) → children: Dead (31) → 1hr (32)
///           Moisture (33)
///   GVs: gv-fuel-load (40) on FuelModel, gv-moisture (41) on Moisture
///         gv-wind-speed (42) on FuelModel (output, conditionally-set)
///   Variables: fuelLoad (50) ← gv(40), moistureContent (51) ← gv(41)
///   Links: gv(40) → gv(41)
///   Domains: Length domain (70) with units
fn vms_db() -> DataScriptDB {
    let mut db = DataScriptDB::empty(vms_schema());
    db.with_datoms(vec![
        // Application
        d(1, "application", "name", s("BehavePlus"), 1),
        d(1, "application", "modules", r(10), 1),
        d(1, "bp", "uuid", s("app-uuid"), 1),

        // Module: Surface
        d(10, "module", "name", s("Surface"), 1),
        d(10, "module", "submodules", r(20), 1),
        d(10, "module", "diagrams", r(80), 1),
        d(10, "bp", "uuid", s("mod-surface"), 1),

        // Submodule: Weighted (input)
        d(20, "submodule", "name", s("Weighted"), 1),
        d(20, "submodule", "groups", r(30), 1),
        d(20, "submodule", "io", kv("input"), 1),
        d(20, "bp", "uuid", s("submod-weighted"), 1),

        // Submodule: FireBehavior (output)
        d(21, "submodule", "name", s("FireBehavior"), 1),
        d(21, "submodule", "groups", r(33), 1),
        d(21, "submodule", "io", kv("output"), 1),
        d(21, "bp", "uuid", s("submod-fire"), 1),

        // Module has both submodules
        d(10, "module", "submodules", r(21), 1),

        // Group: FuelModel (top-level under Weighted)
        d(30, "group", "name", s("FuelModel"), 1),
        d(30, "group", "children", r(31), 1),
        d(30, "group", "group-variables", r(40), 1),
        d(30, "group", "translation-key", s("fuel_model"), 1),
        d(30, "bp", "uuid", s("group-fuel"), 1),

        // Group: Dead (child of FuelModel)
        d(31, "group", "name", s("Dead"), 1),
        d(31, "group", "children", r(32), 1),
        d(31, "bp", "uuid", s("group-dead"), 1),

        // Group: 1hr (child of Dead)
        d(32, "group", "name", s("1hr"), 1),
        d(32, "bp", "uuid", s("group-1hr"), 1),

        // Group: Moisture (top-level under FireBehavior output submodule)
        d(33, "group", "name", s("Moisture"), 1),
        d(33, "group", "group-variables", r(41), 1),
        d(33, "group", "group-variables", r(42), 1),
        d(33, "bp", "uuid", s("group-moisture"), 1),

        // Group-variable: fuel load
        d(40, "bp", "uuid", s("gv-fuel-load"), 1),
        d(40, "group-variable", "translation-key", s("gv_fuel_load"), 1),
        d(40, "group-variable", "direction", s("upslope"), 1),

        // Group-variable: moisture content (has hide-result)
        d(41, "bp", "uuid", s("gv-moisture"), 1),
        d(41, "group-variable", "translation-key", s("gv_moisture"), 1),
        d(41, "group-variable", "hide-result?", b(true), 1),
        d(41, "group-variable", "hide-graph?", b(true), 1),

        // Group-variable: wind speed (output, conditionally set)
        d(42, "bp", "uuid", s("gv-wind-speed"), 1),
        d(42, "group-variable", "translation-key", s("gv_wind_speed"), 1),
        d(42, "group-variable", "conditionally-set?", b(true), 1),

        // Variable: fuelLoad (continuous, linked to gv-40)
        d(50, "variable", "name", s("fuelLoad"), 1),
        d(50, "variable", "group-variables", r(40), 1),
        d(50, "variable", "kind", kv("continuous"), 1),
        d(50, "variable", "native-unit-uuid", s("unit-tons-acre"), 1),
        d(50, "variable", "domain-uuid", s("domain-length"), 1),
        d(50, "bp", "uuid", s("var-fuel-load"), 1),

        // Variable: moistureContent (discrete, linked to gv-41)
        d(51, "variable", "name", s("moistureContent"), 1),
        d(51, "variable", "group-variables", r(41), 1),
        d(51, "variable", "kind", kv("discrete"), 1),
        d(51, "bp", "uuid", s("var-moisture"), 1),

        // Variable: windSpeed (continuous, linked to gv-42)
        d(52, "variable", "name", s("windSpeed"), 1),
        d(52, "variable", "group-variables", r(42), 1),
        d(52, "variable", "kind", kv("continuous"), 1),
        d(52, "variable", "native-unit-uuid", s("unit-mph"), 1),
        d(52, "bp", "uuid", s("var-wind-speed"), 1),

        // Domain: Length
        d(70, "domain", "name", s("Length"), 1),
        d(70, "bp", "uuid", s("domain-length"), 1),
        d(70, "domain", "native-unit-uuid", s("unit-feet"), 1),
        d(70, "domain", "dimension-uuid", s("dim-length"), 1),
        d(70, "domain", "english-unit-uuid", s("unit-feet"), 1),
        d(70, "domain", "metric-unit-uuid", s("unit-meters"), 1),
        d(70, "domain", "decimals", n(2), 1),

        // Domain-set
        d(75, "domain-set", "name", s("Fire"), 1),
        d(75, "domain-set", "domains", r(70), 1),

        // Links: gv(40) → gv(41)
        d(90, "link", "source", r(40), 1),
        d(90, "link", "destination", r(41), 1),

        // Diagram (for module-diagrams query)
        d(80, "diagram", "group-variable", r(40), 1),
        d(80, "diagram", "input-group-variables", r(40), 1),
        d(80, "diagram", "output-group-variables", r(41), 1),
        d(80, "bp", "uuid", s("diagram-1"), 1),

        // Language: English
        d(60, "language", "shortcode", s("en"), 1),
        d(60, "language", "name", s("English"), 1),
        d(60, "language", "translation", r(62), 1),
        d(60, "language", "translation", r(63), 1),

        // Translations
        d(62, "translation", "key", s("fuel_model"), 1),
        d(62, "translation", "translation", s("Fuel Model"), 1),
        d(63, "translation", "key", s("gv_fuel_load"), 1),
        d(63, "translation", "translation", s("Fuel Load"), 1),

        // CPP class
        d(95, "cpp.class", "name", s("FuelBed"), 1),
        d(95, "bp", "uuid", s("cpp-fuelbed"), 1),
    ]);
    db
}

/// Worksheet DB: one worksheet with two input groups and outputs.
fn ws_db() -> DataScriptDB {
    let mut db = DataScriptDB::empty(ws_schema());
    db.with_datoms(vec![
        // Worksheet
        d(100, "worksheet", "uuid", s("ws-1"), 1),
        d(100, "worksheet", "created", n(1710000000), 1),
        d(100, "worksheet", "modules", s("Surface"), 1),
        d(100, "worksheet", "input-groups", r(110), 1),
        d(100, "worksheet", "input-groups", r(111), 1),
        d(100, "worksheet", "outputs", r(130), 1),
        d(100, "worksheet", "outputs", r(131), 1),
        d(100, "worksheet", "outputs", r(132), 1),
        d(100, "worksheet", "result-table", r(140), 1),

        // Input-group 1: FuelModel, repeat 0
        d(110, "input-group", "group-uuid", s("group-fuel"), 1),
        d(110, "input-group", "repeat-id", n(0), 1),
        d(110, "input-group", "inputs", r(120), 1),

        // Input 1: fuel-load value
        d(120, "input", "group-variable-uuid", s("gv-fuel-load"), 1),
        d(120, "input", "value", s("2.5"), 1),
        d(120, "input", "units-uuid", s("unit-tons-acre"), 1),

        // Input-group 2: FuelModel, repeat 1
        d(111, "input-group", "group-uuid", s("group-fuel"), 1),
        d(111, "input-group", "repeat-id", n(1), 1),
        d(111, "input-group", "inputs", r(121), 1),

        // Input 2: fuel-load value (repeat)
        d(121, "input", "group-variable-uuid", s("gv-fuel-load"), 1),
        d(121, "input", "value", s("3.0"), 1),
        d(121, "input", "units-uuid", s("unit-tons-acre"), 1),

        // Output 1: fuel-load (enabled)
        d(130, "output", "group-variable-uuid", s("gv-fuel-load"), 1),
        d(130, "output", "enabled?", b(true), 1),

        // Output 2: moisture (enabled)
        d(131, "output", "group-variable-uuid", s("gv-moisture"), 1),
        d(131, "output", "enabled?", b(true), 1),

        // Output 3: wind-speed (disabled)
        d(132, "output", "group-variable-uuid", s("gv-wind-speed"), 1),
        d(132, "output", "enabled?", b(false), 1),

        // Result table
        d(140, "result-table", "headers", r(141), 1),
        d(140, "result-table", "headers", r(142), 1),
        d(140, "result-table", "rows", r(150), 1),

        // Result headers
        d(141, "result-header", "group-variable-uuid", s("gv-fuel-load"), 1),
        d(141, "result-header", "repeat-id", n(0), 1),
        d(141, "result-header", "units", s("tons/acre"), 1),

        d(142, "result-header", "group-variable-uuid", s("gv-moisture"), 1),
        d(142, "result-header", "repeat-id", n(0), 1),
        d(142, "result-header", "units", s("%"), 1),

        // Result row
        d(150, "result-row", "id", n(0), 1),
        d(150, "result-row", "cells", r(151), 1),
        d(150, "result-row", "cells", r(152), 1),

        // Result cells
        d(151, "result-cell", "header", r(141), 1),
        d(151, "result-cell", "value", s("2.5"), 1),

        d(152, "result-cell", "header", r(142), 1),
        d(152, "result-cell", "value", s("12.0"), 1),
    ]);
    db
}

// Behave rules (subset used in tests)
const BEHAVE_RULES: &str = r#"[
    [(lookup ?uuid ?e) [?e :bp/uuid ?uuid]]

    [(submodule-root ?submodule ?subgroup)
     [?submodule :submodule/groups ?subgroup]]
    [(submodule-root ?submodule ?subgroup)
     (subgroup ?group ?subgroup)
     [?submodule :submodule/groups ?group]]

    [(subgroup ?g ?s)
     [?g :group/children ?s]]
    [(subgroup ?g ?s)
     [?g :group/children ?x]
     (subgroup ?x ?s)]

    [(group ?s ?g)
     [?s :submodule/groups ?g]]
    [(group ?s ?g)
     [?s :submodule/groups ?x]
     (subgroup ?x ?g)]

    [(submodule ?m ?s)
     [?m :module/submodules ?s]]

    [(group-variable ?g ?gv ?v)
     [?g :group/group-variables ?gv]
     [?v :variable/group-variables ?gv]]

    [(io ?e ?io)
     [?e :submodule/io ?io]]
    [(io ?e ?io)
     (group ?s ?e)
     [?s :submodule/io ?io]]
    [(io ?e ?io)
     (group-variable ?g ?e ?v)
     (group ?s ?g)
     [?s :submodule/io ?io]]

    [(variable-kind ?gv-uuid ?kind)
     (lookup ?gv-uuid ?gv)
     [?v :variable/group-variables ?gv]
     [?v :variable/kind ?kind]]
]"#;

// =========================================================================
// Single-DB VMS queries (solver/queries.cljs, vms/subs.cljs)
// =========================================================================

/// solver/queries.cljs:183-190 — find entity by cpp class name
#[test]
fn vms_class_to_group_variables() {
    let db = vms_db();
    let result = run_q(
        &db,
        "[:find ?c-uuid .
          :in $ ?class-name
          :where
          [?c :cpp.class/name ?class-name]
          [?c :bp/uuid ?c-uuid]]",
        "",
        &[("?class-name", s("FuelBed"))],
    );
    assert_eq!(result.len(), 1);
    assert_eq!(result[0][0], s("cpp-fuelbed"));
}

/// solver/queries.cljs:210-218 — source-links with collection binding
#[test]
fn vms_source_links_collection_binding() {
    let db = vms_db();
    let result = run_q_coll(
        &db,
        "[:find ?gv-uuid ?destination-uuid
          :in $ [?gv-uuid ...]
          :where
          [?s :bp/uuid ?gv-uuid]
          [?l :link/source ?s]
          [?l :link/destination ?d]
          [?d :bp/uuid ?destination-uuid]]",
        "",
        &[],
        &[("?gv-uuid", vec![s("gv-fuel-load"), s("gv-moisture")])],
    );
    // gv-fuel-load (40) has link to gv-moisture (41)
    assert_eq!(result.len(), 1);
    assert_eq!(result[0][0], s("gv-fuel-load"));
    assert_eq!(result[0][1], s("gv-moisture"));
}

/// vms/subs.cljs:151-158 — native units via rules
#[test]
fn vms_native_units_with_rules() {
    let db = vms_db();
    let result = run_q(
        &db,
        "[:find ?unit-uuid .
          :in $ % ?gv-uuid
          :where
          (lookup ?gv-uuid ?gv)
          [?v :variable/group-variables ?gv]
          [?v :variable/kind :continuous]
          [?v :variable/native-unit-uuid ?unit-uuid]]",
        BEHAVE_RULES,
        &[("?gv-uuid", s("gv-fuel-load"))],
    );
    assert_eq!(result.len(), 1);
    assert_eq!(result[0][0], s("unit-tons-acre"));
}

/// vms/subs.cljs:173-180 — translations
#[test]
fn vms_translations() {
    let db = vms_db();
    let result = run_q(
        &db,
        "[:find ?key ?translation
          :in $ ?short-code
          :where
          [?l :language/shortcode ?short-code]
          [?l :language/translation ?t]
          [?t :translation/key ?key]
          [?t :translation/translation ?translation]]",
        "",
        &[("?short-code", s("en"))],
    );
    assert_eq!(result.len(), 2);
    let mut pairs: Vec<(String, String)> = result.iter().map(|t| {
        (match &t[0] { Value::Str(s) => s.clone(), _ => panic!() },
         match &t[1] { Value::Str(s) => s.clone(), _ => panic!() })
    }).collect();
    pairs.sort();
    assert_eq!(pairs, vec![
        ("fuel_model".into(), "Fuel Model".into()),
        ("gv_fuel_load".into(), "Fuel Load".into()),
    ]);
}

/// vms/subs.cljs:229-234 — directional group variable uuids
#[test]
fn vms_directional_gv_uuids() {
    let db = vms_db();
    let result = run_q(
        &db,
        "[:find [?gv-uuid ...]
          :in $
          :where
          [?gv :bp/uuid ?gv-uuid]
          [?gv :group-variable/direction ?direction]]",
        "",
        &[],
    );
    assert_eq!(strs(&result, 0), vec!["gv-fuel-load"]);
}

/// vms/subs.cljs:284-291 — group variable hierarchy (recursive rules)
#[test]
fn vms_group_hierarchy_recursive_rules() {
    let db = vms_db();
    // Find all ancestors of group-1hr (32) via subgroup rule
    let result = run_q(
        &db,
        "[:find [?ancestor ...]
          :in $ % ?child
          :where
          (subgroup ?ancestor ?child)]",
        BEHAVE_RULES,
        &[("?child", n(32))],
    );
    // 32 is child of 31, which is child of 30
    let eids = longs(&result, 0);
    assert!(eids.contains(&30), "should find FuelModel (30)");
    assert!(eids.contains(&31), "should find Dead (31)");
}

/// vms/subs.cljs:217-224 — group-variable is output (rules + equality predicate)
#[test]
fn vms_gv_is_output() {
    let db = vms_db();
    // gv-42 (wind-speed) is on group 33, which is under submodule 21 (output)
    let result = run_q(
        &db,
        "[:find ?is-output .
          :in $ % ?gv
          :where
          [?g :group/group-variables ?gv]
          (submodule-root ?sm ?g)
          [?sm :submodule/io ?io]
          [(= ?io :output) ?is-output]]",
        BEHAVE_RULES,
        &[("?gv", n(42))],
    );
    assert_eq!(result.len(), 1);
    assert_eq!(result[0][0], Value::Bool(true));
}

/// wizard/subs.cljs:912-917 — variable-kind via rules
#[test]
fn vms_variable_kind_via_rules() {
    let db = vms_db();
    let result = run_q(
        &db,
        "[:find ?kind .
          :in $ % ?gv-uuid
          :where
          (variable-kind ?gv-uuid ?kind)]",
        BEHAVE_RULES,
        &[("?gv-uuid", s("gv-fuel-load"))],
    );
    assert_eq!(result.len(), 1);
    assert_eq!(result[0][0], kv("continuous"));
}

/// solver/queries.cljs:154-163 — module-diagrams with regex chain
#[test]
fn vms_module_diagrams_regex_chain() {
    let db = vms_db();
    // Simulates: find modules whose name matches "(?i)surface"
    let result = run_q(
        &db,
        "[:find ?d
          :in $ ?module-name
          :where
          [?m :module/name ?m-name]
          [(str \"(?i)\" ?module-name) ?module-find]
          [(re-pattern ?module-find) ?module-find-re]
          [(re-find ?module-find-re ?m-name)]
          [?m :module/diagrams ?d]]",
        "",
        &[("?module-name", s("surface"))],
    );
    // "(?i)surface" should match "Surface"
    assert_eq!(result.len(), 1);
    assert_eq!(result[0][0], n(80));
}

/// settings/subs.cljs:58-71 — domain info with multiple get-else calls
#[test]
fn vms_domain_info_multiple_get_else() {
    let db = vms_db();
    let result = run_q(
        &db,
        "[:find ?ds-name ?d-name ?d-uuid ?dim-uuid ?native ?decimals ?english ?metric
          :where
          [?ds :domain-set/name ?ds-name]
          [?ds :domain-set/domains ?d]
          [?d :domain/name ?d-name]
          [?d :bp/uuid ?d-uuid]
          [(get-else $ ?d :domain/dimension-uuid \"N/A\") ?dim-uuid]
          [?d :domain/native-unit-uuid ?native]
          [(get-else $ ?d :domain/english-unit-uuid \"N/A\") ?english]
          [(get-else $ ?d :domain/metric-unit-uuid \"N/A\") ?metric]
          [(get-else $ ?d :domain/decimals \"N/A\") ?decimals]]",
        "",
        &[],
    );
    assert_eq!(result.len(), 1);
    let row = &result[0];
    assert_eq!(row[0], s("Fire"));           // domain-set name
    assert_eq!(row[1], s("Length"));          // domain name
    assert_eq!(row[2], s("domain-length"));   // bp/uuid
    assert_eq!(row[3], s("dim-length"));      // dimension-uuid
    assert_eq!(row[4], s("unit-feet"));        // native-unit-uuid
    assert_eq!(row[5], n(2));                 // decimals
    assert_eq!(row[6], s("unit-feet"));        // english
    assert_eq!(row[7], s("unit-meters"));      // metric
}

/// wizard/subs.cljs:784-795 — conditionally-set group variables with rules
#[test]
fn vms_conditionally_set_gvs_with_rules() {
    let db = vms_db();
    let result = run_q(
        &db,
        "[:find [?gv ...]
          :in $ % ?module-eid ?io
          :where
          [?module-eid :module/submodules ?s]
          [?s :submodule/io ?io]
          (group ?s ?g)
          [?g :group/group-variables ?gv]
          [?gv :group-variable/conditionally-set? true]]",
        BEHAVE_RULES,
        &[("?module-eid", n(10)), ("?io", kv("output"))],
    );
    // Only gv-42 (wind-speed) is conditionally-set on output submodule 21
    assert_eq!(longs(&result, 0), vec![42]);
}

/// results/inputs/subs.cljs:45-52 — count aggregate with rules
#[test]
fn vms_count_conditionally_set_with_rules() {
    let db = vms_db();
    let mut q = parse_query(
        "[:find (count ?gv) .
          :in $ % ?s-uuid
          :where
          [?s :bp/uuid ?s-uuid]
          (group ?s ?g)
          (group-variable ?g ?gv ?v)
          [?gv :group-variable/conditionally-set? true]]",
    );
    let rules = parse_rules(BEHAVE_RULES);
    bind_inputs(&mut q, &[("?s-uuid", s("submod-fire"))]);
    let result = resolve_query(&db, &q.where_clauses, &rules);
    let projected = project(&result, &q.find.vars());
    // Should find gv-42 (wind-speed) which is conditionally-set
    // count should be 1
    let agg = persistent_sorted_set::aggregates::aggregate(
        &q.find_elements, projected.tuples,
    );
    assert_eq!(agg.len(), 1);
    assert_eq!(agg[0][0], Value::Long(1));
}

// =========================================================================
// Single-DB Worksheet queries (worksheet/events.cljs, worksheet/subs.cljs)
// =========================================================================

/// worksheet/subs.cljs:39-44 — all worksheets
#[test]
fn ws_all_worksheets() {
    let db = ws_db();
    let result = run_q(
        &db,
        "[:find ?created ?uuid
          :in $
          :where
          [?e :worksheet/uuid ?uuid]
          [?e :worksheet/created ?created]]",
        "",
        &[],
    );
    assert_eq!(result.len(), 1);
    assert_eq!(result[0][1], s("ws-1"));
}

/// worksheet/events.cljs:42-51 — input value lookup
#[test]
fn ws_input_value_lookup() {
    let db = ws_db();
    let result = run_q(
        &db,
        "[:find ?value .
          :in $ ?ws-uuid ?group-uuid ?repeat-id
          :where
          [?ws :worksheet/uuid ?ws-uuid]
          [?ws :worksheet/input-groups ?ig]
          [?ig :input-group/group-uuid ?group-uuid]
          [?ig :input-group/repeat-id ?repeat-id]
          [?ig :input-group/inputs ?i]
          [?i :input/value ?value]]",
        "",
        &[
            ("?ws-uuid", s("ws-1")),
            ("?group-uuid", s("group-fuel")),
            ("?repeat-id", n(0)),
        ],
    );
    assert_eq!(result.len(), 1);
    assert_eq!(result[0][0], s("2.5"));
}

/// worksheet/subs.cljs:572-580 — all output uuids
#[test]
fn ws_all_output_uuids() {
    let db = ws_db();
    let result = run_q(
        &db,
        "[:find [?uuid ...]
          :in $ ?ws-uuid
          :where
          [?w :worksheet/uuid ?ws-uuid]
          [?w :worksheet/outputs ?o]
          [?o :output/group-variable-uuid ?uuid]
          [?o :output/enabled? true]]",
        "",
        &[("?ws-uuid", s("ws-1"))],
    );
    // gv-fuel-load and gv-moisture are enabled, gv-wind-speed is not
    assert_eq!(strs(&result, 0), vec!["gv-fuel-load", "gv-moisture"]);
}

/// worksheet/subs.cljs:992-999 — repeat ids for input group
#[test]
fn ws_repeat_ids_for_group() {
    let db = ws_db();
    let result = run_q(
        &db,
        "[:find [?rid ...]
          :in $ ?ws-uuid ?group-uuid
          :where
          [?w :worksheet/uuid ?ws-uuid]
          [?w :worksheet/input-groups ?ig]
          [?ig :input-group/group-uuid ?group-uuid]
          [?ig :input-group/repeat-id ?rid]]",
        "",
        &[
            ("?ws-uuid", s("ws-1")),
            ("?group-uuid", s("group-fuel")),
        ],
    );
    assert_eq!(longs(&result, 0), vec![0, 1]);
}

// =========================================================================
// Multi-source queries (worksheet/subs.cljs — q-vms pattern)
// =========================================================================

/// worksheet/subs.cljs:498-510 — output uuids with get-else on VMS, multi-source
/// This is one of the most complex real behave patterns: multi-source + rules + get-else.
#[test]
fn multi_output_uuids_with_get_else() {
    let vms = vms_db();
    let ws = ws_db();
    let mut multi = MultiResolver::new(&vms);
    multi.add_source("$ws".to_string(), &ws);

    let result = run_q_multi(
        &multi,
        "[:find ?gv ?hide-result
          :in $ $ws %  ?ws-uuid
          :where
          [$ws ?w :worksheet/uuid ?ws-uuid]
          [$ws ?w :worksheet/outputs ?o]
          [$ws ?o :output/group-variable-uuid ?uuid]
          [$ws ?o :output/enabled? true]
          (lookup ?uuid ?gv)
          [(get-else $ ?gv :group-variable/hide-result? false) ?hide-result]]",
        BEHAVE_RULES,
        &[("?ws-uuid", s("ws-1"))],
        &[],
    );

    // Two enabled outputs: gv-fuel-load (40) and gv-moisture (41)
    // gv-fuel-load has no hide-result → default false
    // gv-moisture has hide-result? true
    assert_eq!(result.len(), 2);

    let fuel = result.iter().find(|r| r[0] == n(40)).unwrap();
    assert_eq!(fuel[1], Value::Bool(false));

    let moisture = result.iter().find(|r| r[0] == n(41)).unwrap();
    assert_eq!(moisture[1], Value::Bool(true));
}

/// worksheet/subs.cljs:545-558 — graphed output with double get-else, multi-source
#[test]
fn multi_graphed_outputs_double_get_else() {
    let vms = vms_db();
    let ws = ws_db();
    let mut multi = MultiResolver::new(&vms);
    multi.add_source("$ws".to_string(), &ws);

    let result = run_q_multi(
        &multi,
        "[:find ?uuid ?hide-result ?graph-result
          :in $ $ws % ?ws-uuid
          :where
          [$ws ?w :worksheet/uuid ?ws-uuid]
          [$ws ?w :worksheet/outputs ?o]
          [$ws ?o :output/group-variable-uuid ?uuid]
          [$ws ?o :output/enabled? true]
          (lookup ?uuid ?gv)
          [(get-else $ ?gv :group-variable/hide-result? false) ?hide-result]
          [(get-else $ ?gv :group-variable/hide-graph? false) ?graph-result]]",
        BEHAVE_RULES,
        &[("?ws-uuid", s("ws-1"))],
        &[],
    );

    assert_eq!(result.len(), 2);

    let fuel = result.iter().find(|r| r[0] == s("gv-fuel-load")).unwrap();
    assert_eq!(fuel[1], Value::Bool(false)); // no hide-result
    assert_eq!(fuel[2], Value::Bool(false)); // no hide-graph

    let moisture = result.iter().find(|r| r[0] == s("gv-moisture")).unwrap();
    assert_eq!(moisture[1], Value::Bool(true));  // has hide-result
    assert_eq!(moisture[2], Value::Bool(true));  // has hide-graph
}

/// worksheet/subs.cljs:290-303 — all variable level units, multi-source + rules
/// Tests the full VMS↔Worksheet cross-DB join with intermediate rule resolution.
#[test]
fn multi_variable_level_units() {
    let vms = vms_db();
    let ws = ws_db();
    let mut multi = MultiResolver::new(&vms);
    multi.add_source("$ws".to_string(), &ws);

    let result = run_q_multi(
        &multi,
        "[:find ?group-uuid ?repeat-id ?gv-uuid
          :in $ $ws % ?ws-uuid
          :where
          [$ws ?w :worksheet/uuid ?ws-uuid]
          [$ws ?w :worksheet/input-groups ?g]
          [$ws ?g :input-group/group-uuid ?group-uuid]
          [$ws ?g :input-group/repeat-id ?repeat-id]
          [$ws ?g :input-group/inputs ?i]
          [$ws ?i :input/group-variable-uuid ?gv-uuid]
          (lookup ?gv-uuid ?gv)
          (group-variable _ ?gv ?v)
          [?v :variable/kind :continuous]]",
        BEHAVE_RULES,
        &[("?ws-uuid", s("ws-1"))],
        &[],
    );

    // Both input groups reference gv-fuel-load, which is continuous
    assert_eq!(result.len(), 2);
    let mut repeats: Vec<i64> = result.iter().filter_map(|r| match &r[1] {
        Value::Long(n) => Some(*n), _ => None,
    }).collect();
    repeats.sort();
    assert_eq!(repeats, vec![0, 1]);
    assert!(result.iter().all(|r| r[2] == s("gv-fuel-load")));
}

/// worksheet/subs.cljs:950-961 — csv export headers, multi-source + get-else
#[test]
fn multi_csv_export_headers() {
    let vms = vms_db();
    let ws = ws_db();
    let mut multi = MultiResolver::new(&vms);
    multi.add_source("$ws".to_string(), &ws);

    let result = run_q_multi(
        &multi,
        "[:find ?gv-uuid ?repeat-id ?units ?hide-csv
          :in $ $ws % ?ws-uuid
          :where
          [$ws ?w :worksheet/uuid ?ws-uuid]
          [$ws ?w :worksheet/result-table ?r]
          [$ws ?r :result-table/headers ?h]
          [$ws ?h :result-header/repeat-id ?repeat-id]
          [$ws ?h :result-header/group-variable-uuid ?gv-uuid]
          [$ws ?h :result-header/units ?units]
          (lookup ?gv-uuid ?gv)
          [(get-else $ ?gv :group-variable/hide-csv? false) ?hide-csv]]",
        BEHAVE_RULES,
        &[("?ws-uuid", s("ws-1"))],
        &[],
    );

    assert_eq!(result.len(), 2);
    // Both headers should have hide-csv false (no attr set)
    for row in &result {
        assert_eq!(row[3], Value::Bool(false));
    }
    let gv_uuids = strs(&result, 0);
    assert!(gv_uuids.contains(&"gv-fuel-load".to_string()));
    assert!(gv_uuids.contains(&"gv-moisture".to_string()));
}

// =========================================================================
// Complex query patterns (print/subs.cljs, wizard/subs.cljs)
// =========================================================================

/// print/subs.cljs:60-88 — result table cell lookup with collection bindings
#[test]
fn ws_result_table_cell_with_collections() {
    let db = ws_db();
    let result = run_q_coll(
        &db,
        "[:find ?i ?value
          :in $ ?ws-uuid ?row-gv-uuid [?i ...] ?output-gv-uuid
          :where
          [?w :worksheet/uuid ?ws-uuid]
          [?w :worksheet/result-table ?rt]
          [?rt :result-table/rows ?r]
          [?r :result-row/cells ?c1]
          [?c1 :result-cell/header ?h1]
          [?h1 :result-header/group-variable-uuid ?row-gv-uuid]
          [?c1 :result-cell/value ?i]
          [?r :result-row/cells ?c2]
          [?c2 :result-cell/header ?h2]
          [?h2 :result-header/group-variable-uuid ?output-gv-uuid]
          [?c2 :result-cell/value ?value]]",
        "",
        &[
            ("?ws-uuid", s("ws-1")),
            ("?row-gv-uuid", s("gv-fuel-load")),
            ("?output-gv-uuid", s("gv-moisture")),
        ],
        &[("?i", vec![s("2.5")])],
    );
    assert_eq!(result.len(), 1);
    assert_eq!(result[0][0], s("2.5"));   // row index value
    assert_eq!(result[0][1], s("12.0"));  // output value
}

/// Worksheet query: find all modules for a worksheet
/// solver/queries.cljs:254-260
#[test]
fn ws_worksheet_modules() {
    let db = ws_db();
    let result = run_q(
        &db,
        "[:find [?modules ...]
          :in $ ?ws-uuid
          :where
          [?w :worksheet/uuid ?ws-uuid]
          [?w :worksheet/modules ?modules]]",
        "",
        &[("?ws-uuid", s("ws-1"))],
    );
    assert_eq!(strs(&result, 0), vec!["Surface"]);
}

// =========================================================================
// Edge cases and defensive tests
// =========================================================================

/// Empty collection input should return zero results (not crash)
#[test]
fn empty_collection_returns_zero_results() {
    let db = vms_db();
    let result = run_q_coll(
        &db,
        "[:find ?gv-uuid ?dest-uuid
          :in $ [?gv-uuid ...]
          :where
          [?s :bp/uuid ?gv-uuid]
          [?l :link/source ?s]
          [?l :link/destination ?d]
          [?d :bp/uuid ?dest-uuid]]",
        "",
        &[],
        &[("?gv-uuid", vec![])],
    );
    assert_eq!(result.len(), 0);
}

/// Multiple collection bindings in same query
#[test]
fn multiple_collection_bindings() {
    let db = ws_db();
    let result = run_q_coll(
        &db,
        "[:find ?gv-uuid ?rid
          :in $ ?ws-uuid [?gv-uuid ...] [?rid ...]
          :where
          [?w :worksheet/uuid ?ws-uuid]
          [?w :worksheet/input-groups ?ig]
          [?ig :input-group/repeat-id ?rid]
          [?ig :input-group/inputs ?i]
          [?i :input/group-variable-uuid ?gv-uuid]]",
        "",
        &[("?ws-uuid", s("ws-1"))],
        &[
            ("?gv-uuid", vec![s("gv-fuel-load")]),
            ("?rid", vec![n(0), n(1)]),
        ],
    );
    // Two input groups, both with gv-fuel-load, repeats 0 and 1
    assert_eq!(result.len(), 2);
}

/// Query with no :in clause (implicit $) should work
#[test]
fn implicit_default_source() {
    let db = vms_db();
    let result = run_q(
        &db,
        "[:find [?gv-uuid ...]
          :where
          [?v :variable/group-variables ?gv]
          [?gv :bp/uuid ?gv-uuid]
          [?v :variable/kind :continuous]]",
        "",
        &[],
    );
    // fuelLoad and windSpeed are continuous
    let uuids = strs(&result, 0);
    assert!(uuids.contains(&"gv-fuel-load".to_string()));
    assert!(uuids.contains(&"gv-wind-speed".to_string()));
    assert!(!uuids.contains(&"gv-moisture".to_string())); // discrete
}

/// get-else with string default "N/A" (settings/subs.cljs pattern)
#[test]
fn get_else_with_string_default() {
    let db = vms_db();
    // gv-fuel-load (40) has no :group-variable/hide-csv? → should get "N/A" string default
    let result = run_q(
        &db,
        "[:find ?uuid ?val
          :where
          [?gv :bp/uuid ?uuid]
          [?gv :group-variable/translation-key _]
          [(get-else $ ?gv :group-variable/hide-csv? \"N/A\") ?val]]",
        "",
        &[],
    );
    // All 3 group-variables should return "N/A" since none has hide-csv?
    assert_eq!(result.len(), 3);
    for row in &result {
        assert_eq!(row[1], s("N/A"));
    }
}

/// Multi-source: query patterns on $ws with no matching data
/// Ensures empty results, no panics.
#[test]
fn multi_source_no_matching_ws_data() {
    let vms = vms_db();
    let ws = ws_db();
    let mut multi = MultiResolver::new(&vms);
    multi.add_source("$ws".to_string(), &ws);

    let result = run_q_multi(
        &multi,
        "[:find ?uuid
          :in $ $ws % ?ws-uuid
          :where
          [$ws ?w :worksheet/uuid ?ws-uuid]
          [$ws ?w :worksheet/outputs ?o]
          [$ws ?o :output/group-variable-uuid ?uuid]
          [$ws ?o :output/enabled? true]
          (lookup ?uuid ?gv)
          [?gv :group-variable/conditionally-set? true]]",
        BEHAVE_RULES,
        &[("?ws-uuid", s("ws-1"))],
        &[],
    );
    // gv-wind-speed is conditionally-set but output is disabled
    // gv-fuel-load and gv-moisture are enabled but not conditionally-set
    assert_eq!(result.len(), 0);
}

/// Multi-source with rules + predicate filter
#[test]
fn multi_source_rules_and_predicate() {
    let vms = vms_db();
    let ws = ws_db();
    let mut multi = MultiResolver::new(&vms);
    multi.add_source("$ws".to_string(), &ws);

    let result = run_q_multi(
        &multi,
        "[:find ?gv-uuid ?kind
          :in $ $ws % ?ws-uuid
          :where
          [$ws ?w :worksheet/uuid ?ws-uuid]
          [$ws ?w :worksheet/input-groups ?ig]
          [$ws ?ig :input-group/inputs ?i]
          [$ws ?i :input/group-variable-uuid ?gv-uuid]
          (variable-kind ?gv-uuid ?kind)
          [(= ?kind :continuous)]]",
        BEHAVE_RULES,
        &[("?ws-uuid", s("ws-1"))],
        &[],
    );
    // Only gv-fuel-load is in the worksheet inputs and is continuous
    assert!(result.len() >= 1);
    for row in &result {
        assert_eq!(row[0], s("gv-fuel-load"));
        assert_eq!(row[1], kv("continuous"));
    }
}
