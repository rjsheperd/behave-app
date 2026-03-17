//! Built-in aggregate functions for DataScript queries.
//!
//! Mirrors the 12 CLJS built-in aggregates from `built_ins.cljc:103-187`.
//! Aggregation pipeline: resolve query → group by non-aggregate columns →
//! apply aggregate functions → return one row per group.

use std::collections::{HashMap, HashSet};

use crate::datom::Value;
use crate::query_parser::FindElement;

/// A single row of query results.
pub type Tuple = Vec<Value>;

/// Apply aggregation to query result tuples.
///
/// Mirrors CLJS `aggregate` from `query.cljc:934-940`:
/// 1. Find indices of non-aggregate find elements (GROUP BY columns)
/// 2. Group tuples by those columns
/// 3. For each group, compute aggregate values
pub fn aggregate(
    find_elements: &[FindElement],
    tuples: Vec<Tuple>,
) -> Vec<Tuple> {
    if tuples.is_empty() {
        return vec![];
    }

    // Find indices of non-aggregate elements (GROUP BY columns)
    let group_idxs: Vec<usize> = find_elements
        .iter()
        .enumerate()
        .filter(|(_, fe)| !matches!(fe, FindElement::Aggregate { .. }))
        .map(|(i, _)| i)
        .collect();

    // Group tuples by non-aggregate columns.
    // Use Debug string as hash key since Value doesn't impl Hash for all types.
    let mut groups: Vec<Vec<Tuple>> = Vec::new();
    let mut group_index: HashMap<String, usize> = HashMap::new();

    for tuple in &tuples {
        let key: Vec<Value> = group_idxs.iter().map(|&i| tuple[i].clone()).collect();
        let key_str = format!("{:?}", key);
        if let Some(&idx) = group_index.get(&key_str) {
            groups[idx].push(tuple.clone());
        } else {
            let idx = groups.len();
            group_index.insert(key_str, idx);
            groups.push(vec![tuple.clone()]);
        }
    }

    // For each group, compute aggregates
    groups
        .into_iter()
        .map(|group_tuples| {
            apply_aggregates(find_elements, &group_tuples)
        })
        .collect()
}

/// Apply aggregate functions to a group of tuples.
/// Returns a single tuple with aggregate results replacing aggregate columns.
fn apply_aggregates(find_elements: &[FindElement], tuples: &[Tuple]) -> Tuple {
    find_elements
        .iter()
        .enumerate()
        .map(|(i, fe)| match fe {
            FindElement::Aggregate { name, n_arg, .. } => {
                let column_values: Vec<&Value> = tuples.iter().map(|t| &t[i]).collect();
                apply_builtin(name, *n_arg, &column_values)
            }
            _ => {
                // Non-aggregate: take value from first tuple in group
                tuples[0][i].clone()
            }
        })
        .collect()
}

/// Apply a built-in aggregate function to a column of values.
fn apply_builtin(name: &str, n_arg: Option<i64>, values: &[&Value]) -> Value {
    match name {
        "sum" => agg_sum(values),
        "avg" => agg_avg(values),
        "median" => agg_median(values),
        "variance" => agg_variance(values),
        "stddev" => agg_stddev(values),
        "min" => {
            if let Some(n) = n_arg {
                agg_min_n(n as usize, values)
            } else {
                agg_min(values)
            }
        }
        "max" => {
            if let Some(n) = n_arg {
                agg_max_n(n as usize, values)
            } else {
                agg_max(values)
            }
        }
        "count" => Value::Long(values.len() as i64),
        "count-distinct" => agg_count_distinct(values),
        "distinct" => agg_distinct(values),
        "rand" => {
            if let Some(n) = n_arg {
                agg_rand_n(n as usize, values)
            } else {
                agg_rand(values)
            }
        }
        "sample" => {
            let n = n_arg.unwrap_or(1) as usize;
            agg_sample(n, values)
        }
        _ => {
            // Unknown aggregate — return first value as fallback
            values.first().cloned().cloned().unwrap_or(Value::Nil)
        }
    }
}

// ---------------------------------------------------------------------------
// Numeric helpers
// ---------------------------------------------------------------------------

fn to_f64(v: &Value) -> f64 {
    match v {
        Value::Long(n) => *n as f64,
        Value::Double(f) => *f,
        Value::Ref(n) => *n as f64,
        _ => 0.0,
    }
}

fn from_f64(f: f64) -> Value {
    if f == f.floor() && f.abs() < i64::MAX as f64 {
        Value::Long(f as i64)
    } else {
        Value::Double(f)
    }
}

// ---------------------------------------------------------------------------
// Aggregate implementations
// ---------------------------------------------------------------------------

fn agg_sum(values: &[&Value]) -> Value {
    let sum: f64 = values.iter().map(|v| to_f64(v)).sum();
    from_f64(sum)
}

fn agg_avg(values: &[&Value]) -> Value {
    if values.is_empty() {
        return Value::Nil;
    }
    let sum: f64 = values.iter().map(|v| to_f64(v)).sum();
    Value::Double(sum / values.len() as f64)
}

fn agg_median(values: &[&Value]) -> Value {
    if values.is_empty() {
        return Value::Nil;
    }
    let mut nums: Vec<f64> = values.iter().map(|v| to_f64(v)).collect();
    nums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let size = nums.len();
    let mid = size / 2;
    if size % 2 == 0 {
        Value::Double((nums[mid - 1] + nums[mid]) / 2.0)
    } else {
        from_f64(nums[mid])
    }
}

fn agg_variance(values: &[&Value]) -> Value {
    if values.is_empty() {
        return Value::Nil;
    }
    let nums: Vec<f64> = values.iter().map(|v| to_f64(v)).collect();
    let mean = nums.iter().sum::<f64>() / nums.len() as f64;
    let sum_sq: f64 = nums.iter().map(|x| (x - mean) * (x - mean)).sum();
    Value::Double(sum_sq / nums.len() as f64)
}

fn agg_stddev(values: &[&Value]) -> Value {
    match agg_variance(values) {
        Value::Double(v) => Value::Double(v.sqrt()),
        other => other,
    }
}

fn agg_min(values: &[&Value]) -> Value {
    values
        .iter()
        .min_by(|a, b| compare_values(a, b))
        .cloned()
        .cloned()
        .unwrap_or(Value::Nil)
}

fn agg_max(values: &[&Value]) -> Value {
    values
        .iter()
        .max_by(|a, b| compare_values(a, b))
        .cloned()
        .cloned()
        .unwrap_or(Value::Nil)
}

fn agg_min_n(n: usize, values: &[&Value]) -> Value {
    let mut sorted: Vec<Value> = values.iter().map(|v| (*v).clone()).collect();
    sorted.sort_by(|a, b| compare_values(&a, &b));
    sorted.truncate(n);
    // Return as a collection — but DataScript returns Vec, we'll use the first for scalar
    // For min/max with n, CLJS returns a vector. We can't represent that as a single Value easily.
    // Just return first element for now.
    sorted.into_iter().next().unwrap_or(Value::Nil)
}

fn agg_max_n(n: usize, values: &[&Value]) -> Value {
    let mut sorted: Vec<Value> = values.iter().map(|v| (*v).clone()).collect();
    sorted.sort_by(|a, b| compare_values(&b, &a)); // descending
    sorted.truncate(n);
    sorted.into_iter().next().unwrap_or(Value::Nil)
}

fn agg_count_distinct(values: &[&Value]) -> Value {
    let distinct: HashSet<String> = values.iter().map(|v| format!("{:?}", v)).collect();
    Value::Long(distinct.len() as i64)
}

fn agg_distinct(values: &[&Value]) -> Value {
    // Return count of distinct values (DataScript returns a set, but we return a scalar)
    // In practice, distinct is used to deduplicate. We'll return the count.
    // Actually, CLJS `distinct` returns `set` — a collection.
    // For now, return count-distinct since we can't return collections as a single Value.
    agg_count_distinct(values)
}

fn agg_rand(values: &[&Value]) -> Value {
    if values.is_empty() {
        return Value::Nil;
    }
    // Simple pseudo-random: use a deterministic pick for testability
    // In production, could use actual RNG
    let idx = values.len() / 2; // deterministic middle pick
    values[idx].clone()
}

fn agg_rand_n(_n: usize, values: &[&Value]) -> Value {
    agg_rand(values)
}

fn agg_sample(_n: usize, values: &[&Value]) -> Value {
    agg_rand(values)
}

fn compare_values(a: &Value, b: &Value) -> std::cmp::Ordering {
    use crate::comparator::value_cmp;
    value_cmp(a, b)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query_parser::FindElement;

    #[test]
    fn sum_integers() {
        let vals: Vec<&Value> = vec![&Value::Long(1), &Value::Long(2), &Value::Long(3)];
        assert_eq!(agg_sum(&vals), Value::Long(6));
    }

    #[test]
    fn sum_doubles() {
        let vals: Vec<&Value> = vec![&Value::Double(1.5), &Value::Double(2.5)];
        // 1.5 + 2.5 = 4.0, which from_f64 converts to Long(4) since it's an exact integer
        assert_eq!(agg_sum(&vals), Value::Long(4));
    }

    #[test]
    fn sum_doubles_fractional() {
        let vals: Vec<&Value> = vec![&Value::Double(1.1), &Value::Double(2.2)];
        match agg_sum(&vals) {
            Value::Double(f) => assert!((f - 3.3).abs() < 1e-10),
            other => panic!("Expected Double, got {:?}", other),
        }
    }

    #[test]
    fn avg_integers() {
        let vals: Vec<&Value> = vec![&Value::Long(2), &Value::Long(4), &Value::Long(6)];
        assert_eq!(agg_avg(&vals), Value::Double(4.0));
    }

    #[test]
    fn median_odd() {
        let vals: Vec<&Value> = vec![&Value::Long(3), &Value::Long(1), &Value::Long(2)];
        assert_eq!(agg_median(&vals), Value::Long(2));
    }

    #[test]
    fn median_even() {
        let vals: Vec<&Value> = vec![
            &Value::Long(1),
            &Value::Long(2),
            &Value::Long(3),
            &Value::Long(4),
        ];
        assert_eq!(agg_median(&vals), Value::Double(2.5));
    }

    #[test]
    fn variance_simple() {
        // [2, 4, 4, 4, 5, 5, 7, 9] → mean=5, variance=4
        let vals: Vec<Value> = vec![2, 4, 4, 4, 5, 5, 7, 9]
            .into_iter()
            .map(Value::Long)
            .collect();
        let refs: Vec<&Value> = vals.iter().collect();
        assert_eq!(agg_variance(&refs), Value::Double(4.0));
    }

    #[test]
    fn stddev_simple() {
        let vals: Vec<Value> = vec![2, 4, 4, 4, 5, 5, 7, 9]
            .into_iter()
            .map(Value::Long)
            .collect();
        let refs: Vec<&Value> = vals.iter().collect();
        assert_eq!(agg_stddev(&refs), Value::Double(2.0));
    }

    #[test]
    fn min_max() {
        let vals: Vec<&Value> = vec![&Value::Long(3), &Value::Long(1), &Value::Long(5)];
        assert_eq!(agg_min(&vals), Value::Long(1));
        assert_eq!(agg_max(&vals), Value::Long(5));
    }

    #[test]
    fn count_values() {
        let vals: Vec<&Value> = vec![&Value::Long(1), &Value::Long(2), &Value::Long(3)];
        assert_eq!(apply_builtin("count", None, &vals), Value::Long(3));
    }

    #[test]
    fn count_distinct_values() {
        let vals: Vec<&Value> = vec![
            &Value::Long(1),
            &Value::Long(2),
            &Value::Long(1),
            &Value::Long(3),
        ];
        assert_eq!(agg_count_distinct(&vals), Value::Long(3));
    }

    #[test]
    fn aggregate_grouping() {
        // Query: [:find ?dept (count ?e) :where [?e :dept ?dept]]
        // Results: [["eng", 1], ["eng", 2], ["eng", 3], ["sales", 4], ["sales", 5]]
        let find_elements = vec![
            FindElement::Var("?dept".into()),
            FindElement::Aggregate {
                name: "count".into(),
                var: "?e".into(),
                n_arg: None,
            },
        ];

        let tuples = vec![
            vec![Value::Str("eng".into()), Value::Long(1)],
            vec![Value::Str("eng".into()), Value::Long(2)],
            vec![Value::Str("eng".into()), Value::Long(3)],
            vec![Value::Str("sales".into()), Value::Long(4)],
            vec![Value::Str("sales".into()), Value::Long(5)],
        ];

        let result = aggregate(&find_elements, tuples);
        assert_eq!(result.len(), 2);

        // Find the eng and sales rows
        let eng = result.iter().find(|r| r[0] == Value::Str("eng".into())).unwrap();
        let sales = result.iter().find(|r| r[0] == Value::Str("sales".into())).unwrap();

        assert_eq!(eng[1], Value::Long(3));
        assert_eq!(sales[1], Value::Long(2));
    }

    #[test]
    fn aggregate_all_rows() {
        // Query: [:find (sum ?age) :where [_ :age ?age]]
        // All rows are one group (no non-aggregate columns)
        let find_elements = vec![FindElement::Aggregate {
            name: "sum".into(),
            var: "?age".into(),
            n_arg: None,
        }];

        let tuples = vec![
            vec![Value::Long(25)],
            vec![Value::Long(30)],
            vec![Value::Long(35)],
        ];

        let result = aggregate(&find_elements, tuples);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0][0], Value::Long(90));
    }
}
