use std::cmp::Ordering;
use std::rc::Rc;

use persistent_sorted_set::Key;
use persistent_sorted_set::node::Node;
use persistent_sorted_set::leaf::Leaf;
use persistent_sorted_set::results::{AddResult, RemoveResult};
use persistent_sorted_set::settings::Settings;

fn int_cmp(a: &Key, b: &Key) -> Ordering {
    a.cmp(b)
}

fn small_settings() -> Settings {
    Settings::new(4)
}

fn get_leaf_keys(result: &AddResult) -> Vec<Key> {
    match result {
        AddResult::One(node) => match node.as_ref() {
            Node::Leaf(l) => l.keys().to_vec(),
            _ => panic!("expected leaf"),
        },
        _ => panic!("expected One"),
    }
}

// --- Add tests ---

#[test]
fn test_leaf_add_to_empty() {
    let leaf = Leaf::new(small_settings());
    let result = leaf.add(&5, &int_cmp, &small_settings());
    assert_eq!(get_leaf_keys(&result), vec![5]);
}

#[test]
fn test_leaf_add_sorted_order() {
    let settings = small_settings();
    let leaf = Leaf::with_keys(vec![1, 3], settings.clone());
    let result = leaf.add(&2, &int_cmp, &settings);
    assert_eq!(get_leaf_keys(&result), vec![1, 2, 3]);
}

#[test]
fn test_leaf_add_duplicate() {
    let settings = small_settings();
    let leaf = Leaf::with_keys(vec![1, 3, 5], settings.clone());
    let result = leaf.add(&3, &int_cmp, &settings);
    assert!(matches!(result, AddResult::Unchanged));
}

#[test]
fn test_leaf_add_at_beginning() {
    let settings = small_settings();
    let leaf = Leaf::with_keys(vec![3, 5, 7], settings.clone());
    let result = leaf.add(&1, &int_cmp, &settings);
    assert_eq!(get_leaf_keys(&result), vec![1, 3, 5, 7]);
}

#[test]
fn test_leaf_add_at_end() {
    let settings = small_settings();
    let leaf = Leaf::with_keys(vec![1, 3, 5], settings.clone());
    let result = leaf.add(&7, &int_cmp, &settings);
    assert_eq!(get_leaf_keys(&result), vec![1, 3, 5, 7]);
}

#[test]
fn test_leaf_add_until_split() {
    let settings = small_settings(); // bf=4
    let leaf = Leaf::with_keys(vec![1, 3, 5, 7], settings.clone());
    let result = leaf.add(&4, &int_cmp, &settings);
    match result {
        AddResult::Split(left, right) => {
            let lk = match left.as_ref() {
                Node::Leaf(l) => l.keys(),
                _ => panic!("expected leaf"),
            };
            let rk = match right.as_ref() {
                Node::Leaf(l) => l.keys(),
                _ => panic!("expected leaf"),
            };
            // total 5 elements, half1 = 3, half2 = 2
            assert_eq!(lk.len(), 3);
            assert_eq!(rk.len(), 2);
            // All elements present
            let mut all: Vec<Key> = lk.iter().chain(rk.iter()).copied().collect();
            all.sort();
            assert_eq!(all, vec![1, 3, 4, 5, 7]);
        }
        _ => panic!("expected Split"),
    }
}

#[test]
fn test_leaf_split_distribution() {
    let settings = small_settings(); // bf=4
    let leaf = Leaf::with_keys(vec![1, 2, 3, 4], settings.clone());
    let result = leaf.add(&5, &int_cmp, &settings);
    match result {
        AddResult::Split(left, right) => {
            assert_eq!(left.len(), 3); // half1 = (5+1)/2 = 3
            assert_eq!(right.len(), 2);
        }
        _ => panic!("expected Split"),
    }
}

#[test]
fn test_leaf_split_insert_first_half() {
    let settings = small_settings();
    let leaf = Leaf::with_keys(vec![2, 4, 6, 8], settings.clone());
    let result = leaf.add(&1, &int_cmp, &settings); // ins=0, in first half
    match result {
        AddResult::Split(left, right) => {
            let lk = match left.as_ref() {
                Node::Leaf(l) => l.keys(),
                _ => panic!("expected leaf"),
            };
            let rk = match right.as_ref() {
                Node::Leaf(l) => l.keys(),
                _ => panic!("expected leaf"),
            };
            assert_eq!(lk, &[1, 2, 4]);
            assert_eq!(rk, &[6, 8]);
        }
        _ => panic!("expected Split"),
    }
}

#[test]
fn test_leaf_split_insert_second_half() {
    let settings = small_settings();
    let leaf = Leaf::with_keys(vec![2, 4, 6, 8], settings.clone());
    let result = leaf.add(&7, &int_cmp, &settings); // ins=3, in second half
    match result {
        AddResult::Split(left, right) => {
            let lk = match left.as_ref() {
                Node::Leaf(l) => l.keys(),
                _ => panic!("expected leaf"),
            };
            let rk = match right.as_ref() {
                Node::Leaf(l) => l.keys(),
                _ => panic!("expected leaf"),
            };
            assert_eq!(lk, &[2, 4, 6]);
            assert_eq!(rk, &[7, 8]);
        }
        _ => panic!("expected Split"),
    }
}

// --- Remove tests ---

#[test]
fn test_leaf_remove_only_key() {
    let settings = small_settings();
    let leaf = Leaf::with_keys(vec![5], settings.clone());
    let result = leaf.remove(&5, None, None, &int_cmp, &settings);
    match result {
        RemoveResult::Rebalanced { left, center, right } => {
            assert!(left.is_none());
            assert!(right.is_none());
            assert_eq!(center.len(), 0);
        }
        _ => panic!("expected Rebalanced"),
    }
}

#[test]
fn test_leaf_remove_no_rebalance() {
    let settings = small_settings(); // min_bf = 2
    let leaf = Leaf::with_keys(vec![1, 3, 5], settings.clone());
    let result = leaf.remove(&3, None, None, &int_cmp, &settings);
    match result {
        RemoveResult::Rebalanced { center, .. } => {
            assert_eq!(center.keys(), &[1, 5]);
        }
        _ => panic!("expected Rebalanced"),
    }
}

#[test]
fn test_leaf_remove_not_found() {
    let settings = small_settings();
    let leaf = Leaf::with_keys(vec![1, 3, 5], settings.clone());
    let result = leaf.remove(&4, None, None, &int_cmp, &settings);
    assert!(matches!(result, RemoveResult::Unchanged));
}

#[test]
fn test_leaf_remove_join_left() {
    let settings = small_settings(); // bf=4, min_bf=2
    let center = Leaf::with_keys(vec![5, 6], settings.clone());
    let left = Leaf::with_keys(vec![1, 2], settings.clone());
    // After removing one from center, len=1 < min_bf=2
    // left(2) + center(1) = 3 <= bf(4), so join
    let result = center.remove(&5, Some(&left), None, &int_cmp, &settings);
    match result {
        RemoveResult::Rebalanced { left: rl, center: rc, right: rr } => {
            assert!(rl.is_none()); // merged into center
            assert!(rr.is_none());
            assert_eq!(rc.keys(), &[1, 2, 6]);
        }
        _ => panic!("expected Rebalanced"),
    }
}

#[test]
fn test_leaf_remove_join_right() {
    let settings = small_settings();
    let center = Leaf::with_keys(vec![5, 6], settings.clone());
    let right = Leaf::with_keys(vec![8, 9], settings.clone());
    let result = center.remove(&5, None, Some(&right), &int_cmp, &settings);
    match result {
        RemoveResult::Rebalanced { left: rl, center: rc, right: rr } => {
            assert!(rl.is_none());
            assert!(rr.is_none()); // merged into center
            assert_eq!(rc.keys(), &[6, 8, 9]);
        }
        _ => panic!("expected Rebalanced"),
    }
}

#[test]
fn test_leaf_remove_borrow_left() {
    let settings = small_settings(); // bf=4, min_bf=2
    let center = Leaf::with_keys(vec![5, 6], settings.clone());
    // left(4) + center(1) = 5 > bf(4), can't join left.
    // right(4) + center(1) = 5 > bf(4), can't join right.
    // left(4) >= right(4), so borrow from left.
    let left2 = Leaf::with_keys(vec![1, 2, 3, 4], settings.clone());
    let right2 = Leaf::with_keys(vec![8, 9, 10, 11], settings.clone());
    let result = center.remove(&5, Some(&left2), Some(&right2), &int_cmp, &settings);
    match result {
        RemoveResult::Rebalanced { left: rl, center: rc, right: rr } => {
            assert!(rl.is_some());
            // total = left(4) + center(1) = 5, new_left = 2, new_center = 3
            assert_eq!(rl.unwrap().len() + rc.len(), 5);
            // right should be unchanged (passed through)
            assert!(rr.is_some());
        }
        _ => panic!("expected Rebalanced"),
    }
}

#[test]
fn test_leaf_remove_root_no_rebalance() {
    let settings = small_settings();
    let leaf = Leaf::with_keys(vec![5], settings.clone());
    // Root leaf with no siblings — no rebalance even though underflow
    let result = leaf.remove(&5, None, None, &int_cmp, &settings);
    match result {
        RemoveResult::Rebalanced { center, .. } => {
            assert!(center.is_empty());
        }
        _ => panic!("expected Rebalanced"),
    }
}

// --- Contains tests ---

#[test]
fn test_leaf_contains_found() {
    let leaf = Leaf::with_keys(vec![1, 3, 5], Settings::default());
    assert!(leaf.contains(&3, &int_cmp));
}

#[test]
fn test_leaf_contains_missing() {
    let leaf = Leaf::with_keys(vec![1, 3, 5], Settings::default());
    assert!(!leaf.contains(&4, &int_cmp));
}

#[test]
fn test_leaf_contains_empty() {
    let leaf = Leaf::new(Settings::default());
    assert!(!leaf.contains(&1, &int_cmp));
}
