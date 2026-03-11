use std::cmp::Ordering;
use persistent_sorted_set::node::Node;
use persistent_sorted_set::Key;

fn int_cmp(a: &Key, b: &Key) -> Ordering {
    a.cmp(b)
}

#[test]
fn test_search_empty_keys() {
    let keys: Vec<Key> = vec![];
    assert_eq!(Node::search(&keys, &5, &int_cmp), Err(0));
}

#[test]
fn test_search_single_key_found() {
    let keys = vec![5];
    assert_eq!(Node::search(&keys, &5, &int_cmp), Ok(0));
}

#[test]
fn test_search_single_key_missing() {
    let keys = vec![5];
    assert_eq!(Node::search(&keys, &3, &int_cmp), Err(0));
    assert_eq!(Node::search(&keys, &7, &int_cmp), Err(1));
}

#[test]
fn test_search_multiple_found() {
    let keys = vec![1, 3, 5, 7, 9];
    assert_eq!(Node::search(&keys, &5, &int_cmp), Ok(2));
}

#[test]
fn test_search_multiple_missing() {
    let keys = vec![1, 3, 5, 7, 9];
    assert_eq!(Node::search(&keys, &4, &int_cmp), Err(2));
}

#[test]
fn test_search_first_boundary() {
    let keys = vec![1, 3, 5, 7, 9];
    // first >= 4 is at idx 2 (value 5)
    assert_eq!(Node::search_first(&keys, &4, &int_cmp), Err(2));
}

#[test]
fn test_search_first_exact() {
    let keys = vec![1, 3, 5, 7, 9];
    assert_eq!(Node::search_first(&keys, &5, &int_cmp), Ok(2));
}

#[test]
fn test_search_last_boundary() {
    let keys = vec![1, 3, 5, 7, 9];
    // last <= 4 is at idx 1 (value 3)
    assert_eq!(Node::search_last(&keys, &4, &int_cmp), Some(1));
}

#[test]
fn test_search_last_exact() {
    let keys = vec![1, 3, 5, 7, 9];
    assert_eq!(Node::search_last(&keys, &5, &int_cmp), Some(2));
}

#[test]
fn test_search_last_none() {
    let keys = vec![5, 7, 9];
    assert_eq!(Node::search_last(&keys, &3, &int_cmp), None);
}

#[test]
fn test_search_first_beyond() {
    let keys = vec![1, 3, 5];
    // past end
    assert_eq!(Node::search_first(&keys, &10, &int_cmp), Err(3));
}
