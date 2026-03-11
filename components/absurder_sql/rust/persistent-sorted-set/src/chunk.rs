use std::cmp::Ordering;
use std::rc::Rc;

use crate::key::Key;
use crate::node::Node;
use crate::seq::Seq;

/// Chunked iteration for efficient batch access within a single leaf.
/// Used by ClojureScript's chunked-seq protocol.
#[derive(Clone)]
pub struct Chunk {
    keys: Vec<Key>,
    idx: usize,
    end: usize,
    ascending: bool,
    version: u64,
    set_version: Rc<std::cell::Cell<u64>>,
}

impl Chunk {
    /// Create from a Seq positioned at a leaf.
    pub fn from_seq(seq: &Seq) -> Option<Chunk> {
        // Access leaf keys — Seq must be on a leaf node
        let leaf = &seq.leaf;
        match leaf.as_ref() {
            Node::Leaf(l) => {
                let idx = seq.idx;
                let keys = &l.keys;
                let ascending = seq.ascending;

                let end = if ascending {
                    let mut e = l.keys.len() - 1;
                    if let (Some(kt), Some(cmp)) = (&seq.key_to, &seq.cmp) {
                        while e > idx && cmp(&keys[e], kt) == Ordering::Greater {
                            e -= 1;
                        }
                    }
                    e
                } else {
                    let mut e = 0;
                    if let (Some(kt), Some(cmp)) = (&seq.key_to, &seq.cmp) {
                        while e < idx && cmp(&keys[e], kt) == Ordering::Less {
                            e += 1;
                        }
                    }
                    e
                };

                Some(Chunk {
                    keys: keys.clone(),
                    idx,
                    end,
                    ascending,
                    version: seq.version,
                    set_version: Rc::clone(&seq.set_version),
                })
            }
            _ => None,
        }
    }

    fn check_version(&self) {
        assert_eq!(
            self.version,
            self.set_version.get(),
            "iterating and mutating a transient set at the same time"
        );
    }

    pub fn count(&self) -> usize {
        self.check_version();
        if self.ascending {
            self.end - self.idx + 1
        } else {
            self.idx - self.end + 1
        }
    }

    pub fn nth(&self, i: usize) -> Option<Key> {
        self.check_version();
        if i < self.count() {
            let actual = if self.ascending {
                self.idx + i
            } else {
                self.idx - i
            };
            Some(self.keys[actual].clone())
        } else {
            None
        }
    }

    pub fn drop_first(&self) -> Chunk {
        self.check_version();
        assert_ne!(self.idx, self.end, "dropFirst of empty chunk");
        Chunk {
            keys: self.keys.clone(),
            idx: if self.ascending {
                self.idx + 1
            } else {
                self.idx - 1
            },
            end: self.end,
            ascending: self.ascending,
            version: self.version,
            set_version: Rc::clone(&self.set_version),
        }
    }

    pub fn reduce<F, A>(&self, f: F, init: A) -> A
    where
        F: Fn(A, &Key) -> A,
    {
        self.check_version();
        let mut ret = f(init, &self.keys[self.idx]);
        if self.ascending {
            for x in (self.idx + 1)..=self.end {
                ret = f(ret, &self.keys[x]);
            }
        } else {
            for x in (self.end..self.idx).rev() {
                ret = f(ret, &self.keys[x]);
            }
        }
        ret
    }

    pub fn to_vec(&self) -> Vec<Key> {
        self.check_version();
        if self.ascending {
            self.keys[self.idx..=self.end].to_vec()
        } else {
            let mut v: Vec<Key> = self.keys[self.end..=self.idx].to_vec();
            v.reverse();
            v
        }
    }
}
