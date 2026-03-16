//! Parser integration tests.
//!
//! Verifies that EDN Datalog queries are correctly parsed and produce the
//! expected results when resolved against a DataScriptDB. Tests are modeled
//! on the production queries and rules from `behave_schema/rules.cljc`.

use persistent_sorted_set::datom::{Datom, Value};
use persistent_sorted_set::db::{DataScriptDB, TX0};
use persistent_sorted_set::query_parser::{
    bind_inputs, parse_query, parse_rules, FindSpec,
};
use persistent_sorted_set::relation::{
    project, resolve_query, Clause, PatternEl, Relation, Rules,
};
use persistent_sorted_set::schema::{
    kw, kw_ns, AttrSchema, Cardinality, Schema, Unique, ValueType,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn d(e: i64, attr: &str, v: Value, tx: i64) -> Datom {
    Datom::new(e, Some(kw(attr)), v, TX0 + tx)
}

fn d_ns(e: i64, ns: &str, name: &str, v: Value, tx: i64) -> Datom {
    Datom::new(e, Some(kw_ns(ns, name)), v, TX0 + tx)
}

/// Run a query with input bindings, return projected result tuples.
fn run_query_with_inputs(
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

/// Extract Long values from column 0.
fn col_longs(tuples: &[Vec<Value>]) -> Vec<i64> {
    let mut v: Vec<i64> = tuples
        .iter()
        .filter_map(|t| match &t[0] {
            Value::Long(n) => Some(*n),
            _ => None,
        })
        .collect();
    v.sort();
    v
}

/// Extract String values from a named column in a Relation.
fn col_strings(rel: &Relation, var: &str) -> Vec<String> {
    let idx = rel.attrs[var];
    let mut v: Vec<String> = rel
        .tuples
        .iter()
        .filter_map(|t| match &t[idx] {
            Value::Str(s) => Some(s.clone()),
            _ => None,
        })
        .collect();
    v.sort();
    v
}

// ---------------------------------------------------------------------------
// Test database: mimics the BehavePlus VMS structure
// ---------------------------------------------------------------------------

fn behave_schema() -> Schema {
    let mut s = Schema::default();
    let ref_many = AttrSchema {
        value_type: Some(ValueType::Ref),
        cardinality: Cardinality::Many,
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

    // variable
    s.attrs.insert(kw_ns("variable", "name"), indexed.clone());
    s.attrs.insert(kw_ns("variable", "group-variables"), ref_many.clone());
    s.attrs.insert(kw_ns("variable", "kind"), indexed.clone());

    // language
    s.attrs.insert(kw_ns("language", "shortcode"), uuid_attr.clone());
    s.attrs.insert(kw_ns("language", "name"), indexed.clone());

    s
}

/// Build a VMS-like hierarchy:
///   app(1) → mod(10) → submod(20) → group(30) → subgroup(31) → subsubgroup(32)
///                                  → group(33)
///   group(30) has group-variable(40), group(33) has group-variable(41)
///   variable(50) ← gv(40), variable(51) ← gv(41)
fn behave_db() -> DataScriptDB {
    let mut db = DataScriptDB::empty(behave_schema());
    db.with_datoms(vec![
        // Application
        d_ns(1, "application", "name", Value::Str("BehavePlus".into()), 1),
        d_ns(1, "application", "modules", Value::Ref(10), 1),
        d_ns(1, "bp", "uuid", Value::Str("app-uuid".into()), 1),

        // Module
        d_ns(10, "module", "name", Value::Str("Surface".into()), 1),
        d_ns(10, "module", "submodules", Value::Ref(20), 1),
        d_ns(10, "bp", "uuid", Value::Str("mod-uuid".into()), 1),

        // Submodule
        d_ns(20, "submodule", "name", Value::Str("Weighted".into()), 1),
        d_ns(20, "submodule", "groups", Value::Ref(30), 1),
        d_ns(20, "submodule", "io", Value::Keyword(kw("input")), 1),
        d_ns(20, "bp", "uuid", Value::Str("submod-uuid".into()), 1),

        // Groups (hierarchy)
        d_ns(30, "group", "name", Value::Str("FuelModel".into()), 1),
        d_ns(30, "group", "children", Value::Ref(31), 1),
        d_ns(30, "group", "group-variables", Value::Ref(40), 1),
        d_ns(30, "group", "translation-key", Value::Str("fuel_model".into()), 1),
        d_ns(30, "bp", "uuid", Value::Str("group-uuid-30".into()), 1),

        d_ns(31, "group", "name", Value::Str("Dead".into()), 1),
        d_ns(31, "group", "children", Value::Ref(32), 1),
        d_ns(31, "bp", "uuid", Value::Str("group-uuid-31".into()), 1),

        d_ns(32, "group", "name", Value::Str("1hr".into()), 1),
        d_ns(32, "bp", "uuid", Value::Str("group-uuid-32".into()), 1),

        d_ns(33, "group", "name", Value::Str("Moisture".into()), 1),
        d_ns(33, "group", "group-variables", Value::Ref(41), 1),
        d_ns(33, "bp", "uuid", Value::Str("group-uuid-33".into()), 1),

        // Group-variables
        d_ns(40, "bp", "uuid", Value::Str("gv-uuid-40".into()), 1),
        d_ns(40, "group-variable", "translation-key", Value::Str("gv_fuel_load".into()), 1),

        d_ns(41, "bp", "uuid", Value::Str("gv-uuid-41".into()), 1),
        d_ns(41, "group-variable", "translation-key", Value::Str("gv_moisture".into()), 1),

        // Variables
        d_ns(50, "variable", "name", Value::Str("fuelLoad".into()), 1),
        d_ns(50, "variable", "group-variables", Value::Ref(40), 1),
        d_ns(50, "variable", "kind", Value::Keyword(kw("continuous")), 1),
        d_ns(50, "bp", "uuid", Value::Str("var-uuid-50".into()), 1),

        d_ns(51, "variable", "name", Value::Str("moistureContent".into()), 1),
        d_ns(51, "variable", "group-variables", Value::Ref(41), 1),
        d_ns(51, "variable", "kind", Value::Keyword(kw("discrete")), 1),
        d_ns(51, "bp", "uuid", Value::Str("var-uuid-51".into()), 1),

        // Languages
        d_ns(60, "language", "shortcode", Value::Str("en".into()), 1),
        d_ns(60, "language", "name", Value::Str("English".into()), 1),
        d_ns(61, "language", "shortcode", Value::Str("es".into()), 1),
        d_ns(61, "language", "name", Value::Str("Spanish".into()), 1),
    ]);
    db
}

/// Production-like rules from `behave_schema/rules.cljc`.
/// Note: EDN comments (`;`) are stripped because the `edn 0.3` crate does not support them.
const VMS_RULES: &str = r#"
[[(lookup ?uuid ?e) [?e :bp/uuid ?uuid]]

 [(subgroup ?g ?s) [?g :group/children ?s]]
 [(subgroup ?g ?s) [?g :group/children ?x] (subgroup ?x ?s)]

 [(group ?s ?g)  [?s :submodule/groups ?g]]
 [(group ?s ?g)  [?s :submodule/groups ?x] (subgroup ?x ?g)]

 [(group-variable ?g ?gv ?v)
  [?g :group/group-variables ?gv]
  [?v :variable/group-variables ?gv]]

 [(variable ?gv ?v)
  [?v :variable/group-variables ?gv]]

 [(app-root ?a ?g)
  [?sm :submodule/groups ?g]
  [?m :module/submodules ?sm]
  [?a :application/modules ?m]]

 [(app-root ?a ?s)
  (subgroup ?g ?s)
  [?sm :submodule/groups ?g]
  [?m :module/submodules ?sm]
  [?a :application/modules ?m]]

 [(submodule-root ?submodule ?subgroup)
  [?submodule :submodule/groups ?subgroup]]
 [(submodule-root ?submodule ?subgroup)
  (subgroup ?group ?subgroup)
  [?submodule :submodule/groups ?group]]

 [(translation-key ?e ?k) [?e :group/translation-key ?k]]
 [(translation-key ?e ?k) [?e :group-variable/translation-key ?k]]]
"#;

// ===================================================================
// Parser: find spec variants
// ===================================================================

#[test]
fn find_rel() {
    let q = parse_query("[:find ?a ?b ?c :where [?a :foo ?b] [?b :bar ?c]]");
    assert_eq!(q.find, FindSpec::Rel(vec!["?a".into(), "?b".into(), "?c".into()]));
}

#[test]
fn find_scalar() {
    let q = parse_query("[:find ?name . :where [_ :name ?name]]");
    assert_eq!(q.find, FindSpec::Scalar("?name".into()));
    assert_eq!(q.find.vars(), vec!["?name"]);
}

#[test]
fn find_coll() {
    let q = parse_query("[:find [?e ...] :where [?e :type :person]]");
    assert_eq!(q.find, FindSpec::Coll("?e".into()));
}

#[test]
fn find_tuple() {
    let q = parse_query("[:find [?name ?age] :where [1 :name ?name] [1 :age ?age]]");
    assert_eq!(q.find, FindSpec::Tuple(vec!["?name".into(), "?age".into()]));
}

// ===================================================================
// Parser: :in clause
// ===================================================================

#[test]
fn in_db_only() {
    let q = parse_query("[:find ?e :in $ :where [?e :name _]]");
    assert!(q.in_vars.is_empty());
}

#[test]
fn in_db_and_rules() {
    let q = parse_query("[:find ?e :in $ % :where (some-rule ?e)]");
    assert!(q.in_vars.is_empty());
}

#[test]
fn in_with_params() {
    let q = parse_query("[:find ?e :in $ % ?uuid ?min-age :where [?e :bp/uuid ?uuid]]");
    assert_eq!(q.in_vars, vec!["?uuid", "?min-age"]);
}

// ===================================================================
// Parser: clause types
// ===================================================================

#[test]
fn clause_pattern_three_elements() {
    let q = parse_query("[:find ?e :where [?e :name \"Alice\"]]");
    assert_eq!(q.where_clauses.len(), 1);
    match &q.where_clauses[0] {
        Clause::Pattern(p) => {
            assert_eq!(p[0], PatternEl::Var("?e".into()));
            assert!(matches!(&p[1], PatternEl::Const(Value::Keyword(_))));
            assert_eq!(p[2], PatternEl::Const(Value::Str("Alice".into())));
            assert_eq!(p[3], PatternEl::Blank);
        }
        other => panic!("expected Pattern, got {:?}", other),
    }
}

#[test]
fn clause_pattern_four_elements() {
    let q = parse_query("[:find ?e ?tx :where [?e :name _ ?tx]]");
    match &q.where_clauses[0] {
        Clause::Pattern(p) => {
            assert_eq!(p[3], PatternEl::Var("?tx".into()));
        }
        _ => panic!("expected Pattern"),
    }
}

#[test]
fn clause_rule_call() {
    let q = parse_query("[:find ?s :in $ % :where (subgroup ?g ?s)]");
    match &q.where_clauses[0] {
        Clause::RuleCall { name, args } => {
            assert_eq!(name, "subgroup");
            assert_eq!(args[0], PatternEl::Var("?g".into()));
            assert_eq!(args[1], PatternEl::Var("?s".into()));
        }
        other => panic!("expected RuleCall, got {:?}", other),
    }
}

#[test]
fn clause_rule_call_with_constant() {
    let q = parse_query("[:find ?s :in $ % :where (lookup \"some-uuid\" ?e)]");
    match &q.where_clauses[0] {
        Clause::RuleCall { name, args } => {
            assert_eq!(name, "lookup");
            assert_eq!(args[0], PatternEl::Const(Value::Str("some-uuid".into())));
            assert_eq!(args[1], PatternEl::Var("?e".into()));
        }
        other => panic!("expected RuleCall, got {:?}", other),
    }
}

#[test]
fn clause_predicate_gt() {
    let q = parse_query("[:find ?e :where [?e :age ?a] [(> ?a 21)]]");
    match &q.where_clauses[1] {
        Clause::Predicate { name, args } => {
            assert_eq!(name, ">");
            assert_eq!(args[0], PatternEl::Var("?a".into()));
            assert_eq!(args[1], PatternEl::Const(Value::Long(21)));
        }
        other => panic!("expected Predicate, got {:?}", other),
    }
}

#[test]
fn clause_predicate_number_check() {
    let q = parse_query("[:find ?v :where [?v :val ?x] [(number? ?x)]]");
    match &q.where_clauses[1] {
        Clause::Predicate { name, args } => {
            assert_eq!(name, "number?");
            assert_eq!(args.len(), 1);
        }
        other => panic!("expected Predicate, got {:?}", other),
    }
}

#[test]
fn clause_not() {
    let q = parse_query("[:find ?e :where [?e :name _] (not [?e :deleted true])]");
    assert_eq!(q.where_clauses.len(), 2);
    match &q.where_clauses[1] {
        Clause::Not(inner) => {
            assert_eq!(inner.len(), 1);
            assert!(matches!(&inner[0], Clause::Pattern(_)));
        }
        other => panic!("expected Not, got {:?}", other),
    }
}

#[test]
fn clause_or() {
    let q = parse_query(
        "[:find ?e :where (or [?e :type :group] [?e :type :subgroup])]",
    );
    match &q.where_clauses[0] {
        Clause::Or(branches) => {
            assert_eq!(branches.len(), 2);
            assert!(matches!(&branches[0][0], Clause::Pattern(_)));
            assert!(matches!(&branches[1][0], Clause::Pattern(_)));
        }
        other => panic!("expected Or, got {:?}", other),
    }
}

#[test]
fn clause_and_inside_or() {
    let q = parse_query(
        "[:find ?e :where (or (and [?e :type :a] [?e :active true]) [?e :type :b])]",
    );
    match &q.where_clauses[0] {
        Clause::Or(branches) => {
            assert_eq!(branches.len(), 2);
            // First branch has 2 clauses from the (and ...)
            assert_eq!(branches[0].len(), 2);
            // Second branch has 1 clause
            assert_eq!(branches[1].len(), 1);
        }
        other => panic!("expected Or, got {:?}", other),
    }
}

// ===================================================================
// Parser: value types
// ===================================================================

#[test]
fn value_string() {
    let q = parse_query(r#"[:find ?e :where [?e :name "Alice"]]"#);
    match &q.where_clauses[0] {
        Clause::Pattern(p) => assert_eq!(p[2], PatternEl::Const(Value::Str("Alice".into()))),
        _ => panic!("expected Pattern"),
    }
}

#[test]
fn value_integer() {
    let q = parse_query("[:find ?e :where [?e :age 42]]");
    match &q.where_clauses[0] {
        Clause::Pattern(p) => assert_eq!(p[2], PatternEl::Const(Value::Long(42))),
        _ => panic!("expected Pattern"),
    }
}

#[test]
fn value_negative_integer() {
    let q = parse_query("[:find ?e :where [?e :offset -5]]");
    match &q.where_clauses[0] {
        Clause::Pattern(p) => assert_eq!(p[2], PatternEl::Const(Value::Long(-5))),
        _ => panic!("expected Pattern"),
    }
}

#[test]
fn value_boolean_true() {
    let q = parse_query("[:find ?e :where [?e :active true]]");
    match &q.where_clauses[0] {
        Clause::Pattern(p) => assert_eq!(p[2], PatternEl::Const(Value::Bool(true))),
        _ => panic!("expected Pattern"),
    }
}

#[test]
fn value_boolean_false() {
    let q = parse_query("[:find ?e :where [?e :active false]]");
    match &q.where_clauses[0] {
        Clause::Pattern(p) => assert_eq!(p[2], PatternEl::Const(Value::Bool(false))),
        _ => panic!("expected Pattern"),
    }
}

#[test]
fn value_keyword_simple() {
    let q = parse_query("[:find ?e :where [?e :type :worksheet]]");
    match &q.where_clauses[0] {
        Clause::Pattern(p) => {
            assert_eq!(p[2], PatternEl::Const(Value::Keyword(kw("worksheet"))));
        }
        _ => panic!("expected Pattern"),
    }
}

#[test]
fn value_keyword_namespaced() {
    let q = parse_query("[:find ?e :where [?e :variable/kind :continuous]]");
    match &q.where_clauses[0] {
        Clause::Pattern(p) => {
            assert_eq!(
                p[1],
                PatternEl::Const(Value::Keyword(kw_ns("variable", "kind")))
            );
            assert_eq!(
                p[2],
                PatternEl::Const(Value::Keyword(kw("continuous")))
            );
        }
        _ => panic!("expected Pattern"),
    }
}

#[test]
fn value_wildcard() {
    let q = parse_query("[:find ?e :where [?e :name _]]");
    match &q.where_clauses[0] {
        Clause::Pattern(p) => assert_eq!(p[2], PatternEl::Blank),
        _ => panic!("expected Pattern"),
    }
}

// ===================================================================
// Parser: rules
// ===================================================================

#[test]
fn rules_single_clause_body() {
    let rules = parse_rules("[[(lookup ?uuid ?e) [?e :bp/uuid ?uuid]]]");
    assert_eq!(rules.len(), 1);
    let b = &rules["lookup"];
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].head_args, vec!["?uuid", "?e"]);
    assert_eq!(b[0].body.len(), 1);
}

#[test]
fn rules_multi_clause_body() {
    let rules = parse_rules(
        "[[(app-root ?a ?g)
           [?sm :submodule/groups ?g]
           [?m :module/submodules ?sm]
           [?a :application/modules ?m]]]",
    );
    assert_eq!(rules["app-root"][0].body.len(), 3);
}

#[test]
fn rules_recursive_two_branches() {
    let rules = parse_rules(
        "[[(subgroup ?g ?s) [?g :group/children ?s]]
          [(subgroup ?g ?s) [?g :group/children ?x] (subgroup ?x ?s)]]",
    );
    let b = &rules["subgroup"];
    assert_eq!(b.len(), 2);
    assert_eq!(b[0].body.len(), 1);
    assert_eq!(b[1].body.len(), 2);
    assert!(matches!(&b[1].body[1], Clause::RuleCall { name, .. } if name == "subgroup"));
}

#[test]
fn rules_multi_branch_or_semantics() {
    // translation-key has many branches (different attributes)
    let rules = parse_rules(
        "[[(translation-key ?e ?k) [?e :group/translation-key ?k]]
          [(translation-key ?e ?k) [?e :group-variable/translation-key ?k]]
          [(translation-key ?e ?k) [?e :module/translation-key ?k]]]",
    );
    assert_eq!(rules["translation-key"].len(), 3);
}

#[test]
fn rules_chained_rule_calls() {
    // io rule calls group-variable and group
    let rules = parse_rules(
        "[[(group-variable ?g ?gv ?v)
           [?g :group/group-variables ?gv]
           [?v :variable/group-variables ?gv]]
          [(io ?e ?io)
           (group-variable ?g ?e ?v)
           (group ?s ?g)
           [?s :submodule/io ?io]]]",
    );
    assert!(rules.contains_key("group-variable"));
    assert!(rules.contains_key("io"));
    let io_body = &rules["io"][0].body;
    assert_eq!(io_body.len(), 3);
    assert!(matches!(&io_body[0], Clause::RuleCall { name, .. } if name == "group-variable"));
    assert!(matches!(&io_body[1], Clause::RuleCall { name, .. } if name == "group"));
}

#[test]
fn rules_full_production_set_parses() {
    // Verify the full production-like rule set parses without errors
    let rules = parse_rules(VMS_RULES);
    assert!(rules.contains_key("lookup"));
    assert!(rules.contains_key("subgroup"));
    assert!(rules.contains_key("group"));
    assert!(rules.contains_key("group-variable"));
    assert!(rules.contains_key("variable"));
    assert!(rules.contains_key("app-root"));
    assert!(rules.contains_key("submodule-root"));
    assert!(rules.contains_key("translation-key"));
    assert_eq!(rules["subgroup"].len(), 2);
    assert_eq!(rules["app-root"].len(), 2);
    assert_eq!(rules["translation-key"].len(), 2);
}

// ===================================================================
// Parser: input binding
// ===================================================================

#[test]
fn bind_string_input() {
    let mut q = parse_query("[:find ?e :in $ ?uuid :where [?e :bp/uuid ?uuid]]");
    bind_inputs(&mut q, &[("?uuid", Value::Str("abc".into()))]);
    match &q.where_clauses[0] {
        Clause::Pattern(p) => {
            assert_eq!(p[2], PatternEl::Const(Value::Str("abc".into())));
        }
        _ => panic!("expected Pattern"),
    }
}

#[test]
fn bind_long_input() {
    let mut q = parse_query("[:find ?name :in $ ?eid :where [?eid :name ?name]]");
    bind_inputs(&mut q, &[("?eid", Value::Long(42))]);
    match &q.where_clauses[0] {
        Clause::Pattern(p) => {
            assert_eq!(p[0], PatternEl::Const(Value::Long(42)));
        }
        _ => panic!("expected Pattern"),
    }
}

#[test]
fn bind_input_in_rule_call() {
    let mut q = parse_query("[:find ?e :in $ % ?uuid :where (lookup ?uuid ?e)]");
    bind_inputs(&mut q, &[("?uuid", Value::Str("xyz".into()))]);
    match &q.where_clauses[0] {
        Clause::RuleCall { args, .. } => {
            assert_eq!(args[0], PatternEl::Const(Value::Str("xyz".into())));
        }
        _ => panic!("expected RuleCall"),
    }
}

#[test]
fn bind_input_in_predicate() {
    let mut q = parse_query("[:find ?e :in $ ?min :where [?e :age ?a] [(>= ?a ?min)]]");
    bind_inputs(&mut q, &[("?min", Value::Long(21))]);
    match &q.where_clauses[1] {
        Clause::Predicate { args, .. } => {
            assert_eq!(args[1], PatternEl::Const(Value::Long(21)));
        }
        _ => panic!("expected Predicate"),
    }
}

#[test]
fn bind_multiple_inputs() {
    let mut q = parse_query(
        "[:find ?e :in $ ?name ?age :where [?e :name ?name] [?e :age ?age]]",
    );
    bind_inputs(
        &mut q,
        &[
            ("?name", Value::Str("Alice".into())),
            ("?age", Value::Long(30)),
        ],
    );
    match &q.where_clauses[0] {
        Clause::Pattern(p) => assert_eq!(p[2], PatternEl::Const(Value::Str("Alice".into()))),
        _ => panic!("expected Pattern"),
    }
    match &q.where_clauses[1] {
        Clause::Pattern(p) => assert_eq!(p[2], PatternEl::Const(Value::Long(30))),
        _ => panic!("expected Pattern"),
    }
}

#[test]
fn bind_leaves_unbound_vars_alone() {
    let mut q = parse_query("[:find ?e ?name :in $ ?age :where [?e :name ?name] [?e :age ?age]]");
    bind_inputs(&mut q, &[("?age", Value::Long(30))]);
    // ?name should remain a Var
    match &q.where_clauses[0] {
        Clause::Pattern(p) => assert_eq!(p[2], PatternEl::Var("?name".into())),
        _ => panic!("expected Pattern"),
    }
    // ?age should be Const
    match &q.where_clauses[1] {
        Clause::Pattern(p) => assert_eq!(p[2], PatternEl::Const(Value::Long(30))),
        _ => panic!("expected Pattern"),
    }
}

// ===================================================================
// Integration: parse + resolve against DataScriptDB
// ===================================================================

#[test]
fn resolve_simple_pattern_query() {
    let db = behave_db();
    let tuples = run_query_with_inputs(
        &db,
        "[:find ?name :where [?e :application/name ?name]]",
        "",
        &[],
    );
    assert_eq!(tuples.len(), 1);
    assert_eq!(tuples[0][0], Value::Str("BehavePlus".into()));
}

#[test]
fn resolve_two_pattern_join() {
    let db = behave_db();
    let tuples = run_query_with_inputs(
        &db,
        "[:find ?mod-name :where [?a :application/modules ?m] [?m :module/name ?mod-name]]",
        "",
        &[],
    );
    assert_eq!(tuples.len(), 1);
    assert_eq!(tuples[0][0], Value::Str("Surface".into()));
}

#[test]
fn resolve_lookup_rule() {
    let db = behave_db();
    let tuples = run_query_with_inputs(
        &db,
        r#"[:find ?e :in $ % :where (lookup "mod-uuid" ?e)]"#,
        VMS_RULES,
        &[],
    );
    assert_eq!(col_longs(&tuples), vec![10]);
}

#[test]
fn resolve_lookup_rule_with_input_binding() {
    let db = behave_db();
    let tuples = run_query_with_inputs(
        &db,
        "[:find ?e :in $ % ?uuid :where (lookup ?uuid ?e)]",
        VMS_RULES,
        &[("?uuid", Value::Str("group-uuid-30".into()))],
    );
    assert_eq!(col_longs(&tuples), vec![30]);
}

#[test]
fn resolve_recursive_subgroup() {
    let db = behave_db();
    // group(30) → children: 31 → children: 32
    let tuples = run_query_with_inputs(
        &db,
        "[:find ?s :in $ % ?root :where (subgroup ?root ?s)]",
        VMS_RULES,
        &[("?root", Value::Long(30))],
    );
    let subs = col_longs(&tuples);
    assert_eq!(subs, vec![31, 32], "30 has children 31, and 31 has child 32");
}

#[test]
fn resolve_recursive_subgroup_with_names() {
    let db = behave_db();
    let result = {
        let mut q = parse_query(
            "[:find ?name :in $ % ?root :where (subgroup ?root ?s) [?s :group/name ?name]]",
        );
        let rules = parse_rules(VMS_RULES);
        bind_inputs(&mut q, &[("?root", Value::Long(30))]);
        let rel = resolve_query(&db, &q.where_clauses, &rules);
        project(&rel, &q.find.vars())
    };
    let mut names = col_strings(&result, "?name");
    names.sort();
    assert_eq!(names, vec!["1hr", "Dead"]);
}

#[test]
fn resolve_group_from_submodule() {
    let db = behave_db();
    // (group ?s ?g) finds all groups reachable from submodule 20:
    // direct: 30, via subgroup: 31, 32
    let tuples = run_query_with_inputs(
        &db,
        "[:find ?g :in $ % ?s :where (group ?s ?g)]",
        VMS_RULES,
        &[("?s", Value::Long(20))],
    );
    let groups = col_longs(&tuples);
    assert_eq!(groups, vec![30, 31, 32]);
}

#[test]
fn resolve_group_variable() {
    let db = behave_db();
    let tuples = run_query_with_inputs(
        &db,
        "[:find ?v-name
          :in $ % ?g
          :where
          (group-variable ?g ?gv ?v)
          [?v :variable/name ?v-name]]",
        VMS_RULES,
        &[("?g", Value::Long(30))],
    );
    assert_eq!(tuples.len(), 1);
    assert_eq!(tuples[0][0], Value::Str("fuelLoad".into()));
}

#[test]
fn resolve_app_root_direct() {
    let db = behave_db();

    // First verify the chain works step-by-step without rules
    let step1 = run_query_with_inputs(
        &db,
        "[:find ?sm :in $ ?g :where [?sm :submodule/groups ?g]]",
        "",
        &[("?g", Value::Long(30))],
    );
    assert_eq!(col_longs(&step1), vec![20], "step1: submodule for group 30");

    let step2 = run_query_with_inputs(
        &db,
        "[:find ?m :in $ ?sm :where [?m :module/submodules ?sm]]",
        "",
        &[("?sm", Value::Long(20))],
    );
    assert_eq!(col_longs(&step2), vec![10], "step2: module for submodule 20");

    let step3 = run_query_with_inputs(
        &db,
        "[:find ?a :in $ ?m :where [?a :application/modules ?m]]",
        "",
        &[("?m", Value::Long(10))],
    );
    assert_eq!(col_longs(&step3), vec![1], "step3: app for module 10");

    // Now with the rule
    let tuples = run_query_with_inputs(
        &db,
        "[:find ?a :in $ % ?g :where (app-root ?a ?g)]",
        VMS_RULES,
        &[("?g", Value::Long(30))],
    );
    assert_eq!(col_longs(&tuples), vec![1]);
}

#[test]
fn resolve_app_root_via_subgroup() {
    let db = behave_db();
    // app-root for subgroup 32 (transitive via subgroup rule)
    let tuples = run_query_with_inputs(
        &db,
        "[:find ?a :in $ % ?s :where (app-root ?a ?s)]",
        VMS_RULES,
        &[("?s", Value::Long(32))],
    );
    assert_eq!(col_longs(&tuples), vec![1]);
}

#[test]
fn resolve_submodule_root() {
    let db = behave_db();
    // submodule-root for subsubgroup 32
    let tuples = run_query_with_inputs(
        &db,
        "[:find ?sm :in $ % ?sg :where (submodule-root ?sm ?sg)]",
        VMS_RULES,
        &[("?sg", Value::Long(32))],
    );
    assert_eq!(col_longs(&tuples), vec![20]);
}

#[test]
fn resolve_translation_key_multi_branch() {
    let db = behave_db();
    // translation-key finds keys from either :group/translation-key or
    // :group-variable/translation-key
    let tuples = run_query_with_inputs(
        &db,
        "[:find ?k :in $ % ?e :where (translation-key ?e ?k)]",
        VMS_RULES,
        &[("?e", Value::Long(30))],
    );
    assert_eq!(tuples.len(), 1);
    assert_eq!(tuples[0][0], Value::Str("fuel_model".into()));

    let tuples2 = run_query_with_inputs(
        &db,
        "[:find ?k :in $ % ?e :where (translation-key ?e ?k)]",
        VMS_RULES,
        &[("?e", Value::Long(40))],
    );
    assert_eq!(tuples2.len(), 1);
    assert_eq!(tuples2[0][0], Value::Str("gv_fuel_load".into()));
}

#[test]
fn resolve_not_clause() {
    let db = behave_db();
    // Find groups that are NOT subgroups of group 30
    // group 30 has children 31, 32 — so groups that are not 31 or 32
    let result = {
        let mut q = parse_query(
            "[:find ?g :in $ % ?root
              :where
              [?g :group/name _]
              (not (subgroup ?root ?g))]",
        );
        let rules = parse_rules(VMS_RULES);
        bind_inputs(&mut q, &[("?root", Value::Long(30))]);
        let rel = resolve_query(&db, &q.where_clauses, &rules);
        project(&rel, &q.find.vars())
    };
    let groups = col_longs(&result.tuples);
    // 30 and 33 are not subgroups of 30
    assert!(groups.contains(&30));
    assert!(groups.contains(&33));
    assert!(!groups.contains(&31));
    assert!(!groups.contains(&32));
}

#[test]
fn resolve_predicate_filter() {
    // Simple DB for predicate test
    let mut schema = Schema::default();
    schema.attrs.insert(kw("score"), AttrSchema::default());
    schema.attrs.insert(kw("name"), AttrSchema { index: true, ..Default::default() });
    let mut db = DataScriptDB::empty(schema);
    for i in 1..=5 {
        db.with_datom(d(i, "name", Value::Str(format!("p{}", i)), 1));
        db.with_datom(d(i, "score", Value::Long(i * 10), 1));
    }

    let tuples = run_query_with_inputs(
        &db,
        "[:find ?name :where [?e :name ?name] [?e :score ?s] [(> ?s 30)]]",
        "",
        &[],
    );
    let mut names: Vec<String> = tuples
        .iter()
        .map(|t| match &t[0] {
            Value::Str(s) => s.clone(),
            _ => panic!("expected Str"),
        })
        .collect();
    names.sort();
    assert_eq!(names, vec!["p4", "p5"]);
}

#[test]
fn resolve_or_clause() {
    let db = behave_db();
    let result = {
        let q = parse_query(
            "[:find ?name :where
              (or [?e :module/name ?name]
                  [?e :submodule/name ?name])]",
        );
        let rel = resolve_query(&db, &q.where_clauses, &Rules::new());
        project(&rel, &q.find.vars())
    };
    let mut names = col_strings(&result, "?name");
    names.sort();
    assert_eq!(names, vec!["Surface", "Weighted"]);
}

#[test]
fn resolve_language_query() {
    // Matches: (language ?code ?l) from production rules
    let db = behave_db();
    let rules_str = r#"[[(language ?code ?l) [?l :language/shortcode ?code]]]"#;
    let tuples = run_query_with_inputs(
        &db,
        r#"[:find ?l :in $ % ?code :where (language ?code ?l)]"#,
        rules_str,
        &[("?code", Value::Str("en".into()))],
    );
    assert_eq!(col_longs(&tuples), vec![60]);
}

#[test]
fn resolve_no_results() {
    let db = behave_db();
    let tuples = run_query_with_inputs(
        &db,
        r#"[:find ?e :where [?e :application/name "Nonexistent"]]"#,
        "",
        &[],
    );
    assert!(tuples.is_empty());
}

#[test]
fn resolve_multiple_clauses_narrows_results() {
    let db = behave_db();
    // Only continuous variables
    let tuples = run_query_with_inputs(
        &db,
        "[:find ?name :where [?v :variable/name ?name] [?v :variable/kind :continuous]]",
        "",
        &[],
    );
    assert_eq!(tuples.len(), 1);
    assert_eq!(tuples[0][0], Value::Str("fuelLoad".into()));
}

// ---------------------------------------------------------------------------
// EDN comment stripping
// ---------------------------------------------------------------------------

#[test]
fn parse_query_with_line_comments() {
    let q = parse_query(
        ";; find all entities
         [:find ?e ;; the entity id
          :where
          [?e :name _]] ;; pattern clause",
    );
    assert_eq!(q.find.vars(), vec!["?e"]);
    assert_eq!(q.where_clauses.len(), 1);
}

#[test]
fn parse_rules_with_comments() {
    let rules = parse_rules(
        ";; recursive rule
         [[(ancestor ?a ?d)
           ;; base case
           [?a :parent ?d]]
          [(ancestor ?a ?d)
           [?a :parent ?x]
           ;; recurse
           (ancestor ?x ?d)]]",
    );
    assert!(rules.contains_key("ancestor"));
    assert_eq!(rules["ancestor"].len(), 2);
}

#[test]
fn comments_inside_strings_preserved() {
    let q = parse_query(
        "[:find ?e :where [?e :name \";; not a comment\"]]",
    );
    assert_eq!(q.where_clauses.len(), 1);
    if let Clause::Pattern(parts) = &q.where_clauses[0] {
        assert_eq!(parts[2], PatternEl::Const(Value::Str(";; not a comment".into())));
    } else {
        panic!("expected pattern clause");
    }
}
