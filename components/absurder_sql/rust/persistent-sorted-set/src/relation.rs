//! Relational algebra for Datalog query resolution.
//!
//! Mirrors the CLJS `query.cljc` relation operations:
//! - `Relation`: a binding table mapping variable names to tuple columns
//! - `hash_join`: equi-join on shared variables
//! - `sum_rel`: union of two relations with the same schema
//! - `prod_rel`: cartesian product
//! - `subtract_rel`: set difference on shared variables
//! - `collapse_rels`: fold a new relation into an accumulator via hash-joins
//! - `solve_rule`: stack-based Datalog rule evaluation
//! - `resolve_clause`: dispatches Pattern, RuleCall, And, Or, Not, Predicate

use std::collections::HashMap;

use crate::comparator::value_cmp;
use crate::datom::Value;

/// A query variable, e.g. `?name` or `?e`.
pub type Var = String;

/// A single tuple in a relation — positional values indexed by `Relation.attrs`.
pub type Tuple = Vec<Value>;

/// A relation: a set of tuples with named columns.
///
/// Mirrors the CLJS `(defrecord Relation [attrs tuples])` from `query.cljc`.
/// `attrs` maps variable names to column indices within each tuple.
#[derive(Clone, Debug)]
pub struct Relation {
    pub attrs: HashMap<Var, usize>,
    pub tuples: Vec<Tuple>,
}

impl Relation {
    pub fn new(attrs: HashMap<Var, usize>, tuples: Vec<Tuple>) -> Self {
        Self { attrs, tuples }
    }

    pub fn empty(vars: &[Var]) -> Self {
        let attrs = vars.iter().cloned().enumerate().map(|(i, v)| (v, i)).collect();
        Self { attrs, tuples: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.tuples.is_empty()
    }

    pub fn width(&self) -> usize {
        self.attrs.len()
    }
}

// ---------------------------------------------------------------------------
// Pattern elements and clause types for Datalog evaluation
// ---------------------------------------------------------------------------

/// A pattern element: either a concrete value or a variable to bind.
#[derive(Clone, Debug, PartialEq)]
pub enum PatternEl {
    /// A bound variable name (e.g. "?e", "?name")
    Var(String),
    /// A concrete value to match
    Const(Value),
    /// Wildcard — match anything, don't bind
    Blank,
}

/// A clause in a query WHERE or rule body.
#[derive(Clone, Debug)]
pub enum Clause {
    Pattern([PatternEl; 4]),
    RuleCall { name: String, args: Vec<PatternEl> },
    Predicate { name: String, args: Vec<PatternEl> },
    And(Vec<Clause>),
    Or(Vec<Vec<Clause>>),
    Not(Vec<Clause>),
}

/// A single rule branch: head args + body clauses.
#[derive(Clone, Debug)]
pub struct RuleBranch {
    pub head_args: Vec<Var>,
    pub body: Vec<Clause>,
}

/// Named rules: maps rule name → branches.
pub type Rules = HashMap<String, Vec<RuleBranch>>;

/// Trait for resolving patterns against a database.
/// Allows the rule engine to be tested with `DataScriptDB` (native)
/// and used with `WasmDataScript` (WASM) without coupling.
pub trait PatternResolver {
    fn resolve_pattern(&self, pattern: &[PatternEl; 4]) -> Relation;
}

/// A unit relation — no variables, one empty tuple. Identity for `prod_rel`.
pub fn unit_rel() -> Relation {
    Relation {
        attrs: HashMap::new(),
        tuples: vec![vec![]],
    }
}

// ---------------------------------------------------------------------------
// Key extraction for hash-join
// ---------------------------------------------------------------------------

/// Extract a composite key from a tuple for the given column indices.
/// Returns a `Vec<Value>` usable as a HashMap key (Value implements Hash+Eq).
fn extract_key(tuple: &Tuple, indices: &[usize]) -> Vec<Value> {
    indices.iter().map(|&i| tuple[i].clone()).collect()
}

// ---------------------------------------------------------------------------
// Hash-join
// ---------------------------------------------------------------------------

/// Equi-join two relations on their shared variables.
///
/// Mirrors CLJS `hash-join` (query.cljc:345-376):
/// 1. Find common variables
/// 2. Hash-group `rel1` by common keys
/// 3. Probe with `rel2` tuples
/// 4. Output tuples have union of all variables
///
/// If no shared variables, produces a cartesian product.
pub fn hash_join(rel1: &Relation, rel2: &Relation) -> Relation {
    let common: Vec<Var> = rel1
        .attrs
        .keys()
        .filter(|k| rel2.attrs.contains_key(*k))
        .cloned()
        .collect();

    if common.is_empty() {
        return prod_rel(rel1, rel2);
    }

    // Columns to keep from each relation:
    // - All columns from rel1
    // - Only non-common columns from rel2
    let keep_vars1: Vec<Var> = {
        let mut v: Vec<_> = rel1.attrs.iter().collect();
        v.sort_by_key(|(_, &idx)| idx);
        v.into_iter().map(|(var, _)| var.clone()).collect()
    };
    let keep_vars2: Vec<Var> = {
        let mut v: Vec<_> = rel2
            .attrs
            .iter()
            .filter(|(k, _)| !rel1.attrs.contains_key(*k))
            .collect();
        v.sort_by_key(|(_, &idx)| idx);
        v.into_iter().map(|(var, _)| var.clone()).collect()
    };

    let keep_idxs1: Vec<usize> = keep_vars1.iter().map(|v| rel1.attrs[v]).collect();
    let keep_idxs2: Vec<usize> = keep_vars2.iter().map(|v| rel2.attrs[v]).collect();

    let common_idxs1: Vec<usize> = common.iter().map(|v| rel1.attrs[v]).collect();
    let common_idxs2: Vec<usize> = common.iter().map(|v| rel2.attrs[v]).collect();

    // Build hash table from rel1
    let mut hash: HashMap<Vec<Value>, Vec<&Tuple>> = HashMap::new();
    for t in &rel1.tuples {
        let key = extract_key(t, &common_idxs1);
        hash.entry(key).or_default().push(t);
    }

    // Probe with rel2
    let out_width = keep_vars1.len() + keep_vars2.len();
    let mut result_tuples = Vec::new();

    for t2 in &rel2.tuples {
        let key = extract_key(t2, &common_idxs2);
        if let Some(matches) = hash.get(&key) {
            for t1 in matches {
                let mut out = Vec::with_capacity(out_width);
                for &i in &keep_idxs1 {
                    out.push(t1[i].clone());
                }
                for &i in &keep_idxs2 {
                    out.push(t2[i].clone());
                }
                result_tuples.push(out);
            }
        }
    }

    // Build output attrs
    let out_attrs: HashMap<Var, usize> = keep_vars1
        .iter()
        .chain(keep_vars2.iter())
        .enumerate()
        .map(|(i, v)| (v.clone(), i))
        .collect();

    Relation {
        attrs: out_attrs,
        tuples: result_tuples,
    }
}

// ---------------------------------------------------------------------------
// Sum (union)
// ---------------------------------------------------------------------------

/// Union two relations with the same variable schema.
///
/// Mirrors CLJS `sum-rel` (query.cljc:130-167).
/// If attrs differ in ordering, remaps rel2's tuples to match rel1's layout.
pub fn sum_rel(a: &Relation, b: &Relation) -> Relation {
    if a.tuples.is_empty() {
        return b.clone();
    }
    if b.tuples.is_empty() {
        return a.clone();
    }

    if a.attrs == b.attrs {
        // Same layout — just concatenate
        let mut tuples = a.tuples.clone();
        tuples.extend_from_slice(&b.tuples);
        return Relation {
            attrs: a.attrs.clone(),
            tuples,
        };
    }

    // Different column order — remap b's tuples to a's layout
    let remap: Vec<usize> = {
        let mut v = vec![0; a.attrs.len()];
        for (var, &idx_a) in &a.attrs {
            if let Some(&idx_b) = b.attrs.get(var) {
                v[idx_a] = idx_b;
            }
        }
        v
    };

    let mut tuples = a.tuples.clone();
    for t_b in &b.tuples {
        let mut remapped = vec![Value::Nil; a.attrs.len()];
        for (idx_a, &idx_b) in remap.iter().enumerate() {
            remapped[idx_a] = t_b[idx_b].clone();
        }
        tuples.push(remapped);
    }

    Relation {
        attrs: a.attrs.clone(),
        tuples,
    }
}

// ---------------------------------------------------------------------------
// Cartesian product
// ---------------------------------------------------------------------------

/// Cartesian product of two relations (no shared variables).
///
/// Mirrors CLJS `prod-rel` (query.cljc:169-185).
pub fn prod_rel(rel1: &Relation, rel2: &Relation) -> Relation {
    let vars1: Vec<Var> = {
        let mut v: Vec<_> = rel1.attrs.iter().collect();
        v.sort_by_key(|(_, &idx)| idx);
        v.into_iter().map(|(var, _)| var.clone()).collect()
    };
    let vars2: Vec<Var> = {
        let mut v: Vec<_> = rel2.attrs.iter().collect();
        v.sort_by_key(|(_, &idx)| idx);
        v.into_iter().map(|(var, _)| var.clone()).collect()
    };

    let out_attrs: HashMap<Var, usize> = vars1
        .iter()
        .chain(vars2.iter())
        .enumerate()
        .map(|(i, v)| (v.clone(), i))
        .collect();

    let idxs1: Vec<usize> = vars1.iter().map(|v| rel1.attrs[v]).collect();
    let idxs2: Vec<usize> = vars2.iter().map(|v| rel2.attrs[v]).collect();

    let mut tuples = Vec::with_capacity(rel1.tuples.len() * rel2.tuples.len());
    for t1 in &rel1.tuples {
        for t2 in &rel2.tuples {
            let mut out = Vec::with_capacity(idxs1.len() + idxs2.len());
            for &i in &idxs1 {
                out.push(t1[i].clone());
            }
            for &i in &idxs2 {
                out.push(t2[i].clone());
            }
            tuples.push(out);
        }
    }

    Relation {
        attrs: out_attrs,
        tuples,
    }
}

// ---------------------------------------------------------------------------
// Set difference (for `not` clauses)
// ---------------------------------------------------------------------------

/// Remove from `a` all tuples whose shared-variable projection appears in `b`.
///
/// Mirrors CLJS `subtract-rel` (query.cljc:378-386).
pub fn subtract_rel(a: &Relation, b: &Relation) -> Relation {
    let common: Vec<Var> = a
        .attrs
        .keys()
        .filter(|k| b.attrs.contains_key(*k))
        .cloned()
        .collect();

    if common.is_empty() {
        // No shared vars — subtraction has no effect
        return a.clone();
    }

    let common_idxs_a: Vec<usize> = common.iter().map(|v| a.attrs[v]).collect();
    let common_idxs_b: Vec<usize> = common.iter().map(|v| b.attrs[v]).collect();

    // Hash b's keys
    let mut b_keys: std::collections::HashSet<Vec<Value>> =
        std::collections::HashSet::with_capacity(b.tuples.len());
    for t in &b.tuples {
        b_keys.insert(extract_key(t, &common_idxs_b));
    }

    // Filter a
    let tuples: Vec<Tuple> = a
        .tuples
        .iter()
        .filter(|t| !b_keys.contains(&extract_key(t, &common_idxs_a)))
        .cloned()
        .collect();

    Relation {
        attrs: a.attrs.clone(),
        tuples,
    }
}

// ---------------------------------------------------------------------------
// Collapse: fold a new relation into an accumulator
// ---------------------------------------------------------------------------

/// Fold `new_rel` into `rels` by hash-joining with any relation that shares
/// variables, passing through disjoint relations unchanged.
///
/// Mirrors CLJS `collapse-rels` (query.cljc:457-465).
pub fn collapse_rels(rels: &[Relation], new_rel: Relation) -> Vec<Relation> {
    let mut acc = Vec::new();
    let mut joined = new_rel;

    for rel in rels {
        let has_common = rel.attrs.keys().any(|k| joined.attrs.contains_key(k));
        if has_common {
            joined = hash_join(rel, &joined);
        } else {
            acc.push(rel.clone());
        }
    }

    acc.push(joined);
    acc
}

/// Project a relation down to only the specified variables.
pub fn project(rel: &Relation, vars: &[Var]) -> Relation {
    let indices: Vec<usize> = vars
        .iter()
        .filter_map(|v| rel.attrs.get(v).copied())
        .collect();
    let kept_vars: Vec<&Var> = vars
        .iter()
        .filter(|v| rel.attrs.contains_key(*v))
        .collect();

    let out_attrs: HashMap<Var, usize> = kept_vars
        .iter()
        .enumerate()
        .map(|(i, &&ref v)| (v.clone(), i))
        .collect();

    let tuples: Vec<Tuple> = rel
        .tuples
        .iter()
        .map(|t| indices.iter().map(|&i| t[i].clone()).collect())
        .collect();

    Relation {
        attrs: out_attrs,
        tuples,
    }
}

// ---------------------------------------------------------------------------
// Rule evaluation: alpha-rename, guards, solve_rule, resolve_clause
// ---------------------------------------------------------------------------

/// Alpha-rename a rule branch: substitute head args with call args,
/// and rename internal variables with `__auto__N` suffix to prevent capture.
fn alpha_rename(
    branch: &RuleBranch,
    call_args: &[PatternEl],
    seq_id: usize,
) -> Vec<Clause> {
    // Build substitution map: head_arg → call_arg
    let mut subst: HashMap<String, PatternEl> = HashMap::new();
    for (head_var, call_el) in branch.head_args.iter().zip(call_args.iter()) {
        subst.insert(head_var.clone(), call_el.clone());
    }

    let suffix = format!("__auto__{}", seq_id);

    fn rename_el(
        el: &PatternEl,
        subst: &HashMap<String, PatternEl>,
        suffix: &str,
    ) -> PatternEl {
        match el {
            PatternEl::Var(v) => {
                if let Some(replacement) = subst.get(v) {
                    replacement.clone()
                } else {
                    // Internal variable — alpha-rename
                    PatternEl::Var(format!("{}{}", v, suffix))
                }
            }
            other => other.clone(),
        }
    }

    fn rename_clause(
        clause: &Clause,
        subst: &HashMap<String, PatternEl>,
        suffix: &str,
    ) -> Clause {
        match clause {
            Clause::Pattern(p) => Clause::Pattern([
                rename_el(&p[0], subst, suffix),
                rename_el(&p[1], subst, suffix),
                rename_el(&p[2], subst, suffix),
                rename_el(&p[3], subst, suffix),
            ]),
            Clause::RuleCall { name, args } => Clause::RuleCall {
                name: name.clone(),
                args: args.iter().map(|a| rename_el(a, subst, suffix)).collect(),
            },
            Clause::Predicate { name, args } => Clause::Predicate {
                name: name.clone(),
                args: args.iter().map(|a| rename_el(a, subst, suffix)).collect(),
            },
            Clause::And(cs) => Clause::And(
                cs.iter().map(|c| rename_clause(c, subst, suffix)).collect(),
            ),
            Clause::Or(branches) => Clause::Or(
                branches
                    .iter()
                    .map(|b| b.iter().map(|c| rename_clause(c, subst, suffix)).collect())
                    .collect(),
            ),
            Clause::Not(cs) => Clause::Not(
                cs.iter().map(|c| rename_clause(c, subst, suffix)).collect(),
            ),
        }
    }

    branch
        .body
        .iter()
        .map(|c| rename_clause(c, &subst, &suffix))
        .collect()
}

/// Maximum recursion depth for rule evaluation.
/// Prevents infinite loops on cyclic data. Sufficient for deep hierarchies.
const MAX_RULE_DEPTH: usize = 64;

/// Apply a predicate filter to the context relations.
fn apply_predicate(ctx: &mut Vec<Relation>, name: &str, args: &[PatternEl]) {
    // Find the relation that contains the vars referenced by the predicate
    let pred_vars: Vec<&str> = args
        .iter()
        .filter_map(|el| match el {
            PatternEl::Var(v) => Some(v.as_str()),
            _ => None,
        })
        .collect();

    if pred_vars.is_empty() {
        return;
    }

    // Find which relation(s) contain these vars and apply filter in-place
    for rel in ctx.iter_mut() {
        let has_all = pred_vars.iter().all(|v| rel.attrs.contains_key(*v));
        if !has_all {
            continue;
        }

        let filtered = apply_predicate_filter(rel, name, args);
        *rel = filtered;
    }
}

/// Apply a predicate filter to a single relation.
fn apply_predicate_filter(rel: &Relation, name: &str, args: &[PatternEl]) -> Relation {
    use std::cmp::Ordering;

    let tuples: Vec<Tuple> = rel
        .tuples
        .iter()
        .filter(|tuple| {
            // Resolve predicate arguments to actual values
            let resolved: Vec<Value> = args
                .iter()
                .map(|el| match el {
                    PatternEl::Var(v) => {
                        if let Some(&idx) = rel.attrs.get(v) {
                            tuple[idx].clone()
                        } else {
                            Value::Nil
                        }
                    }
                    PatternEl::Const(v) => v.clone(),
                    PatternEl::Blank => Value::Nil,
                })
                .collect();

            match name {
                "number?" => matches!(
                    &resolved[0],
                    Value::Long(_) | Value::Double(_) | Value::Ref(_)
                ),
                ">" => resolved.len() >= 2
                    && value_cmp(&resolved[0], &resolved[1]) == Ordering::Greater,
                "<" => resolved.len() >= 2
                    && value_cmp(&resolved[0], &resolved[1]) == Ordering::Less,
                ">=" => resolved.len() >= 2
                    && value_cmp(&resolved[0], &resolved[1]) != Ordering::Less,
                "<=" => resolved.len() >= 2
                    && value_cmp(&resolved[0], &resolved[1]) != Ordering::Greater,
                "=" => resolved.len() >= 2 && resolved[0] == resolved[1],
                "!=" => resolved.len() >= 2 && resolved[0] != resolved[1],
                "-differ?" => {
                    // At least one pair of args differs
                    if resolved.len() < 2 {
                        true
                    } else {
                        let half = resolved.len() / 2;
                        (0..half).any(|i| resolved[i] != resolved[half + i])
                    }
                }
                _ => true, // Unknown predicate — pass through
            }
        })
        .cloned()
        .collect();

    Relation::new(rel.attrs.clone(), tuples)
}

/// Collapse a context of relations into a single relation.
fn collapse_ctx(ctx: Vec<Relation>) -> Relation {
    if ctx.is_empty() {
        unit_rel()
    } else if ctx.len() == 1 {
        ctx.into_iter().next().unwrap()
    } else {
        ctx.into_iter()
            .reduce(|a, b| hash_join(&a, &b))
            .unwrap()
    }
}

/// Depth-limited rule evaluation. For each branch of the rule, alpha-renames
/// the body and resolves all clauses (including nested rule calls) recursively.
///
/// Termination is guaranteed by:
/// 1. Depth limit (`MAX_RULE_DEPTH`) prevents infinite recursion
/// 2. On finite data, pattern resolution eventually yields empty relations
pub fn solve_rule<R: PatternResolver>(
    resolver: &R,
    rule_name: &str,
    call_args: &[PatternEl],
    rules: &Rules,
) -> Relation {
    solve_rule_depth(resolver, rule_name, call_args, rules, 0, &mut 0)
}

fn solve_rule_depth<R: PatternResolver>(
    resolver: &R,
    rule_name: &str,
    call_args: &[PatternEl],
    rules: &Rules,
    depth: usize,
    seq_id: &mut usize,
) -> Relation {
    if depth >= MAX_RULE_DEPTH {
        return Relation::empty(&[]);
    }

    let branches = match rules.get(rule_name) {
        Some(b) => b,
        None => return Relation::empty(&[]),
    };

    let final_vars: Vec<Var> = call_args
        .iter()
        .filter_map(|el| match el {
            PatternEl::Var(v) => Some(v.clone()),
            _ => None,
        })
        .collect();

    let mut result = Relation::empty(&final_vars);

    for branch in branches {
        let renamed = alpha_rename(branch, call_args, *seq_id);
        *seq_id += 1;

        let mut ctx: Vec<Relation> = Vec::new();
        let mut dead = false;

        for clause in &renamed {
            resolve_clause_depth(resolver, &mut ctx, clause, rules, depth, seq_id);
            if ctx.iter().any(|r| r.is_empty() && !r.attrs.is_empty()) {
                dead = true;
                break;
            }
        }

        if !dead {
            let resolved = collapse_ctx(ctx);
            if !resolved.is_empty() {
                let projected = project(&resolved, &final_vars);
                result = sum_rel(&result, &projected);
            }
        }
    }

    result
}

/// Dispatch a single clause, updating the context relations.
///
/// Handles all clause types: Pattern, RuleCall, And, Or, Not, Predicate.
pub fn resolve_clause<R: PatternResolver>(
    resolver: &R,
    ctx: &mut Vec<Relation>,
    clause: &Clause,
    rules: &Rules,
) {
    resolve_clause_depth(resolver, ctx, clause, rules, 0, &mut 0);
}

fn resolve_clause_depth<R: PatternResolver>(
    resolver: &R,
    ctx: &mut Vec<Relation>,
    clause: &Clause,
    rules: &Rules,
    depth: usize,
    seq_id: &mut usize,
) {
    match clause {
        Clause::Pattern(p) => {
            let rel = resolver.resolve_pattern(p);
            *ctx = collapse_rels(ctx, rel);
        }
        Clause::RuleCall { name, args } => {
            let rel = solve_rule_depth(resolver, name, args, rules, depth + 1, seq_id);
            *ctx = collapse_rels(ctx, rel);
        }
        Clause::And(cs) => {
            for c in cs {
                resolve_clause_depth(resolver, ctx, c, rules, depth, seq_id);
            }
        }
        Clause::Or(branches) => {
            let mut union = Relation::empty(&[]);
            let mut first = true;
            for branch in branches {
                let mut branch_ctx = ctx.clone();
                for c in branch {
                    resolve_clause_depth(resolver, &mut branch_ctx, c, rules, depth, seq_id);
                }
                let branch_rel = collapse_ctx(branch_ctx);
                if first {
                    union = branch_rel;
                    first = false;
                } else {
                    union = sum_rel(&union, &branch_rel);
                }
            }
            *ctx = vec![union];
        }
        Clause::Not(cs) => {
            let mut neg_ctx = ctx.clone();
            for c in cs {
                resolve_clause_depth(resolver, &mut neg_ctx, c, rules, depth, seq_id);
            }
            let negation = collapse_ctx(neg_ctx);
            let combined = collapse_ctx(ctx.clone());
            *ctx = vec![subtract_rel(&combined, &negation)];
        }
        Clause::Predicate { name, args } => {
            apply_predicate(ctx, name, args);
        }
    }
}

/// Resolve a full query: a list of clauses (which may include rule calls)
/// against a resolver and a set of rules.
pub fn resolve_query<R: PatternResolver>(
    resolver: &R,
    clauses: &[Clause],
    rules: &Rules,
) -> Relation {
    let mut ctx: Vec<Relation> = Vec::new();
    let mut seq_id: usize = 0;
    for clause in clauses {
        resolve_clause_depth(resolver, &mut ctx, clause, rules, 0, &mut seq_id);
    }
    collapse_ctx(ctx)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::{Attr, Value};

    fn v_long(n: i64) -> Value {
        Value::Long(n)
    }
    fn v_str(s: &str) -> Value {
        Value::Str(s.to_string())
    }
    fn v_ref(n: i64) -> Value {
        Value::Ref(n)
    }

    fn attrs(pairs: &[(&str, usize)]) -> HashMap<Var, usize> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    // === unit_rel ===

    #[test]
    fn unit_rel_has_one_empty_tuple() {
        let r = unit_rel();
        assert!(r.attrs.is_empty());
        assert_eq!(r.tuples.len(), 1);
        assert!(r.tuples[0].is_empty());
    }

    // === hash_join: no common vars (cartesian product) ===

    #[test]
    fn hash_join_no_common_vars() {
        let r1 = Relation::new(
            attrs(&[("?a", 0)]),
            vec![vec![v_long(1)], vec![v_long(2)]],
        );
        let r2 = Relation::new(
            attrs(&[("?b", 0)]),
            vec![vec![v_str("x")], vec![v_str("y")]],
        );
        let joined = hash_join(&r1, &r2);
        assert_eq!(joined.tuples.len(), 4); // 2 × 2
        assert!(joined.attrs.contains_key("?a"));
        assert!(joined.attrs.contains_key("?b"));
    }

    // === hash_join: one common var ===

    #[test]
    fn hash_join_one_common_var() {
        // rel1: ?e → ?name
        let r1 = Relation::new(
            attrs(&[("?e", 0), ("?name", 1)]),
            vec![
                vec![v_long(1), v_str("Alice")],
                vec![v_long(2), v_str("Bob")],
                vec![v_long(3), v_str("Carol")],
            ],
        );
        // rel2: ?e → ?age
        let r2 = Relation::new(
            attrs(&[("?e", 0), ("?age", 1)]),
            vec![
                vec![v_long(1), v_long(30)],
                vec![v_long(2), v_long(25)],
                // entity 3 has no age → dropped from join
            ],
        );
        let joined = hash_join(&r1, &r2);

        assert_eq!(joined.tuples.len(), 2);
        assert!(joined.attrs.contains_key("?e"));
        assert!(joined.attrs.contains_key("?name"));
        assert!(joined.attrs.contains_key("?age"));
        assert_eq!(joined.width(), 3);

        // Verify values
        let e_idx = joined.attrs["?e"];
        let name_idx = joined.attrs["?name"];
        let age_idx = joined.attrs["?age"];
        for t in &joined.tuples {
            match &t[e_idx] {
                Value::Long(1) => {
                    assert_eq!(t[name_idx], v_str("Alice"));
                    assert_eq!(t[age_idx], v_long(30));
                }
                Value::Long(2) => {
                    assert_eq!(t[name_idx], v_str("Bob"));
                    assert_eq!(t[age_idx], v_long(25));
                }
                other => panic!("unexpected entity {:?}", other),
            }
        }
    }

    // === hash_join: multiple common vars ===

    #[test]
    fn hash_join_two_common_vars() {
        let r1 = Relation::new(
            attrs(&[("?e", 0), ("?a", 1), ("?v1", 2)]),
            vec![
                vec![v_long(1), v_str("name"), v_str("Alice")],
                vec![v_long(1), v_str("age"), v_long(30)],
                vec![v_long(2), v_str("name"), v_str("Bob")],
            ],
        );
        let r2 = Relation::new(
            attrs(&[("?e", 0), ("?a", 1), ("?v2", 2)]),
            vec![
                vec![v_long(1), v_str("name"), v_str("ALICE")],
                vec![v_long(1), v_str("age"), v_long(31)],
            ],
        );
        let joined = hash_join(&r1, &r2);
        // Only (1, "name") and (1, "age") match
        assert_eq!(joined.tuples.len(), 2);
        assert!(joined.attrs.contains_key("?v1"));
        assert!(joined.attrs.contains_key("?v2"));
    }

    // === hash_join: empty relations ===

    #[test]
    fn hash_join_empty_left() {
        let r1 = Relation::empty(&["?e".into(), "?name".into()]);
        let r2 = Relation::new(
            attrs(&[("?e", 0), ("?age", 1)]),
            vec![vec![v_long(1), v_long(30)]],
        );
        let joined = hash_join(&r1, &r2);
        assert!(joined.is_empty());
    }

    #[test]
    fn hash_join_empty_right() {
        let r1 = Relation::new(
            attrs(&[("?e", 0), ("?name", 1)]),
            vec![vec![v_long(1), v_str("Alice")]],
        );
        let r2 = Relation::empty(&["?e".into(), "?age".into()]);
        let joined = hash_join(&r1, &r2);
        assert!(joined.is_empty());
    }

    // === hash_join: no matches ===

    #[test]
    fn hash_join_no_matching_keys() {
        let r1 = Relation::new(
            attrs(&[("?e", 0)]),
            vec![vec![v_long(1)], vec![v_long(2)]],
        );
        let r2 = Relation::new(
            attrs(&[("?e", 0)]),
            vec![vec![v_long(3)], vec![v_long(4)]],
        );
        let joined = hash_join(&r1, &r2);
        assert!(joined.is_empty());
    }

    // === hash_join: duplicate keys (fan-out) ===

    #[test]
    fn hash_join_fan_out() {
        // rel1 has two tuples with ?e=1
        let r1 = Relation::new(
            attrs(&[("?e", 0), ("?x", 1)]),
            vec![
                vec![v_long(1), v_str("a")],
                vec![v_long(1), v_str("b")],
            ],
        );
        let r2 = Relation::new(
            attrs(&[("?e", 0), ("?y", 1)]),
            vec![
                vec![v_long(1), v_str("X")],
                vec![v_long(1), v_str("Y")],
            ],
        );
        let joined = hash_join(&r1, &r2);
        // 2 × 2 = 4 output tuples
        assert_eq!(joined.tuples.len(), 4);
    }

    // === sum_rel ===

    #[test]
    fn sum_rel_same_attrs() {
        let r1 = Relation::new(
            attrs(&[("?e", 0), ("?n", 1)]),
            vec![vec![v_long(1), v_str("Alice")]],
        );
        let r2 = Relation::new(
            attrs(&[("?e", 0), ("?n", 1)]),
            vec![vec![v_long(2), v_str("Bob")]],
        );
        let summed = sum_rel(&r1, &r2);
        assert_eq!(summed.tuples.len(), 2);
    }

    #[test]
    fn sum_rel_empty_left() {
        let r1 = Relation::empty(&["?e".into()]);
        let r2 = Relation::new(attrs(&[("?e", 0)]), vec![vec![v_long(1)]]);
        let summed = sum_rel(&r1, &r2);
        assert_eq!(summed.tuples.len(), 1);
    }

    #[test]
    fn sum_rel_empty_right() {
        let r1 = Relation::new(attrs(&[("?e", 0)]), vec![vec![v_long(1)]]);
        let r2 = Relation::empty(&["?e".into()]);
        let summed = sum_rel(&r1, &r2);
        assert_eq!(summed.tuples.len(), 1);
    }

    #[test]
    fn sum_rel_reordered_attrs() {
        let r1 = Relation::new(
            attrs(&[("?a", 0), ("?b", 1)]),
            vec![vec![v_long(1), v_long(2)]],
        );
        let r2 = Relation::new(
            attrs(&[("?b", 0), ("?a", 1)]),
            vec![vec![v_long(20), v_long(10)]],
        );
        let summed = sum_rel(&r1, &r2);
        assert_eq!(summed.tuples.len(), 2);
        // Both tuples should follow r1's layout: ?a at 0, ?b at 1
        let a_idx = summed.attrs["?a"];
        let b_idx = summed.attrs["?b"];
        assert_eq!(summed.tuples[0][a_idx], v_long(1));
        assert_eq!(summed.tuples[0][b_idx], v_long(2));
        assert_eq!(summed.tuples[1][a_idx], v_long(10));
        assert_eq!(summed.tuples[1][b_idx], v_long(20));
    }

    // === prod_rel ===

    #[test]
    fn prod_rel_basic() {
        let r1 = Relation::new(
            attrs(&[("?x", 0)]),
            vec![vec![v_long(1)], vec![v_long(2)]],
        );
        let r2 = Relation::new(
            attrs(&[("?y", 0)]),
            vec![vec![v_str("a")], vec![v_str("b")], vec![v_str("c")]],
        );
        let product = prod_rel(&r1, &r2);
        assert_eq!(product.tuples.len(), 6); // 2 × 3
        assert_eq!(product.width(), 2);
    }

    #[test]
    fn prod_rel_with_empty() {
        let r1 = Relation::new(attrs(&[("?x", 0)]), vec![vec![v_long(1)]]);
        let r2 = Relation::empty(&["?y".into()]);
        let product = prod_rel(&r1, &r2);
        assert!(product.is_empty());
    }

    #[test]
    fn prod_rel_unit() {
        let r = Relation::new(
            attrs(&[("?x", 0)]),
            vec![vec![v_long(1)], vec![v_long(2)]],
        );
        let u = unit_rel();
        let product = prod_rel(&r, &u);
        assert_eq!(product.tuples.len(), 2);
        assert_eq!(product.width(), 1); // only ?x
    }

    // === subtract_rel ===

    #[test]
    fn subtract_rel_basic() {
        let a = Relation::new(
            attrs(&[("?e", 0), ("?n", 1)]),
            vec![
                vec![v_long(1), v_str("Alice")],
                vec![v_long(2), v_str("Bob")],
                vec![v_long(3), v_str("Carol")],
            ],
        );
        let b = Relation::new(
            attrs(&[("?e", 0)]),
            vec![vec![v_long(2)]],
        );
        let result = subtract_rel(&a, &b);
        assert_eq!(result.tuples.len(), 2);
        let e_idx = result.attrs["?e"];
        assert!(result.tuples.iter().all(|t| t[e_idx] != v_long(2)));
    }

    #[test]
    fn subtract_rel_no_common_vars() {
        let a = Relation::new(attrs(&[("?x", 0)]), vec![vec![v_long(1)]]);
        let b = Relation::new(attrs(&[("?y", 0)]), vec![vec![v_long(1)]]);
        let result = subtract_rel(&a, &b);
        // No common vars → nothing removed
        assert_eq!(result.tuples.len(), 1);
    }

    #[test]
    fn subtract_rel_all_removed() {
        let a = Relation::new(
            attrs(&[("?e", 0)]),
            vec![vec![v_long(1)], vec![v_long(2)]],
        );
        let b = Relation::new(
            attrs(&[("?e", 0)]),
            vec![vec![v_long(1)], vec![v_long(2)]],
        );
        let result = subtract_rel(&a, &b);
        assert!(result.is_empty());
    }

    #[test]
    fn subtract_rel_empty_b() {
        let a = Relation::new(
            attrs(&[("?e", 0)]),
            vec![vec![v_long(1)], vec![v_long(2)]],
        );
        let b = Relation::empty(&["?e".into()]);
        let result = subtract_rel(&a, &b);
        assert_eq!(result.tuples.len(), 2);
    }

    // === collapse_rels ===

    #[test]
    fn collapse_rels_joins_shared_vars() {
        let existing = vec![Relation::new(
            attrs(&[("?e", 0), ("?name", 1)]),
            vec![
                vec![v_long(1), v_str("Alice")],
                vec![v_long(2), v_str("Bob")],
            ],
        )];
        let new_rel = Relation::new(
            attrs(&[("?e", 0), ("?age", 1)]),
            vec![vec![v_long(1), v_long(30)]],
        );
        let result = collapse_rels(&existing, new_rel);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tuples.len(), 1); // only entity 1 matched
        assert!(result[0].attrs.contains_key("?name"));
        assert!(result[0].attrs.contains_key("?age"));
    }

    #[test]
    fn collapse_rels_passes_through_disjoint() {
        let existing = vec![Relation::new(
            attrs(&[("?x", 0)]),
            vec![vec![v_long(1)]],
        )];
        let new_rel = Relation::new(
            attrs(&[("?y", 0)]),
            vec![vec![v_long(2)]],
        );
        let result = collapse_rels(&existing, new_rel);
        // No shared vars → both kept separately
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn collapse_rels_joins_some_passes_others() {
        let existing = vec![
            Relation::new(
                attrs(&[("?e", 0), ("?name", 1)]),
                vec![vec![v_long(1), v_str("Alice")]],
            ),
            Relation::new(
                attrs(&[("?x", 0)]),
                vec![vec![v_long(99)]],
            ),
        ];
        let new_rel = Relation::new(
            attrs(&[("?e", 0), ("?age", 1)]),
            vec![vec![v_long(1), v_long(30)]],
        );
        let result = collapse_rels(&existing, new_rel);
        assert_eq!(result.len(), 2); // joined (?e) + passed through (?x)
    }

    // === project ===

    #[test]
    fn project_subset() {
        let rel = Relation::new(
            attrs(&[("?e", 0), ("?name", 1), ("?age", 2)]),
            vec![
                vec![v_long(1), v_str("Alice"), v_long(30)],
                vec![v_long(2), v_str("Bob"), v_long(25)],
            ],
        );
        let projected = project(&rel, &["?name".into(), "?age".into()]);
        assert_eq!(projected.width(), 2);
        assert_eq!(projected.tuples.len(), 2);
        assert!(!projected.attrs.contains_key("?e"));
        let name_idx = projected.attrs["?name"];
        assert_eq!(projected.tuples[0][name_idx], v_str("Alice"));
    }

    #[test]
    fn project_nonexistent_var_ignored() {
        let rel = Relation::new(
            attrs(&[("?e", 0)]),
            vec![vec![v_long(1)]],
        );
        let projected = project(&rel, &["?e".into(), "?missing".into()]);
        assert_eq!(projected.width(), 1);
        assert!(projected.attrs.contains_key("?e"));
    }

    // === mixed value types in joins ===

    #[test]
    fn hash_join_with_ref_values() {
        // Simulate: [?e :parent ?p] joined with [?p :name ?n]
        let r1 = Relation::new(
            attrs(&[("?e", 0), ("?p", 1)]),
            vec![
                vec![v_long(1), v_ref(10)],
                vec![v_long(2), v_ref(10)],
                vec![v_long(3), v_ref(20)],
            ],
        );
        // Note: ?p values in r2 must match the type — Ref(10) != Long(10)
        let r2 = Relation::new(
            attrs(&[("?p", 0), ("?n", 1)]),
            vec![
                vec![v_ref(10), v_str("Parent10")],
                vec![v_ref(20), v_str("Parent20")],
            ],
        );
        let joined = hash_join(&r1, &r2);
        assert_eq!(joined.tuples.len(), 3);
    }

    #[test]
    fn hash_join_ref_vs_long_no_match() {
        // Ref(10) should NOT match Long(10)
        let r1 = Relation::new(
            attrs(&[("?x", 0)]),
            vec![vec![v_ref(10)]],
        );
        let r2 = Relation::new(
            attrs(&[("?x", 0)]),
            vec![vec![v_long(10)]],
        );
        let joined = hash_join(&r1, &r2);
        assert!(joined.is_empty(), "Ref(10) != Long(10)");
    }

    // === keyword values ===

    #[test]
    fn hash_join_keyword_values() {
        let kw = Value::Keyword(Attr::Keyword {
            ns: Some("db".into()),
            name: "ident".into(),
        });
        let r1 = Relation::new(
            attrs(&[("?a", 0), ("?extra", 1)]),
            vec![vec![kw.clone(), v_long(1)]],
        );
        let r2 = Relation::new(
            attrs(&[("?a", 0), ("?other", 1)]),
            vec![vec![kw.clone(), v_long(2)]],
        );
        let joined = hash_join(&r1, &r2);
        assert_eq!(joined.tuples.len(), 1);
    }

    // === large join ===

    #[test]
    fn hash_join_100_entities() {
        let r1 = Relation::new(
            attrs(&[("?e", 0), ("?name", 1)]),
            (1..=100)
                .map(|i| vec![v_long(i), v_str(&format!("name{}", i))])
                .collect(),
        );
        let r2 = Relation::new(
            attrs(&[("?e", 0), ("?age", 1)]),
            (1..=100)
                .map(|i| vec![v_long(i), v_long(20 + i)])
                .collect(),
        );
        let joined = hash_join(&r1, &r2);
        assert_eq!(joined.tuples.len(), 100);
        assert_eq!(joined.width(), 3); // ?e, ?name, ?age
    }

    // === chained joins (simulating multi-clause query) ===

    #[test]
    fn chained_collapse_three_patterns() {
        // Simulate: [?e :name ?n] [?e :age ?a] [?e :email ?m]
        let r1 = Relation::new(
            attrs(&[("?e", 0), ("?n", 1)]),
            vec![
                vec![v_long(1), v_str("Alice")],
                vec![v_long(2), v_str("Bob")],
            ],
        );
        let r2 = Relation::new(
            attrs(&[("?e", 0), ("?a", 1)]),
            vec![
                vec![v_long(1), v_long(30)],
                vec![v_long(2), v_long(25)],
            ],
        );
        let r3 = Relation::new(
            attrs(&[("?e", 0), ("?m", 1)]),
            vec![vec![v_long(1), v_str("a@b.com")]],
            // entity 2 has no email
        );

        let rels = collapse_rels(&[], r1);
        let rels = collapse_rels(&rels, r2);
        let rels = collapse_rels(&rels, r3);

        assert_eq!(rels.len(), 1);
        let final_rel = &rels[0];
        assert_eq!(final_rel.tuples.len(), 1); // only entity 1
        assert_eq!(final_rel.width(), 4); // ?e, ?n, ?a, ?m
    }
}
