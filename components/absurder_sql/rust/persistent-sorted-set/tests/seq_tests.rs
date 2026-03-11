use std::cmp::Ordering;
use std::rc::Rc;

use persistent_sorted_set::Key;
use persistent_sorted_set::PersistentSortedSet;

fn make_cmp() -> Rc<dyn Fn(&Key, &Key) -> Ordering> {
    Rc::new(|a: &Key, b: &Key| a.cmp(b))
}

fn make_set(keys: &[Key]) -> PersistentSortedSet {
    let mut set = PersistentSortedSet::empty(make_cmp());
    for k in keys {
        set = set.conj(k);
    }
    set
}

// --- Forward iteration ---

#[test]
fn test_seq_empty_set() {
    let set = PersistentSortedSet::empty(make_cmp());
    assert!(set.seq().is_none());
}

#[test]
fn test_seq_single_element() {
    let set = make_set(&[42]);
    let seq = set.seq().unwrap();
    assert_eq!(seq.first(), 42);
    assert!(seq.next().is_none());
}

#[test]
fn test_seq_full_traversal() {
    let set = make_set(&[5, 1, 3, 2, 4]);
    let arr = set.to_vec();
    assert_eq!(arr, vec![1, 2, 3, 4, 5]);
}

#[test]
fn test_seq_to_array() {
    let set = make_set(&(1..=20).collect::<Vec<_>>());
    let seq = set.seq().unwrap();
    let arr = seq.to_vec();
    assert_eq!(arr, (1..=20).collect::<Vec<_>>());
}

// --- Reverse iteration ---

#[test]
fn test_rseq_empty_set() {
    let set = PersistentSortedSet::empty(make_cmp());
    assert!(set.rseq().is_none());
}

#[test]
fn test_rseq_full_traversal() {
    let set = make_set(&(1..=10).collect::<Vec<_>>());
    let rseq = set.rseq().unwrap();
    let arr = rseq.to_vec();
    assert_eq!(arr, (1..=10).rev().collect::<Vec<_>>());
}

#[test]
fn test_rseq_matches_reverse() {
    let set = make_set(&(1..=50).collect::<Vec<_>>());
    let fwd = set.to_vec();
    let rev = set.rseq().unwrap().to_vec();
    let mut expected = fwd;
    expected.reverse();
    assert_eq!(rev, expected);
}

// --- Slicing ---

#[test]
fn test_slice_full_range() {
    let set = make_set(&(1..=10).collect::<Vec<_>>());
    let seq = set.slice(None, None).unwrap();
    let full = set.seq().unwrap();
    assert_eq!(seq.to_vec(), full.to_vec());
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
fn test_slice_from_to() {
    let set = make_set(&(1..=10).collect::<Vec<_>>());
    let slice = set.slice(Some(&3), Some(&7)).unwrap();
    assert_eq!(slice.to_vec(), vec![3, 4, 5, 6, 7]);
}

#[test]
fn test_slice_from_missing() {
    let set = make_set(&[1, 3, 5, 7, 9]);
    let slice = set.slice(Some(&4), None).unwrap();
    assert_eq!(slice.first(), 5);
}

#[test]
fn test_slice_to_missing() {
    let set = make_set(&[1, 3, 5, 7, 9]);
    let slice = set.slice(None, Some(&6)).unwrap();
    let arr = slice.to_vec();
    assert_eq!(arr, vec![1, 3, 5]);
}

#[test]
fn test_slice_empty_range() {
    let set = make_set(&(1..=10).collect::<Vec<_>>());
    // from=10, to=5 means start at 10 but stop at 5 — since ascending, this is empty
    let slice = set.slice(Some(&11), Some(&5));
    assert!(slice.is_none());
}

#[test]
fn test_slice_single_element() {
    let set = make_set(&(1..=10).collect::<Vec<_>>());
    let slice = set.slice(Some(&5), Some(&5)).unwrap();
    assert_eq!(slice.to_vec(), vec![5]);
}

// --- Reverse slicing ---

#[test]
fn test_rslice_full_range() {
    let set = make_set(&(1..=10).collect::<Vec<_>>());
    let rslice = set.rslice(None, None).unwrap();
    let rseq = set.rseq().unwrap();
    assert_eq!(rslice.to_vec(), rseq.to_vec());
}

#[test]
fn test_rslice_from_to() {
    let set = make_set(&(1..=10).collect::<Vec<_>>());
    let rslice = set.rslice(Some(&7), Some(&3)).unwrap();
    assert_eq!(rslice.to_vec(), vec![7, 6, 5, 4, 3]);
}

#[test]
fn test_rslice_from_missing() {
    let set = make_set(&[1, 3, 5, 7, 9]);
    let rslice = set.rslice(Some(&6), None).unwrap();
    // 6 not in set → starts at last <= 6, which is 5
    assert_eq!(rslice.first(), 5);
}

// --- Seek ---

#[test]
fn test_seek_forward() {
    let set = make_set(&(1..=10).collect::<Vec<_>>());
    let seq = set.seq().unwrap();
    let seeked = seq.seek(&5, None).unwrap();
    assert_eq!(seeked.first(), 5);
    assert_eq!(seeked.to_vec(), vec![5, 6, 7, 8, 9, 10]);
}

#[test]
fn test_seek_beyond_range() {
    let set = make_set(&(1..=10).collect::<Vec<_>>());
    let seq = set.seq().unwrap();
    let seeked = seq.seek(&11, None);
    assert!(seeked.is_none());
}

// --- Large tree iteration ---

#[test]
fn test_seq_large_set() {
    let keys: Vec<Key> = (1..=5000).collect();
    let set = make_set(&keys);
    let arr = set.to_vec();
    assert_eq!(arr, keys);
}

#[test]
fn test_rseq_large_set() {
    let keys: Vec<Key> = (1..=5000).collect();
    let set = make_set(&keys);
    let rev = set.rseq().unwrap().to_vec();
    let expected: Vec<Key> = (1..=5000).rev().collect();
    assert_eq!(rev, expected);
}
