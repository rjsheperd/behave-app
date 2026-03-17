//! Function expression evaluation for DataScript queries.
//!
//! Handles `[(fn ?a ?b) ?result]` clauses — evaluates a function against
//! bound variables and binds the result to an output variable.
//!
//! Mirrors CLJS `bind-by-fn` from `query.cljc:533-560` and the 68 built-in
//! functions from `built_ins.cljc:81-99`.

use std::collections::HashMap;

use crate::comparator::value_cmp;
use crate::datom::{Attr, Value};
use crate::relation::{PatternEl, Relation, Tuple};

/// Apply a function expression to the query context.
///
/// For each tuple in the context, evaluates `fn(args...)` and binds the
/// result to `binding_var`. Tuples where the function returns None are dropped.
pub fn apply_fn_expr(
    ctx: &mut Vec<Relation>,
    name: &str,
    args: &[PatternEl],
    binding_var: &str,
) {
    // Collect vars referenced in args
    let fn_vars: Vec<&str> = args
        .iter()
        .filter_map(|el| match el {
            PatternEl::Var(v) => Some(v.as_str()),
            _ => None,
        })
        .collect();

    if fn_vars.is_empty() && args.iter().all(|a| matches!(a, PatternEl::Const(_))) {
        // All constant args — evaluate once and add as a new relation
        let resolved: Vec<Value> = args
            .iter()
            .map(|el| match el {
                PatternEl::Const(v) => v.clone(),
                _ => Value::Nil,
            })
            .collect();
        if let Some(result) = eval_fn(name, &resolved) {
            let mut attrs = HashMap::new();
            attrs.insert(binding_var.to_string(), 0);
            let rel = Relation::new(attrs, vec![vec![result]]);
            ctx.push(rel);
        }
        return;
    }

    // Find which relation contains the referenced vars
    for rel in ctx.iter_mut() {
        let has_any = fn_vars.iter().any(|v| rel.attrs.contains_key(*v));
        if !has_any {
            continue;
        }

        let mut new_attrs = rel.attrs.clone();
        let binding_col = new_attrs.len();
        new_attrs.insert(binding_var.to_string(), binding_col);

        let new_tuples: Vec<Tuple> = rel
            .tuples
            .iter()
            .filter_map(|tuple| {
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

                eval_fn(name, &resolved).map(|result| {
                    let mut new_tuple = tuple.clone();
                    new_tuple.push(result);
                    new_tuple
                })
            })
            .collect();

        *rel = Relation::new(new_attrs, new_tuples);
        return;
    }
}

/// Returns true if the function name is a known built-in.
pub fn is_known_fn(name: &str) -> bool {
    matches!(name,
        "+" | "-" | "*" | "/" | "quot" | "rem" | "mod" | "inc" | "dec" | "abs" | "max" | "min"
        | "str" | "subs" | "count"
        | "name" | "namespace" | "keyword" | "long" | "double"
        | "zero?" | "pos?" | "neg?" | "even?" | "odd?"
        | "true?" | "false?" | "nil?" | "some?" | "number?" | "string?" | "keyword?"
        | "not" | "identity" | "ground"
        | "=" | "==" | "not=" | "!=" | ">" | "<" | ">=" | "<=" | "compare"
        | "re-find" | "re-matches"
        | "clojure.string/blank?" | "clojure.string/includes?"
        | "clojure.string/starts-with?" | "clojure.string/ends-with?"
        | "get" | "contains?" | "empty?"
        | "tuple" | "vector" | "untuple"
    )
}

/// Evaluate a built-in function. Returns None if unknown or on error.
pub fn eval_fn(name: &str, args: &[Value]) -> Option<Value> {
    match name {
        // Arithmetic
        "+" => num_binop(args, |a, b| a + b),
        "-" => {
            if args.len() == 1 {
                Some(negate(&args[0]))
            } else {
                num_binop(args, |a, b| a - b)
            }
        }
        "*" => num_binop(args, |a, b| a * b),
        "/" => {
            if args.len() >= 2 {
                let b = to_f64(&args[1]);
                if b == 0.0 { None } else { Some(Value::Double(to_f64(&args[0]) / b)) }
            } else {
                None
            }
        }
        "quot" => {
            if args.len() >= 2 {
                match (&args[0], &args[1]) {
                    (Value::Long(a), Value::Long(b)) if *b != 0 => Some(Value::Long(a / b)),
                    _ => {
                        let b = to_f64(&args[1]);
                        if b == 0.0 { None } else { Some(Value::Long((to_f64(&args[0]) / b) as i64)) }
                    }
                }
            } else {
                None
            }
        }
        "rem" | "mod" => {
            if args.len() >= 2 {
                match (&args[0], &args[1]) {
                    (Value::Long(a), Value::Long(b)) if *b != 0 => Some(Value::Long(a % b)),
                    _ => {
                        let b = to_f64(&args[1]);
                        if b == 0.0 { None } else { Some(Value::Double(to_f64(&args[0]) % b)) }
                    }
                }
            } else {
                None
            }
        }
        "inc" => args.first().map(|v| match v {
            Value::Long(n) => Value::Long(n + 1),
            Value::Double(f) => Value::Double(f + 1.0),
            _ => Value::Nil,
        }),
        "dec" => args.first().map(|v| match v {
            Value::Long(n) => Value::Long(n - 1),
            Value::Double(f) => Value::Double(f - 1.0),
            _ => Value::Nil,
        }),
        "abs" => args.first().map(|v| match v {
            Value::Long(n) => Value::Long(n.abs()),
            Value::Double(f) => Value::Double(f.abs()),
            _ => Value::Nil,
        }),
        "max" => {
            if args.len() >= 2 {
                args.iter()
                    .max_by(|a, b| value_cmp(a, b))
                    .cloned()
            } else {
                args.first().cloned()
            }
        }
        "min" => {
            if args.len() >= 2 {
                args.iter()
                    .min_by(|a, b| value_cmp(a, b))
                    .cloned()
            } else {
                args.first().cloned()
            }
        }

        // String
        "str" => {
            let s: String = args.iter().map(|v| value_to_string(v)).collect();
            Some(Value::Str(s))
        }
        "subs" => {
            match args.as_ref() {
                [Value::Str(s), Value::Long(start)] => {
                    let start = *start as usize;
                    if start <= s.len() {
                        Some(Value::Str(s[start..].to_string()))
                    } else {
                        Some(Value::Str(String::new()))
                    }
                }
                [Value::Str(s), Value::Long(start), Value::Long(end)] => {
                    let start = *start as usize;
                    let end = (*end as usize).min(s.len());
                    if start <= end && start <= s.len() {
                        Some(Value::Str(s[start..end].to_string()))
                    } else {
                        Some(Value::Str(String::new()))
                    }
                }
                _ => None,
            }
        }
        "count" => {
            match args.first() {
                Some(Value::Str(s)) => Some(Value::Long(s.len() as i64)),
                _ => Some(Value::Long(0)),
            }
        }

        // Type / keyword / name / namespace
        "name" => args.first().map(|v| match v {
            Value::Keyword(Attr::Keyword { name, .. }) => Value::Str(name.clone()),
            Value::Str(s) => Value::Str(s.clone()),
            _ => Value::Nil,
        }),
        "namespace" => args.first().map(|v| match v {
            Value::Keyword(Attr::Keyword { ns: Some(ns), .. }) => Value::Str(ns.clone()),
            Value::Keyword(Attr::Keyword { ns: None, .. }) => Value::Nil,
            _ => Value::Nil,
        }),
        "keyword" => {
            match args.as_ref() {
                [Value::Str(s)] => {
                    let attr = if let Some((ns, name)) = s.split_once('/') {
                        Attr::Keyword { ns: Some(ns.to_string()), name: name.to_string() }
                    } else {
                        Attr::Keyword { ns: None, name: s.clone() }
                    };
                    Some(Value::Keyword(attr))
                }
                [Value::Str(ns), Value::Str(name)] => {
                    Some(Value::Keyword(Attr::Keyword {
                        ns: Some(ns.clone()),
                        name: name.clone(),
                    }))
                }
                [Value::Keyword(_)] => args.first().cloned(),
                _ => None,
            }
        }
        "long" => args.first().map(|v| match v {
            Value::Long(_) => v.clone(),
            Value::Double(f) => Value::Long(*f as i64),
            Value::Str(s) => s.parse::<i64>().map(Value::Long).unwrap_or(Value::Nil),
            _ => Value::Nil,
        }),
        "double" => args.first().map(|v| match v {
            Value::Double(_) => v.clone(),
            Value::Long(n) => Value::Double(*n as f64),
            Value::Str(s) => s.parse::<f64>().map(Value::Double).unwrap_or(Value::Nil),
            _ => Value::Nil,
        }),

        // Predicates (when used as fn-expr, return the value or nil)
        "zero?" => bool_pred(args, |v| to_f64(v) == 0.0),
        "pos?" => bool_pred(args, |v| to_f64(v) > 0.0),
        "neg?" => bool_pred(args, |v| to_f64(v) < 0.0),
        "even?" => bool_pred(args, |v| match v { Value::Long(n) => n % 2 == 0, _ => false }),
        "odd?" => bool_pred(args, |v| match v { Value::Long(n) => n % 2 != 0, _ => false }),
        "true?" => bool_pred(args, |v| matches!(v, Value::Bool(true))),
        "false?" => bool_pred(args, |v| matches!(v, Value::Bool(false))),
        "nil?" => bool_pred(args, |v| matches!(v, Value::Nil)),
        "some?" => bool_pred(args, |v| !matches!(v, Value::Nil)),
        "number?" => bool_pred(args, |v| matches!(v, Value::Long(_) | Value::Double(_) | Value::Ref(_))),
        "string?" => bool_pred(args, |v| matches!(v, Value::Str(_))),
        "keyword?" => bool_pred(args, |v| matches!(v, Value::Keyword(_))),
        "not" => args.first().map(|v| Value::Bool(!is_truthy(v))),
        "identity" | "ground" => args.first().cloned(),

        // Comparison (as functions returning boolean)
        "=" | "==" => {
            if args.len() >= 2 {
                Some(Value::Bool(args[0] == args[1]))
            } else {
                None
            }
        }
        "not=" | "!=" => {
            if args.len() >= 2 {
                Some(Value::Bool(args[0] != args[1]))
            } else {
                None
            }
        }
        ">" => cmp_pred(args, |o| o == std::cmp::Ordering::Greater),
        "<" => cmp_pred(args, |o| o == std::cmp::Ordering::Less),
        ">=" => cmp_pred(args, |o| o != std::cmp::Ordering::Less),
        "<=" => cmp_pred(args, |o| o != std::cmp::Ordering::Greater),
        "compare" => {
            if args.len() >= 2 {
                let o = value_cmp(&args[0], &args[1]);
                Some(Value::Long(match o {
                    std::cmp::Ordering::Less => -1,
                    std::cmp::Ordering::Equal => 0,
                    std::cmp::Ordering::Greater => 1,
                }))
            } else {
                None
            }
        }

        // Regex
        "re-find" => {
            match args.as_ref() {
                [Value::Str(pattern), Value::Str(s)] => {
                    match regex_lite::Regex::new(pattern) {
                        Ok(re) => re.find(s).map(|m| Value::Str(m.as_str().to_string())),
                        Err(_) => None,
                    }
                }
                _ => None,
            }
        }
        "re-matches" => {
            match args.as_ref() {
                [Value::Str(pattern), Value::Str(s)] => {
                    let full_pattern = format!("^(?:{})$", pattern);
                    match regex_lite::Regex::new(&full_pattern) {
                        Ok(re) => {
                            if re.is_match(s) {
                                Some(Value::Str(s.clone()))
                            } else {
                                None
                            }
                        }
                        Err(_) => None,
                    }
                }
                _ => None,
            }
        }

        // clojure.string functions
        "clojure.string/blank?" => bool_pred(args, |v| match v {
            Value::Str(s) => s.trim().is_empty(),
            Value::Nil => true,
            _ => false,
        }),
        "clojure.string/includes?" => {
            match args.as_ref() {
                [Value::Str(s), Value::Str(sub)] => Some(Value::Bool(s.contains(sub.as_str()))),
                _ => None,
            }
        }
        "clojure.string/starts-with?" => {
            match args.as_ref() {
                [Value::Str(s), Value::Str(prefix)] => Some(Value::Bool(s.starts_with(prefix.as_str()))),
                _ => None,
            }
        }
        "clojure.string/ends-with?" => {
            match args.as_ref() {
                [Value::Str(s), Value::Str(suffix)] => Some(Value::Bool(s.ends_with(suffix.as_str()))),
                _ => None,
            }
        }

        // Collection
        "get" => {
            // (get map key) — for our purposes, not applicable to Value
            None
        }
        "contains?" => {
            // (contains? coll key) — not directly applicable
            None
        }
        "empty?" => bool_pred(args, |v| match v {
            Value::Str(s) => s.is_empty(),
            Value::Nil => true,
            _ => false,
        }),

        // DataScript-specific
        "tuple" | "vector" => Some(Value::Nil), // Can't represent tuples as single Value
        "untuple" => args.first().cloned(),

        // Fallback
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn to_f64(v: &Value) -> f64 {
    match v {
        Value::Long(n) => *n as f64,
        Value::Double(f) => *f,
        Value::Ref(n) => *n as f64,
        _ => 0.0,
    }
}

fn negate(v: &Value) -> Value {
    match v {
        Value::Long(n) => Value::Long(-n),
        Value::Double(f) => Value::Double(-f),
        _ => Value::Nil,
    }
}

fn num_binop(args: &[Value], op: fn(f64, f64) -> f64) -> Option<Value> {
    if args.len() < 2 {
        return None;
    }
    let a = to_f64(&args[0]);
    let b = to_f64(&args[1]);
    let result = op(a, b);
    // Preserve integer type when both inputs are integers
    match (&args[0], &args[1]) {
        (Value::Long(_), Value::Long(_)) if result == result.floor() && result.abs() < i64::MAX as f64 => {
            Some(Value::Long(result as i64))
        }
        _ => Some(Value::Double(result)),
    }
}

fn bool_pred(args: &[Value], pred: impl Fn(&Value) -> bool) -> Option<Value> {
    args.first().map(|v| Value::Bool(pred(v)))
}

fn cmp_pred(args: &[Value], pred: impl Fn(std::cmp::Ordering) -> bool) -> Option<Value> {
    if args.len() >= 2 {
        Some(Value::Bool(pred(value_cmp(&args[0], &args[1]))))
    } else {
        None
    }
}

fn is_truthy(v: &Value) -> bool {
    !matches!(v, Value::Nil | Value::Bool(false))
}

fn value_to_string(v: &Value) -> String {
    match v {
        Value::Nil => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Long(n) => n.to_string(),
        Value::Double(f) => {
            let s = f.to_string();
            if s.contains('.') { s } else { format!("{}.0", s) }
        }
        Value::Str(s) => s.clone(),
        Value::Keyword(Attr::Keyword { ns: Some(ns), name }) => format!(":{}/{}", ns, name),
        Value::Keyword(Attr::Keyword { ns: None, name }) => format!(":{}", name),
        Value::Keyword(Attr::Str(s)) => s.clone(),
        Value::Ref(n) => n.to_string(),
        Value::Instant(ms) => format!("#{}", ms),
        Value::Uuid(bytes) => format!("#{:?}", bytes),
        Value::Bytes(bytes) => format!("<bytes:{}>", bytes.len()),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fn_str_concat() {
        let result = eval_fn("str", &[
            Value::Str("hello".into()),
            Value::Str(" ".into()),
            Value::Str("world".into()),
        ]);
        assert_eq!(result, Some(Value::Str("hello world".into())));
    }

    #[test]
    fn fn_str_mixed_types() {
        let result = eval_fn("str", &[
            Value::Str("age: ".into()),
            Value::Long(30),
        ]);
        assert_eq!(result, Some(Value::Str("age: 30".into())));
    }

    #[test]
    fn fn_add() {
        assert_eq!(eval_fn("+", &[Value::Long(1), Value::Long(2)]), Some(Value::Long(3)));
        assert_eq!(eval_fn("+", &[Value::Long(1), Value::Double(2.5)]), Some(Value::Double(3.5)));
    }

    #[test]
    fn fn_subtract() {
        assert_eq!(eval_fn("-", &[Value::Long(5), Value::Long(3)]), Some(Value::Long(2)));
        assert_eq!(eval_fn("-", &[Value::Long(5)]), Some(Value::Long(-5)));
    }

    #[test]
    fn fn_multiply() {
        assert_eq!(eval_fn("*", &[Value::Long(3), Value::Long(4)]), Some(Value::Long(12)));
    }

    #[test]
    fn fn_divide() {
        assert_eq!(eval_fn("/", &[Value::Long(10), Value::Long(3)]), Some(Value::Double(10.0 / 3.0)));
        assert_eq!(eval_fn("/", &[Value::Long(10), Value::Long(0)]), None);
    }

    #[test]
    fn fn_inc_dec() {
        assert_eq!(eval_fn("inc", &[Value::Long(5)]), Some(Value::Long(6)));
        assert_eq!(eval_fn("dec", &[Value::Long(5)]), Some(Value::Long(4)));
    }

    #[test]
    fn fn_name_namespace() {
        let kw = Value::Keyword(Attr::Keyword {
            ns: Some("person".into()),
            name: "name".into(),
        });
        assert_eq!(eval_fn("name", &[kw.clone()]), Some(Value::Str("name".into())));
        assert_eq!(eval_fn("namespace", &[kw]), Some(Value::Str("person".into())));
    }

    #[test]
    fn fn_subs() {
        assert_eq!(
            eval_fn("subs", &[Value::Str("hello".into()), Value::Long(1)]),
            Some(Value::Str("ello".into()))
        );
        assert_eq!(
            eval_fn("subs", &[Value::Str("hello".into()), Value::Long(1), Value::Long(3)]),
            Some(Value::Str("el".into()))
        );
    }

    #[test]
    fn fn_predicates() {
        assert_eq!(eval_fn("zero?", &[Value::Long(0)]), Some(Value::Bool(true)));
        assert_eq!(eval_fn("zero?", &[Value::Long(1)]), Some(Value::Bool(false)));
        assert_eq!(eval_fn("pos?", &[Value::Long(5)]), Some(Value::Bool(true)));
        assert_eq!(eval_fn("neg?", &[Value::Long(-1)]), Some(Value::Bool(true)));
        assert_eq!(eval_fn("even?", &[Value::Long(4)]), Some(Value::Bool(true)));
        assert_eq!(eval_fn("odd?", &[Value::Long(3)]), Some(Value::Bool(true)));
    }

    #[test]
    fn fn_not() {
        assert_eq!(eval_fn("not", &[Value::Bool(true)]), Some(Value::Bool(false)));
        assert_eq!(eval_fn("not", &[Value::Bool(false)]), Some(Value::Bool(true)));
        assert_eq!(eval_fn("not", &[Value::Nil]), Some(Value::Bool(true)));
    }

    #[test]
    fn fn_identity_ground() {
        assert_eq!(eval_fn("identity", &[Value::Long(42)]), Some(Value::Long(42)));
        assert_eq!(eval_fn("ground", &[Value::Str("x".into())]), Some(Value::Str("x".into())));
    }

    #[test]
    fn fn_re_find() {
        assert_eq!(
            eval_fn("re-find", &[Value::Str("\\d+".into()), Value::Str("abc123def".into())]),
            Some(Value::Str("123".into()))
        );
        assert_eq!(
            eval_fn("re-find", &[Value::Str("\\d+".into()), Value::Str("abcdef".into())]),
            None
        );
    }

    #[test]
    fn fn_string_predicates() {
        assert_eq!(
            eval_fn("clojure.string/blank?", &[Value::Str("  ".into())]),
            Some(Value::Bool(true))
        );
        assert_eq!(
            eval_fn("clojure.string/starts-with?", &[Value::Str("hello".into()), Value::Str("hel".into())]),
            Some(Value::Bool(true))
        );
        assert_eq!(
            eval_fn("clojure.string/includes?", &[Value::Str("hello world".into()), Value::Str("world".into())]),
            Some(Value::Bool(true))
        );
    }

    #[test]
    fn fn_keyword() {
        assert_eq!(
            eval_fn("keyword", &[Value::Str("name".into())]),
            Some(Value::Keyword(Attr::Keyword { ns: None, name: "name".into() }))
        );
        assert_eq!(
            eval_fn("keyword", &[Value::Str("person".into()), Value::Str("name".into())]),
            Some(Value::Keyword(Attr::Keyword { ns: Some("person".into()), name: "name".into() }))
        );
    }

    #[test]
    fn apply_fn_expr_basic() {
        // Simulate: [(str ?first " " ?last) ?full]
        // With relation: {?first: 0, ?last: 1} [["Alice", "Smith"], ["Bob", "Jones"]]
        let mut attrs = HashMap::new();
        attrs.insert("?first".to_string(), 0);
        attrs.insert("?last".to_string(), 1);
        let rel = Relation::new(attrs, vec![
            vec![Value::Str("Alice".into()), Value::Str("Smith".into())],
            vec![Value::Str("Bob".into()), Value::Str("Jones".into())],
        ]);
        let mut ctx = vec![rel];

        apply_fn_expr(
            &mut ctx,
            "str",
            &[
                PatternEl::Var("?first".into()),
                PatternEl::Const(Value::Str(" ".into())),
                PatternEl::Var("?last".into()),
            ],
            "?full",
        );

        assert_eq!(ctx.len(), 1);
        assert_eq!(ctx[0].tuples.len(), 2);
        let full_idx = ctx[0].attrs["?full"];
        assert_eq!(ctx[0].tuples[0][full_idx], Value::Str("Alice Smith".into()));
        assert_eq!(ctx[0].tuples[1][full_idx], Value::Str("Bob Jones".into()));
    }

    #[test]
    fn apply_fn_expr_arithmetic() {
        // [(+ ?a ?b) ?sum]
        let mut attrs = HashMap::new();
        attrs.insert("?a".to_string(), 0);
        attrs.insert("?b".to_string(), 1);
        let rel = Relation::new(attrs, vec![
            vec![Value::Long(10), Value::Long(20)],
            vec![Value::Long(3), Value::Long(7)],
        ]);
        let mut ctx = vec![rel];

        apply_fn_expr(
            &mut ctx,
            "+",
            &[PatternEl::Var("?a".into()), PatternEl::Var("?b".into())],
            "?sum",
        );

        let sum_idx = ctx[0].attrs["?sum"];
        assert_eq!(ctx[0].tuples[0][sum_idx], Value::Long(30));
        assert_eq!(ctx[0].tuples[1][sum_idx], Value::Long(10));
    }
}
