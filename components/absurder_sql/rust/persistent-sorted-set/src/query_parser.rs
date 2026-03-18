//! EDN Datalog query parser.
//!
//! Parses DataScript query strings (EDN format) into the Rust clause/rule types
//! defined in `relation.rs`. Mirrors the CLJS `parser.cljc` for the subset of
//! query features supported by the Rust engine.
//!
//! ## Supported query format
//!
//! ```edn
//! [:find ?name ?age
//!  :in $ % ?param
//!  :where
//!  [?e :name ?name]
//!  [?e :age ?age]
//!  [(> ?age ?param)]
//!  (some-rule ?e ?x)]
//! ```
//!
//! ## Supported rule format
//!
//! ```edn
//! [[(rule-name ?a ?b) [?a :attr ?b]]
//!  [(rule-name ?a ?b) [?a :other ?x] (rule-name ?x ?b)]]
//! ```

use std::collections::HashMap;

use edn::parser::Parser;
use edn::Value as EdnValue;

use crate::datom::{Attr, Value};
use crate::relation::{Clause, PatternEl, RuleBranch, Rules, Var};

/// Strip `;`-style line comments from EDN text.
/// The `edn` 0.3 crate panics on comment syntax, so we remove them before parsing.
pub fn strip_edn_comments(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_string = false;
    let mut escape_next = false;
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if escape_next {
            result.push(ch);
            escape_next = false;
            continue;
        }
        if in_string {
            result.push(ch);
            if ch == '\\' {
                escape_next = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
            result.push(ch);
        } else if ch == ';' {
            // Skip until newline
            for c in chars.by_ref() {
                if c == '\n' {
                    result.push('\n');
                    break;
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Parsed query
// ---------------------------------------------------------------------------

/// The result type for `:find` — how to shape the output.
#[derive(Clone, Debug, PartialEq)]
pub enum FindSpec {
    /// `:find ?a ?b` — relation (set of tuples)
    Rel(Vec<Var>),
    /// `:find [?a ...]` — collection (flat list of single values)
    Coll(Var),
    /// `:find ?a .` — scalar (single value)
    Scalar(Var),
    /// `:find [?a ?b]` — single tuple
    Tuple(Vec<Var>),
}

impl FindSpec {
    pub fn vars(&self) -> Vec<Var> {
        match self {
            FindSpec::Rel(vs) => vs.clone(),
            FindSpec::Coll(v) => vec![v.clone()],
            FindSpec::Scalar(v) => vec![v.clone()],
            FindSpec::Tuple(vs) => vs.clone(),
        }
    }
}

/// An element in a `:find` clause — variable, pull expression, or aggregate.
#[derive(Clone, Debug)]
pub enum FindElement {
    /// Plain variable: `?name`
    Var(Var),
    /// Pull expression: `(pull ?e [:name :age])`
    Pull {
        var: Var,
        /// The raw EDN of the pull pattern (parsed later with schema context).
        pattern_edn: EdnValue,
    },
    /// Aggregate: `(sum ?x)`, `(count ?e)`, `(min 3 ?x)`, etc.
    Aggregate {
        /// Aggregate function name: "sum", "avg", "count", etc.
        name: String,
        /// The variable being aggregated (last argument).
        var: Var,
        /// Optional numeric argument (e.g., `(min 3 ?x)` → n_arg = Some(3)).
        n_arg: Option<i64>,
    },
}

impl FindElement {
    pub fn var(&self) -> &Var {
        match self {
            FindElement::Var(v) => v,
            FindElement::Pull { var, .. } => var,
            FindElement::Aggregate { var, .. } => var,
        }
    }

    pub fn is_aggregate(&self) -> bool {
        matches!(self, FindElement::Aggregate { .. })
    }
}

/// How an `:in` parameter is bound.
#[derive(Clone, Debug)]
pub enum InBinding {
    /// `?x` — single value substituted into patterns
    Scalar(Var),
    /// `[?x ...]` — collection, each element creates a row in an initial relation
    Coll(Var),
    /// `[[?a ?b]]` — tuple, destructured into multiple scalar bindings
    Tuple(Vec<Var>),
}

/// A fully parsed Datalog query.
#[derive(Clone, Debug)]
pub struct ParsedQuery {
    pub find: FindSpec,
    /// Full find elements including pull expressions. Parallel to `find.vars()`.
    pub find_elements: Vec<FindElement>,
    pub in_vars: Vec<String>,
    pub in_bindings: Vec<InBinding>,
    pub where_clauses: Vec<Clause>,
    pub rules: Rules,
}

impl ParsedQuery {
    pub fn has_pull_in_find(&self) -> bool {
        self.find_elements.iter().any(|e| matches!(e, FindElement::Pull { .. }))
    }

    pub fn has_aggregates(&self) -> bool {
        self.find_elements.iter().any(|e| e.is_aggregate())
    }
}

// ---------------------------------------------------------------------------
// EDN → PatternEl / Clause / Rules
// ---------------------------------------------------------------------------

fn is_variable(s: &str) -> bool {
    s.starts_with('?')
}

fn is_src_var(v: &EdnValue) -> bool {
    matches!(v, EdnValue::Symbol(s) if s.starts_with('$'))
}

/// Convert an EDN keyword string to a `Value::Keyword(Attr)`.
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

/// Convert an EDN value to a `PatternEl`.
fn edn_to_pattern_el(v: &EdnValue) -> PatternEl {
    match v {
        EdnValue::Symbol(s) if s == "_" => PatternEl::Blank,
        EdnValue::Symbol(s) if s.starts_with('?') => PatternEl::Var(s.clone()),
        EdnValue::Symbol(s) => {
            // Plain symbol — treat as string constant
            PatternEl::Const(Value::Str(s.clone()))
        }
        EdnValue::Keyword(kw) => {
            PatternEl::Const(Value::Keyword(keyword_to_attr(kw)))
        }
        EdnValue::Integer(n) => PatternEl::Const(Value::Long(*n)),
        EdnValue::Float(f) => PatternEl::Const(Value::Double(f.into_inner())),
        EdnValue::String(s) => PatternEl::Const(Value::Str(s.clone())),
        EdnValue::Boolean(b) => PatternEl::Const(Value::Bool(*b)),
        EdnValue::Nil => PatternEl::Blank,
        _ => PatternEl::Blank,
    }
}

/// Parse a vector like `[?e :name ?n]` or `[$ws ?e :name ?n]` into a `Clause::Pattern`.
/// Vectors are data patterns (always 3 or 4 elements: e, a, v, [tx]).
/// An optional leading source var (`$`, `$ws`) is captured as the pattern's source.
fn parse_pattern(elems: &[EdnValue]) -> Option<Clause> {
    // Check for leading source var ($, $ws, etc.)
    let (source, elems) = if !elems.is_empty() && is_src_var(&elems[0]) {
        let src = match &elems[0] {
            EdnValue::Symbol(s) => {
                if s == "$" { None } else { Some(s.clone()) }
            }
            _ => None,
        };
        (src, &elems[1..])
    } else {
        (None, elems)
    };

    // A pattern needs at least 2 elements (e, a)
    if elems.len() < 2 || elems.len() > 4 {
        return None;
    }

    let mut pattern = [PatternEl::Blank, PatternEl::Blank, PatternEl::Blank, PatternEl::Blank];
    for (i, el) in elems.iter().enumerate() {
        pattern[i] = edn_to_pattern_el(el);
    }
    Some(Clause::Pattern { source, pattern })
}

/// Parse a list like `(rule-name ?a ?b)` into a `Clause::RuleCall`.
fn parse_rule_expr(elems: &[EdnValue]) -> Option<Clause> {
    // Skip leading source var
    let elems = if !elems.is_empty() && is_src_var(&elems[0]) {
        &elems[1..]
    } else {
        elems
    };

    if elems.is_empty() {
        return None;
    }

    let name = match &elems[0] {
        EdnValue::Symbol(s) if !s.starts_with('?') && s != "_" => s.clone(),
        _ => return None,
    };

    let args: Vec<PatternEl> = elems[1..].iter().map(edn_to_pattern_el).collect();
    Some(Clause::RuleCall { name, args })
}

/// Parse a predicate expression like `[(> ?age 21)]` into `Clause::Predicate`.
/// The outer form is a vector containing a single list/vector `[pred-call]`.
fn parse_pred_expr(inner: &[EdnValue]) -> Option<Clause> {
    if inner.is_empty() {
        return None;
    }

    // The first (and only for predicates) element is the call form [fn arg1 arg2 ...]
    let call = &inner[0];
    let call_elems = match call {
        EdnValue::Vector(v) => v.as_slice(),
        EdnValue::List(v) => v.as_slice(),
        _ => return None,
    };

    if call_elems.is_empty() {
        return None;
    }

    let name = match &call_elems[0] {
        EdnValue::Symbol(s) => s.clone(),
        _ => return None,
    };

    let args: Vec<PatternEl> = call_elems[1..].iter().map(edn_to_pattern_el).collect();

    // Database-aware function names (take $ as first arg)
    let is_db_fn = matches!(name.as_str(), "get-else" | "get-some" | "missing?");

    // If there's a second element in `inner`, it's a binding (fn-expr, not pred-expr).
    if inner.len() == 1 {
        // Predicate form — but missing? as predicate is DB-aware
        if is_db_fn {
            Some(Clause::DbFnExpr { name, args, binding: String::new() })
        } else {
            Some(Clause::Predicate { name, args })
        }
    } else {
        // fn-expr: [(fn ?a ?b) ?result]
        let binding = match &inner[1] {
            EdnValue::Symbol(s) if is_variable(s) => s.clone(),
            _ => return None,
        };
        if is_db_fn {
            Some(Clause::DbFnExpr { name, args, binding })
        } else {
            Some(Clause::FnExpr { name, args, binding })
        }
    }
}

/// Parse a single branch of an `or` clause.
/// Each branch is either a single clause or `(and clause+ )`.
fn parse_or_branch(el: &EdnValue) -> Vec<Clause> {
    // Check for (and ...) — list or vector form
    match el {
        EdnValue::List(inner) | EdnValue::Vector(inner)
            if !inner.is_empty()
                && matches!(&inner[0], EdnValue::Symbol(s) if s == "and") =>
        {
            inner[1..].iter().filter_map(parse_clause).collect()
        }
        _ => parse_clause(el).into_iter().collect(),
    }
}

/// Parse a single WHERE clause from EDN.
fn parse_clause(form: &EdnValue) -> Option<Clause> {
    match form {
        // Vector: data pattern `[?e :attr ?v]` or pred-expr `[(> ?a 1)]` or
        // not/or/and with keyword prefix
        EdnValue::Vector(elems) => {
            if elems.is_empty() {
                return None;
            }

            // Check for not/not-join
            if matches!(&elems[0], EdnValue::Symbol(s) if s == "not") {
                let clauses: Vec<Clause> = elems[1..]
                    .iter()
                    .filter_map(parse_clause)
                    .collect();
                return Some(Clause::Not(clauses));
            }
            if matches!(&elems[0], EdnValue::Symbol(s) if s == "not-join") {
                // not-join has [vars...] as second element
                let clauses: Vec<Clause> = elems[2..]
                    .iter()
                    .filter_map(parse_clause)
                    .collect();
                return Some(Clause::Not(clauses));
            }

            // Check for or/or-join
            if matches!(&elems[0], EdnValue::Symbol(s) if s == "or") {
                let branches: Vec<Vec<Clause>> = elems[1..]
                    .iter()
                    .map(parse_or_branch)
                    .collect();
                return Some(Clause::Or(branches));
            }
            if matches!(&elems[0], EdnValue::Symbol(s) if s == "or-join") {
                // or-join has [vars...] as second element, clauses after
                let branches: Vec<Vec<Clause>> = elems[2..]
                    .iter()
                    .map(parse_or_branch)
                    .collect();
                return Some(Clause::Or(branches));
            }

            // Check for and
            if matches!(&elems[0], EdnValue::Symbol(s) if s == "and") {
                let clauses: Vec<Clause> = elems[1..]
                    .iter()
                    .filter_map(parse_clause)
                    .collect();
                return Some(Clause::And(clauses));
            }

            // Determine the "effective" elements (after optional source-var prefix)
            // for pred-expr detection, but pass full elems to parse_pattern
            // so it can capture the source var.
            let elems_ref = if !elems.is_empty() && is_src_var(&elems[0]) {
                &elems[1..]
            } else {
                elems.as_slice()
            };

            // Check if first element (after source) is a nested vector/list → pred-expr
            if !elems_ref.is_empty()
                && matches!(&elems_ref[0], EdnValue::Vector(_) | EdnValue::List(_))
            {
                return parse_pred_expr(elems_ref);
            }

            // Otherwise it's a data pattern — pass full elems so source var is captured
            parse_pattern(elems.as_slice())
        }

        // List: rule expression `(rule-name ?a ?b)` or `(not ...)` etc.
        EdnValue::List(elems) => {
            if elems.is_empty() {
                return None;
            }

            // Check for not/not-join in list form
            if matches!(&elems[0], EdnValue::Symbol(s) if s == "not") {
                let clauses: Vec<Clause> = elems[1..]
                    .iter()
                    .filter_map(parse_clause)
                    .collect();
                return Some(Clause::Not(clauses));
            }
            if matches!(&elems[0], EdnValue::Symbol(s) if s == "not-join") {
                let clauses: Vec<Clause> = elems[2..]
                    .iter()
                    .filter_map(parse_clause)
                    .collect();
                return Some(Clause::Not(clauses));
            }

            // Check for or/or-join in list form
            if matches!(&elems[0], EdnValue::Symbol(s) if s == "or") {
                let branches: Vec<Vec<Clause>> = elems[1..]
                    .iter()
                    .map(parse_or_branch)
                    .collect();
                return Some(Clause::Or(branches));
            }
            if matches!(&elems[0], EdnValue::Symbol(s) if s == "or-join") {
                let branches: Vec<Vec<Clause>> = elems[2..]
                    .iter()
                    .map(parse_or_branch)
                    .collect();
                return Some(Clause::Or(branches));
            }

            // Check for and in list form
            if matches!(&elems[0], EdnValue::Symbol(s) if s == "and") {
                let clauses: Vec<Clause> = elems[1..]
                    .iter()
                    .filter_map(parse_clause)
                    .collect();
                return Some(Clause::And(clauses));
            }

            // Otherwise it's a rule expression
            parse_rule_expr(elems)
        }

        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Query parsing
// ---------------------------------------------------------------------------

/// Split a query vector into sections by keyword.
/// Returns a map of keyword → Vec<EdnValue>.
fn query_to_map(elems: &[EdnValue]) -> HashMap<String, Vec<EdnValue>> {
    let mut result: HashMap<String, Vec<EdnValue>> = HashMap::new();
    let mut current_key: Option<String> = None;

    for el in elems {
        if let EdnValue::Keyword(kw) = el {
            current_key = Some(kw.clone());
            result.entry(kw.clone()).or_default();
        } else if let Some(ref key) = current_key {
            result.entry(key.clone()).or_default().push(el.clone());
        }
    }
    result
}

/// Parse an aggregate find element: `(sum ?x)` or `(min 3 ?x)`.
/// `args` is everything after the function name.
fn parse_aggregate_element(name: &str, args: &[&EdnValue]) -> Option<FindElement> {
    if args.is_empty() {
        return None;
    }

    // Last arg must be a variable
    let last = args.last()?;
    let var = match last {
        EdnValue::Symbol(s) if is_variable(s) => s.clone(),
        _ => return None,
    };

    // Optional numeric argument before the variable (e.g., `(min 3 ?x)`)
    let n_arg = if args.len() >= 2 {
        match args[0] {
            EdnValue::Integer(n) => Some(*n),
            _ => None,
        }
    } else {
        None
    };

    Some(FindElement::Aggregate {
        name: name.to_string(),
        var,
        n_arg,
    })
}

/// Known built-in aggregate function names.
const BUILTIN_AGGREGATES: &[&str] = &[
    "sum", "avg", "median", "variance", "stddev",
    "min", "max", "count", "count-distinct", "distinct",
    "rand", "sample",
];

/// Parse the `:find` section.
/// Parse a find element — variable, `(pull ?var pattern)`, or `(agg ?var)`.
fn parse_find_element(el: &EdnValue) -> Option<FindElement> {
    match el {
        EdnValue::Symbol(s) if is_variable(s) => Some(FindElement::Var(s.clone())),
        EdnValue::List(items) => {
            let items: Vec<&EdnValue> = items.iter().collect();
            if items.len() >= 2 {
                if let EdnValue::Symbol(s) = items[0] {
                    // (pull ?e [:attr ...])
                    if s == "pull" && items.len() >= 3 {
                        // items[1] might be $src or ?var
                        let (var_idx, _) = if let EdnValue::Symbol(s) = items[1] {
                            if s.starts_with('$') {
                                (2, Some(s.clone()))
                            } else {
                                (1, None)
                            }
                        } else {
                            (1, None)
                        };
                        if let EdnValue::Symbol(var) = items[var_idx] {
                            if is_variable(var) && var_idx + 1 < items.len() {
                                return Some(FindElement::Pull {
                                    var: var.clone(),
                                    pattern_edn: items[var_idx + 1].clone(),
                                });
                            }
                        }
                        return None;
                    }

                    // (sum ?x), (count ?e), (min 3 ?x), etc.
                    if BUILTIN_AGGREGATES.contains(&s.as_str()) {
                        return parse_aggregate_element(s, &items[1..]);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn parse_find(elems: &[EdnValue]) -> (FindSpec, Vec<FindElement>) {
    if elems.is_empty() {
        return (FindSpec::Rel(vec![]), vec![]);
    }

    // Check for scalar: `?a .` or `(pull ?a [...]) .`
    if elems.len() >= 2 && matches!(&elems[elems.len() - 1], EdnValue::Symbol(s) if s == ".") {
        if let Some(fe) = parse_find_element(&elems[0]) {
            return (FindSpec::Scalar(fe.var().clone()), vec![fe]);
        }
    }

    // Check for collection: `[?a ...]` or `[(pull ?a [...]) ...]`
    if elems.len() == 1 {
        if let EdnValue::Vector(inner) = &elems[0] {
            if inner.len() == 2 && matches!(&inner[1], EdnValue::Symbol(s) if s == "...") {
                if let Some(fe) = parse_find_element(&inner[0]) {
                    return (FindSpec::Coll(fe.var().clone()), vec![fe]);
                }
            }
            // Check for tuple: `[?a ?b]`
            let fes: Vec<FindElement> = inner
                .iter()
                .filter_map(parse_find_element)
                .collect();
            if fes.len() == inner.len() {
                let vars = fes.iter().map(|fe| fe.var().clone()).collect();
                return (FindSpec::Tuple(vars), fes);
            }
        }
    }

    // Default: relation `?a ?b ?c` or `?a (pull ?e [...]) ?c`
    let fes: Vec<FindElement> = elems
        .iter()
        .filter_map(|el| {
            // Skip `.` dot symbols
            if matches!(el, EdnValue::Symbol(s) if s == ".") {
                return None;
            }
            parse_find_element(el)
        })
        .collect();
    let vars = fes.iter().map(|fe| fe.var().clone()).collect();
    (FindSpec::Rel(vars), fes)
}

/// Parse the `:in` section. Returns (scalar var names, all bindings).
fn parse_in(elems: &[EdnValue]) -> (Vec<String>, Vec<InBinding>) {
    let mut vars = Vec::new();
    let mut bindings = Vec::new();

    for el in elems {
        match el {
            // Skip source vars ($, $ws) and rules var (%)
            EdnValue::Symbol(s) if s.starts_with('$') || s == "%" || s == "_" => {}
            // Scalar binding: ?x
            EdnValue::Symbol(s) if is_variable(s) => {
                vars.push(s.clone());
                bindings.push(InBinding::Scalar(s.clone()));
            }
            // Vector form: [?x ...] (collection) or [[?a ?b]] (tuple)
            EdnValue::Vector(inner) => {
                if inner.len() == 2 {
                    if let EdnValue::Symbol(dots) = &inner[1] {
                        if dots == "..." {
                            // Collection binding: [?x ...]
                            if let EdnValue::Symbol(v) = &inner[0] {
                                if is_variable(v) {
                                    vars.push(v.clone());
                                    bindings.push(InBinding::Coll(v.clone()));
                                }
                            }
                            continue;
                        }
                    }
                }
                // Check for tuple binding: [[?a ?b]]
                if inner.len() == 1 {
                    if let EdnValue::Vector(tuple_vars) = &inner[0] {
                        let tvars: Vec<String> = tuple_vars
                            .iter()
                            .filter_map(|v| match v {
                                EdnValue::Symbol(s) if is_variable(s) => Some(s.clone()),
                                _ => None,
                            })
                            .collect();
                        if tvars.len() == tuple_vars.len() {
                            for tv in &tvars {
                                vars.push(tv.clone());
                            }
                            bindings.push(InBinding::Tuple(tvars));
                            continue;
                        }
                    }
                }
                // Fallback: treat vector elements as individual vars
                for v in inner {
                    if let EdnValue::Symbol(s) = v {
                        if is_variable(s) {
                            vars.push(s.clone());
                            bindings.push(InBinding::Scalar(s.clone()));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    (vars, bindings)
}

/// Parse a Datalog query from an EDN string.
///
/// Supports the standard DataScript query format:
/// ```edn
/// [:find ?name ?age
///  :in $ % ?min-age
///  :where
///  [?e :name ?name]
///  [?e :age ?age]
///  [(>= ?age ?min-age)]
///  (some-rule ?e ?extra)]
/// ```
pub fn parse_query(edn_str: &str) -> ParsedQuery {
    let cleaned = strip_edn_comments(edn_str);
    let mut parser = Parser::new(&cleaned);
    let val = parser
        .read()
        .expect("failed to parse query EDN")
        .expect("empty query EDN");

    parse_query_edn(&val)
}

/// Parse a query from an already-parsed EDN value.
pub fn parse_query_edn(val: &EdnValue) -> ParsedQuery {
    let elems = match val {
        EdnValue::Vector(v) => v.as_slice(),
        EdnValue::Map(_) => {
            return parse_query_map(val);
        }
        _ => panic!("query must be a vector or map"),
    };

    let sections = query_to_map(elems);

    let (find, find_elements) = sections.get("find").map(|v| parse_find(v)).unwrap_or((FindSpec::Rel(vec![]), vec![]));
    let (in_vars, in_bindings) = sections.get("in").map(|v| parse_in(v)).unwrap_or_default();
    let where_clauses = sections
        .get("where")
        .map(|v| v.iter().filter_map(parse_clause).collect())
        .unwrap_or_default();

    ParsedQuery {
        find,
        find_elements,
        in_vars,
        in_bindings,
        where_clauses,
        rules: Rules::new(),
    }
}

/// Parse a query from a map-format EDN value.
fn parse_query_map(val: &EdnValue) -> ParsedQuery {
    let get = |key: &str| -> Vec<EdnValue> {
        if let EdnValue::Map(m) = val {
            for (k, v) in m.iter() {
                if let EdnValue::Keyword(kw) = k {
                    if kw == key {
                        return match v {
                            EdnValue::Vector(elems) => elems.clone(),
                            _ => vec![v.clone()],
                        };
                    }
                }
            }
        }
        vec![]
    };

    let (find, find_elements) = parse_find(&get("find"));
    let (in_vars, in_bindings) = parse_in(&get("in"));
    let where_clauses = get("where")
        .iter()
        .filter_map(parse_clause)
        .collect();

    ParsedQuery {
        find,
        find_elements,
        in_vars,
        in_bindings,
        where_clauses,
        rules: Rules::new(),
    }
}

// ---------------------------------------------------------------------------
// Rules parsing
// ---------------------------------------------------------------------------

/// Parse rules from an EDN string.
///
/// Rules format:
/// ```edn
/// [[(rule-name ?a ?b) [?a :attr ?b]]
///  [(rule-name ?a ?b) [?a :other ?x] (rule-name ?x ?b)]
///  [(other-rule ?x) [?x :foo _]]]
/// ```
pub fn parse_rules(edn_str: &str) -> Rules {
    let cleaned = strip_edn_comments(edn_str);
    let mut parser = Parser::new(&cleaned);
    let val = parser
        .read()
        .expect("failed to parse rules EDN")
        .expect("empty rules EDN");

    parse_rules_edn(&val)
}

/// Parse rules from an already-parsed EDN value.
pub fn parse_rules_edn(val: &EdnValue) -> Rules {
    let branches = match val {
        EdnValue::Vector(v) => v,
        _ => return Rules::new(),
    };

    let mut rules: Rules = HashMap::new();

    for branch in branches {
        let elems = match branch {
            EdnValue::Vector(v) => v,
            _ => continue,
        };
        if elems.is_empty() {
            continue;
        }

        // First element is the head: [rule-name ?arg1 ?arg2 ...]
        let head = match &elems[0] {
            EdnValue::Vector(v) => v,
            EdnValue::List(v) => v,
            _ => continue,
        };
        if head.is_empty() {
            continue;
        }

        let name = match &head[0] {
            EdnValue::Symbol(s) => s.clone(),
            _ => continue,
        };

        let head_args: Vec<Var> = head[1..]
            .iter()
            .filter_map(|el| match el {
                EdnValue::Symbol(s) if is_variable(s) => Some(s.clone()),
                _ => None,
            })
            .collect();

        // Remaining elements are body clauses
        let body: Vec<Clause> = elems[1..]
            .iter()
            .filter_map(parse_clause)
            .collect();

        rules.entry(name).or_default().push(RuleBranch { head_args, body });
    }

    rules
}

// ---------------------------------------------------------------------------
// Input binding
// ---------------------------------------------------------------------------

/// Bind input parameters to a query's WHERE clauses by substituting
/// `PatternEl::Var` with `PatternEl::Const` for each `:in` variable.
pub fn bind_inputs(query: &mut ParsedQuery, inputs: &[(&str, Value)]) {
    let bindings: HashMap<String, Value> = inputs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();

    for clause in &mut query.where_clauses {
        bind_clause(clause, &bindings);
    }
}

/// Process collection and tuple bindings from `:in`, returning initial
/// relations that should be pre-seeded into the query context.
///
/// - `Scalar` bindings are handled by `bind_inputs` (pattern substitution)
/// - `Coll` bindings create a relation with one row per collection element
/// - `Tuple` bindings are expanded to scalar substitutions
///
/// `input_values` maps variable names to their input values. For `Coll`,
/// the value should be `Value::Nil` placeholder — the actual collection
/// is passed via `coll_values` keyed by var name.
pub fn build_collection_relations(
    in_bindings: &[InBinding],
    coll_values: &HashMap<String, Vec<Value>>,
) -> Vec<crate::relation::Relation> {
    use crate::relation::Relation;

    let mut rels = Vec::new();

    for binding in in_bindings {
        match binding {
            InBinding::Coll(var) => {
                if let Some(values) = coll_values.get(var) {
                    let mut attrs = HashMap::new();
                    attrs.insert(var.clone(), 0);
                    let tuples: Vec<Vec<Value>> = values
                        .iter()
                        .map(|v| vec![v.clone()])
                        .collect();
                    // Always push — even empty relations constrain the join to 0 results
                    rels.push(Relation::new(attrs, tuples));
                }
            }
            InBinding::Scalar(_) | InBinding::Tuple(_) => {
                // Scalar: handled by bind_inputs
                // Tuple: caller should expand to scalars before calling bind_inputs
            }
        }
    }

    rels
}

fn bind_clause(clause: &mut Clause, bindings: &HashMap<String, Value>) {
    match clause {
        Clause::Pattern { pattern: p, .. } => {
            for el in p.iter_mut() {
                bind_el(el, bindings);
            }
        }
        Clause::RuleCall { args, .. } => {
            for el in args.iter_mut() {
                bind_el(el, bindings);
            }
        }
        Clause::Predicate { args, .. } => {
            for el in args.iter_mut() {
                bind_el(el, bindings);
            }
        }
        Clause::FnExpr { args, .. } => {
            for el in args.iter_mut() {
                bind_el(el, bindings);
            }
        }
        Clause::DbFnExpr { args, .. } => {
            for el in args.iter_mut() {
                bind_el(el, bindings);
            }
        }
        Clause::And(cs) => {
            for c in cs {
                bind_clause(c, bindings);
            }
        }
        Clause::Or(branches) => {
            for branch in branches {
                for c in branch {
                    bind_clause(c, bindings);
                }
            }
        }
        Clause::Not(cs) => {
            for c in cs {
                bind_clause(c, bindings);
            }
        }
    }
}

fn bind_el(el: &mut PatternEl, bindings: &HashMap<String, Value>) {
    if let PatternEl::Var(name) = el {
        if let Some(val) = bindings.get(name) {
            *el = PatternEl::Const(val.clone());
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relation::PatternEl;
    use crate::datom::Value;
    use crate::schema::kw;

    #[test]
    fn parse_simple_query() {
        let q = parse_query("[:find ?e ?name :where [?e :name ?name]]");
        assert_eq!(q.find, FindSpec::Rel(vec!["?e".into(), "?name".into()]));
        assert_eq!(q.where_clauses.len(), 1);
        match &q.where_clauses[0] {
            Clause::Pattern { pattern: p, .. } => {
                assert_eq!(p[0], PatternEl::Var("?e".into()));
                assert_eq!(p[1], PatternEl::Const(Value::Keyword(kw("name"))));
                assert_eq!(p[2], PatternEl::Var("?name".into()));
                assert_eq!(p[3], PatternEl::Blank);
            }
            other => panic!("expected Pattern, got {:?}", other),
        }
    }

    #[test]
    fn parse_multi_clause_query() {
        let q = parse_query(
            "[:find ?name ?age :where [?e :name ?name] [?e :age ?age]]"
        );
        assert_eq!(q.find.vars(), vec!["?name", "?age"]);
        assert_eq!(q.where_clauses.len(), 2);
    }

    #[test]
    fn parse_scalar_find() {
        let q = parse_query("[:find ?name . :where [?e :name ?name]]");
        assert_eq!(q.find, FindSpec::Scalar("?name".into()));
    }

    #[test]
    fn parse_coll_find() {
        let q = parse_query("[:find [?name ...] :where [?e :name ?name]]");
        assert_eq!(q.find, FindSpec::Coll("?name".into()));
    }

    #[test]
    fn parse_tuple_find() {
        let q = parse_query("[:find [?name ?age] :where [?e :name ?name] [?e :age ?age]]");
        assert_eq!(q.find, FindSpec::Tuple(vec!["?name".into(), "?age".into()]));
    }

    #[test]
    fn parse_in_vars() {
        let q = parse_query(
            "[:find ?e :in $ % ?uuid :where [?e :bp/uuid ?uuid]]"
        );
        assert_eq!(q.in_vars, vec!["?uuid"]);
    }

    #[test]
    fn parse_predicate() {
        let q = parse_query(
            "[:find ?e ?age :where [?e :age ?age] [(> ?age 21)]]"
        );
        assert_eq!(q.where_clauses.len(), 2);
        match &q.where_clauses[1] {
            Clause::Predicate { name, args } => {
                assert_eq!(name, ">");
                assert_eq!(args.len(), 2);
                assert_eq!(args[0], PatternEl::Var("?age".into()));
                assert_eq!(args[1], PatternEl::Const(Value::Long(21)));
            }
            other => panic!("expected Predicate, got {:?}", other),
        }
    }

    #[test]
    fn parse_rule_call_in_where() {
        let q = parse_query(
            "[:find ?e :in $ % :where (subgroup 1 ?e)]"
        );
        assert_eq!(q.where_clauses.len(), 1);
        match &q.where_clauses[0] {
            Clause::RuleCall { name, args } => {
                assert_eq!(name, "subgroup");
                assert_eq!(args.len(), 2);
                assert_eq!(args[0], PatternEl::Const(Value::Long(1)));
                assert_eq!(args[1], PatternEl::Var("?e".into()));
            }
            other => panic!("expected RuleCall, got {:?}", other),
        }
    }

    #[test]
    fn parse_namespaced_attr() {
        let q = parse_query("[:find ?e :where [?e :bp/uuid \"abc\"]]");
        match &q.where_clauses[0] {
            Clause::Pattern { pattern: p, .. } => {
                assert_eq!(
                    p[1],
                    PatternEl::Const(Value::Keyword(Attr::Keyword {
                        ns: Some("bp".into()),
                        name: "uuid".into(),
                    }))
                );
                assert_eq!(p[2], PatternEl::Const(Value::Str("abc".into())));
            }
            other => panic!("expected Pattern, got {:?}", other),
        }
    }

    #[test]
    fn parse_wildcard() {
        let q = parse_query("[:find ?e :where [?e :name _]]");
        match &q.where_clauses[0] {
            Clause::Pattern { pattern: p, .. } => {
                assert_eq!(p[2], PatternEl::Blank);
            }
            _ => panic!("expected Pattern"),
        }
    }

    #[test]
    fn parse_rules_simple() {
        let rules = parse_rules(
            "[[(lookup ?uuid ?e) [?e :bp/uuid ?uuid]]]"
        );
        assert!(rules.contains_key("lookup"));
        let branches = &rules["lookup"];
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].head_args, vec!["?uuid", "?e"]);
        assert_eq!(branches[0].body.len(), 1);
    }

    #[test]
    fn parse_rules_multi_branch() {
        let rules = parse_rules(
            "[[(subgroup ?g ?s) [?g :groups ?s]]
              [(subgroup ?g ?s) [?g :groups ?x] (subgroup ?x ?s)]]"
        );
        let branches = &rules["subgroup"];
        assert_eq!(branches.len(), 2);
        // First branch: one pattern clause
        assert_eq!(branches[0].body.len(), 1);
        // Second branch: one pattern + one rule call
        assert_eq!(branches[1].body.len(), 2);
        match &branches[1].body[1] {
            Clause::RuleCall { name, args } => {
                assert_eq!(name, "subgroup");
                assert_eq!(args.len(), 2);
            }
            other => panic!("expected RuleCall, got {:?}", other),
        }
    }

    #[test]
    fn parse_rules_multiple_names() {
        let rules = parse_rules(
            "[[(lookup ?uuid ?e) [?e :bp/uuid ?uuid]]
              [(app-root ?a ?g)
               [?sm :submodule/groups ?g]
               [?m :module/submodules ?sm]
               [?a :application/modules ?m]]]"
        );
        assert!(rules.contains_key("lookup"));
        assert!(rules.contains_key("app-root"));
        assert_eq!(rules["app-root"][0].body.len(), 3);
    }

    #[test]
    fn bind_inputs_substitutes() {
        let mut q = parse_query(
            "[:find ?e :in $ ?uuid :where [?e :bp/uuid ?uuid]]"
        );
        bind_inputs(&mut q, &[("?uuid", Value::Str("abc-123".into()))]);
        match &q.where_clauses[0] {
            Clause::Pattern { pattern: p, .. } => {
                assert_eq!(p[2], PatternEl::Const(Value::Str("abc-123".into())));
            }
            _ => panic!("expected Pattern"),
        }
    }

    #[test]
    fn parse_not_clause() {
        let q = parse_query(
            "[:find ?e :where [?e :name _] (not [?e :hidden true])]"
        );
        assert_eq!(q.where_clauses.len(), 2);
        match &q.where_clauses[1] {
            Clause::Not(clauses) => {
                assert_eq!(clauses.len(), 1);
            }
            other => panic!("expected Not, got {:?}", other),
        }
    }

    #[test]
    fn parse_or_clause() {
        let q = parse_query(
            "[:find ?e :where (or [?e :type :a] [?e :type :b])]"
        );
        assert_eq!(q.where_clauses.len(), 1);
        match &q.where_clauses[0] {
            Clause::Or(branches) => {
                assert_eq!(branches.len(), 2);
            }
            other => panic!("expected Or, got {:?}", other),
        }
    }

    #[test]
    fn parse_keyword_value() {
        let q = parse_query("[:find ?e :where [?e :type :worksheet]]");
        match &q.where_clauses[0] {
            Clause::Pattern { pattern: p, .. } => {
                assert_eq!(
                    p[2],
                    PatternEl::Const(Value::Keyword(kw("worksheet")))
                );
            }
            _ => panic!("expected Pattern"),
        }
    }

    #[test]
    fn parse_boolean_value() {
        let q = parse_query("[:find ?e :where [?e :active true]]");
        match &q.where_clauses[0] {
            Clause::Pattern { pattern: p, .. } => {
                assert_eq!(p[2], PatternEl::Const(Value::Bool(true)));
            }
            _ => panic!("expected Pattern"),
        }
    }

    #[test]
    fn roundtrip_parse_and_resolve() {
        // Integration test: parse query + rules, resolve against DataScriptDB
        use crate::db::DataScriptDB;
        use crate::relation::{project, resolve_query};
        use crate::schema::{AttrSchema, Schema, ValueType, Cardinality};

        let mut schema = Schema::default();
        schema.attrs.insert(kw("groups"), AttrSchema {
            value_type: Some(ValueType::Ref),
            cardinality: Cardinality::Many,
            ..Default::default()
        });
        schema.attrs.insert(kw("name"), AttrSchema { index: true, ..Default::default() });

        let mut db = DataScriptDB::empty(schema);
        use crate::datom::Datom;
        use crate::db::TX0;
        db.with_datoms(vec![
            Datom::new(1, Some(kw("name")), Value::Str("root".into()), TX0 + 1),
            Datom::new(1, Some(kw("groups")), Value::Ref(2), TX0 + 1),
            Datom::new(2, Some(kw("name")), Value::Str("child".into()), TX0 + 1),
            Datom::new(2, Some(kw("groups")), Value::Ref(3), TX0 + 1),
            Datom::new(3, Some(kw("name")), Value::Str("grandchild".into()), TX0 + 1),
        ]);

        let q = parse_query(
            "[:find ?name :in $ % :where (subgroup 1 ?s) [?s :name ?name]]"
        );
        let rules = parse_rules(
            "[[(subgroup ?g ?s) [?g :groups ?s]]
              [(subgroup ?g ?s) [?g :groups ?x] (subgroup ?x ?s)]]"
        );

        let result = resolve_query(&db, &q.where_clauses, &rules);
        let projected = project(&result, &q.find.vars());

        let name_idx = projected.attrs["?name"];
        let mut names: Vec<&str> = projected.tuples.iter().filter_map(|t| match &t[name_idx] {
            Value::Str(s) => Some(s.as_str()),
            _ => None,
        }).collect();
        names.sort();
        assert_eq!(names, vec!["child", "grandchild"]);
    }

    // Pull-in-find parsing tests

    #[test]
    fn parse_pull_in_find_rel() {
        let q = parse_query(
            "[:find ?name (pull ?e [:age]) :where [?e :name ?name]]"
        );
        assert_eq!(q.find.vars(), vec!["?name".to_string(), "?e".to_string()]);
        assert_eq!(q.find_elements.len(), 2);
        assert!(matches!(&q.find_elements[0], FindElement::Var(v) if v == "?name"));
        assert!(matches!(&q.find_elements[1], FindElement::Pull { var, .. } if var == "?e"));
        assert!(q.has_pull_in_find());
    }

    #[test]
    fn parse_pull_in_find_coll() {
        let q = parse_query(
            "[:find [(pull ?e [*]) ...] :where [?e :name _]]"
        );
        assert_eq!(q.find.vars(), vec!["?e".to_string()]);
        assert!(matches!(&q.find_elements[0], FindElement::Pull { var, .. } if var == "?e"));
    }

    #[test]
    fn parse_pull_in_find_scalar() {
        let q = parse_query(
            "[:find (pull ?e [:name :age]) . :where [?e :name \"Alice\"]]"
        );
        assert!(matches!(q.find, FindSpec::Scalar(_)));
        assert!(matches!(&q.find_elements[0], FindElement::Pull { var, .. } if var == "?e"));
    }

    #[test]
    fn parse_no_pull_in_find() {
        let q = parse_query("[:find ?e :where [?e :name _]]");
        assert!(!q.has_pull_in_find());
        assert!(matches!(&q.find_elements[0], FindElement::Var(v) if v == "?e"));
    }
}
