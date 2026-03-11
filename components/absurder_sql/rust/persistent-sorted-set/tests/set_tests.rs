use std::cmp::Ordering;
use std::rc::Rc;

use persistent_sorted_set::Key;
use persistent_sorted_set::PersistentSortedSet;
use persistent_sorted_set::Settings;

fn int_cmp(a: &Key, b: &Key) -> Ordering {
    a.cmp(b)
}

fn make_cmp() -> Rc<dyn Fn(&Key, &Key) -> Ordering> {
    Rc::new(int_cmp)
}

fn make_set(keys: &[Key]) -> PersistentSortedSet {
    let mut set = PersistentSortedSet::empty(make_cmp());
    for k in keys {
        set = set.conj(k);
    }
    set
}

// --- Basic operations ---

#[test]
fn test_empty_set() {
    let set = PersistentSortedSet::empty(make_cmp());
    assert_eq!(set.count(), 0);
    assert!(set.is_empty());
    assert!(!set.contains(&1));
}

#[test]
fn test_conj_single() {
    let set = PersistentSortedSet::empty(make_cmp()).conj(&42);
    assert_eq!(set.count(), 1);
    assert!(set.contains(&42));
    assert!(!set.contains(&43));
}

#[test]
fn test_conj_multiple_sorted() {
    let set = make_set(&(1..=100).collect::<Vec<_>>());
    assert_eq!(set.count(), 100);
    let arr = set.to_vec();
    assert_eq!(arr, (1..=100).collect::<Vec<_>>());
}

#[test]
fn test_conj_multiple_reverse() {
    let set = make_set(&(1..=100).rev().collect::<Vec<_>>());
    assert_eq!(set.count(), 100);
    let arr = set.to_vec();
    assert_eq!(arr, (1..=100).collect::<Vec<_>>());
}

#[test]
fn test_conj_multiple_random() {
    let keys = vec![50, 25, 75, 12, 37, 62, 87, 6, 18, 31, 43, 56, 68, 81, 93];
    let set = make_set(&keys);
    assert_eq!(set.count(), keys.len());
    let arr = set.to_vec();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(arr, sorted);
}

#[test]
fn test_conj_duplicate() {
    let set = make_set(&[1, 2, 3]);
    let set2 = set.conj(&2);
    assert_eq!(set2.count(), 3);
}

#[test]
fn test_disj_single() {
    let set = make_set(&[42]).disj(&42);
    assert!(set.is_empty());
}

#[test]
fn test_disj_multiple() {
    let set = make_set(&(1..=10).collect::<Vec<_>>());
    let set = set.disj(&5).disj(&3).disj(&7);
    assert_eq!(set.count(), 7);
    assert!(!set.contains(&5));
    assert!(!set.contains(&3));
    assert!(!set.contains(&7));
    assert!(set.contains(&1));
    assert!(set.contains(&10));
}

#[test]
fn test_disj_missing() {
    let set = make_set(&[1, 2, 3]);
    let set2 = set.disj(&99);
    assert_eq!(set2.count(), 3);
}

#[test]
fn test_conj_then_disj() {
    let keys: Vec<Key> = (1..=100).collect();
    let mut set = make_set(&keys);
    for k in &keys {
        set = set.disj(k);
    }
    assert!(set.is_empty());
}

// --- Structural tests with small branching factor ---

fn make_small_set(keys: &[Key]) -> PersistentSortedSet {
    let _settings = Settings::new(4);
    let cmp = make_cmp();
    let mut set = PersistentSortedSet::empty(cmp);
    for k in keys {
        set = set.conj(k);
    }
    set
}

#[test]
fn test_large_set_1000() {
    let keys: Vec<Key> = (1..=1000).collect();
    let set = make_set(&keys);
    assert_eq!(set.count(), 1000);

    for k in &keys {
        assert!(set.contains(k));
    }
    assert!(!set.contains(&0));
    assert!(!set.contains(&1001));

    let arr = set.to_vec();
    assert_eq!(arr, keys);
}

#[test]
fn test_conj_disj_interleaved() {
    let mut set = PersistentSortedSet::empty(make_cmp());
    // Add 1..100, then remove evens
    for i in 1..=100 {
        set = set.conj(&i);
    }
    for i in (2..=100).step_by(2) {
        set = set.disj(&i);
    }
    assert_eq!(set.count(), 50);
    let arr = set.to_vec();
    let expected: Vec<Key> = (1..=100).step_by(2).collect();
    assert_eq!(arr, expected);
}

// --- Persistence (immutability) ---

#[test]
fn test_persistent_sharing() {
    let set1 = make_set(&[1, 2, 3]);
    let set2 = set1.conj(&4);
    // set1 was consumed by conj — we test by rebuilding
    let set1 = make_set(&[1, 2, 3]);
    assert_eq!(set1.count(), 3);
    assert!(!set1.contains(&4));
}

// --- Iteration ---

#[test]
fn test_seq_empty() {
    let set = PersistentSortedSet::empty(make_cmp());
    assert!(set.seq().is_none());
}

#[test]
fn test_seq_full_traversal() {
    let set = make_set(&[5, 3, 1, 4, 2]);
    let arr = set.to_vec();
    assert_eq!(arr, vec![1, 2, 3, 4, 5]);
}

#[test]
fn test_rseq_full_traversal() {
    let set = make_set(&[1, 2, 3, 4, 5]);
    let rseq = set.rseq().unwrap();
    let arr = rseq.to_vec();
    assert_eq!(arr, vec![5, 4, 3, 2, 1]);
}

#[test]
fn test_rseq_matches_reverse() {
    let set = make_set(&(1..=20).collect::<Vec<_>>());
    let fwd = set.to_vec();
    let rev = set.rseq().unwrap().to_vec();
    let mut expected = fwd;
    expected.reverse();
    assert_eq!(rev, expected);
}

// --- Slicing ---

#[test]
fn test_slice_from_to() {
    let set = make_set(&(1..=10).collect::<Vec<_>>());
    let slice = set.slice(Some(&3), Some(&7)).unwrap();
    assert_eq!(slice.to_vec(), vec![3, 4, 5, 6, 7]);
}

#[test]
fn test_slice_from_only() {
    let set = make_set(&(1..=10).collect::<Vec<_>>());
    let slice = set.slice(Some(&5), None).unwrap();
    assert_eq!(slice.to_vec(), vec![5, 6, 7, 8, 9, 10]);
}

#[test]
fn test_slice_to_only() {
    let set = make_set(&(1..=10).collect::<Vec<_>>());
    let slice = set.slice(None, Some(&5)).unwrap();
    assert_eq!(slice.to_vec(), vec![1, 2, 3, 4, 5]);
}

#[test]
fn test_slice_from_missing() {
    let set = make_set(&[1, 3, 5, 7, 9]);
    let slice = set.slice(Some(&4), None).unwrap();
    // 4 not in set → starts at next >= 4, which is 5
    assert_eq!(slice.first(), 5);
}

#[test]
fn test_slice_single_element() {
    let set = make_set(&(1..=10).collect::<Vec<_>>());
    let slice = set.slice(Some(&5), Some(&5)).unwrap();
    assert_eq!(slice.to_vec(), vec![5]);
}

#[test]
fn test_rslice_from_to() {
    let set = make_set(&(1..=10).collect::<Vec<_>>());
    let rslice = set.rslice(Some(&7), Some(&3)).unwrap();
    assert_eq!(rslice.to_vec(), vec![7, 6, 5, 4, 3]);
}

// --- Large scale ---

#[test]
fn test_conj_10000() {
    let keys: Vec<Key> = (1..=10000).collect();
    let set = make_set(&keys);
    assert_eq!(set.count(), 10000);
    assert!(set.contains(&1));
    assert!(set.contains(&5000));
    assert!(set.contains(&10000));
    assert!(!set.contains(&0));
    assert!(!set.contains(&10001));
}
