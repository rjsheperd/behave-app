//! Leaf node: the bottom level of the B+ tree.
//! Holds a sorted `Vec<Key>` with no children. Supports add (with split) and
//! remove (with merge/borrow from siblings).

use std::rc::Rc;

use crate::key::Key;
use crate::node::{Comparator, Node};
use crate::results::{AddResult, RemoveResult};
use crate::settings::Settings;

#[derive(Clone, Debug)]
pub struct Leaf {
    pub(crate) keys: Vec<Key>,
}

impl Leaf {
    pub fn new(_settings: Settings) -> Self {
        Self {
            keys: Vec::new(),
        }
    }

    pub fn with_keys(keys: Vec<Key>, _settings: Settings) -> Self {
        Self { keys }
    }

    pub fn keys(&self) -> &[Key] {
        &self.keys
    }

    pub fn contains(&self, key: &Key, cmp: &Comparator) -> bool {
        Node::search(&self.keys, key, cmp).is_ok()
    }

    pub fn add(&self, key: &Key, cmp: &Comparator, settings: &Settings) -> AddResult {
        match Node::search(&self.keys, key, cmp) {
            Ok(_) => AddResult::Unchanged,
            Err(ins) => {
                if self.keys.len() < settings.branching_factor() {
                    // Fits in current node — create new leaf with key inserted
                    let mut new_keys = Vec::with_capacity(self.keys.len() + 1);
                    new_keys.extend_from_slice(&self.keys[..ins]);
                    new_keys.push(key.clone());
                    new_keys.extend_from_slice(&self.keys[ins..]);
                    AddResult::One(Rc::new(Node::Leaf(Leaf::with_keys(
                        new_keys,
                        settings.clone(),
                    ))))
                } else {
                    // Need to split
                    let total = self.keys.len() + 1;
                    let half1 = (total + 1) / 2;
                    let half2 = total - half1;

                    if ins < half1 {
                        let mut keys1 = Vec::with_capacity(half1);
                        keys1.extend_from_slice(&self.keys[..ins]);
                        keys1.push(key.clone());
                        keys1.extend_from_slice(&self.keys[ins..half1 - 1]);

                        let mut keys2 = Vec::with_capacity(half2);
                        keys2.extend_from_slice(&self.keys[half1 - 1..]);

                        AddResult::Split(
                            Rc::new(Node::Leaf(Leaf::with_keys(keys1, settings.clone()))),
                            Rc::new(Node::Leaf(Leaf::with_keys(keys2, settings.clone()))),
                        )
                    } else {
                        let mut keys1 = Vec::with_capacity(half1);
                        keys1.extend_from_slice(&self.keys[..half1]);

                        let ins2 = ins - half1;
                        let mut keys2 = Vec::with_capacity(half2);
                        keys2.extend_from_slice(&self.keys[half1..half1 + ins2]);
                        keys2.push(key.clone());
                        keys2.extend_from_slice(&self.keys[half1 + ins2..]);

                        AddResult::Split(
                            Rc::new(Node::Leaf(Leaf::with_keys(keys1, settings.clone()))),
                            Rc::new(Node::Leaf(Leaf::with_keys(keys2, settings.clone()))),
                        )
                    }
                }
            }
        }
    }

    pub fn remove(
        &self,
        key: &Key,
        left: Option<&Leaf>,
        right: Option<&Leaf>,
        cmp: &Comparator,
        settings: &Settings,
    ) -> RemoveResult {
        let idx = match Node::search(&self.keys, key, cmp) {
            Ok(idx) => idx,
            Err(_) => return RemoveResult::Unchanged,
        };

        let new_len = self.keys.len() - 1;

        // Build keys without the removed element
        let make_removed = || {
            let mut new_keys = Vec::with_capacity(new_len);
            new_keys.extend_from_slice(&self.keys[..idx]);
            new_keys.extend_from_slice(&self.keys[idx + 1..]);
            new_keys
        };

        // No rebalancing needed (enough keys or root leaf)
        if new_len >= settings.min_branching_factor() || (left.is_none() && right.is_none()) {
            let new_keys = make_removed();
            return RemoveResult::Rebalanced {
                left: left.map(|l| Rc::new(Node::Leaf(l.clone()))),
                center: Rc::new(Node::Leaf(Leaf::with_keys(new_keys, settings.clone()))),
                right: right.map(|r| Rc::new(Node::Leaf(r.clone()))),
            };
        }

        let left_len = left.map_or(0, |l| l.keys.len());
        let right_len = right.map_or(0, |r| r.keys.len());

        // Can join with left
        if let Some(l) = left {
            if left_len + new_len <= settings.branching_factor() {
                let mut join_keys = Vec::with_capacity(left_len + new_len);
                join_keys.extend_from_slice(&l.keys);
                join_keys.extend_from_slice(&self.keys[..idx]);
                join_keys.extend_from_slice(&self.keys[idx + 1..]);
                return RemoveResult::Rebalanced {
                    left: None,
                    center: Rc::new(Node::Leaf(Leaf::with_keys(join_keys, settings.clone()))),
                    right: right.map(|r| Rc::new(Node::Leaf(r.clone()))),
                };
            }
        }

        // Can join with right
        if let Some(r) = right {
            if new_len + right_len <= settings.branching_factor() {
                let mut join_keys = Vec::with_capacity(new_len + right_len);
                join_keys.extend_from_slice(&self.keys[..idx]);
                join_keys.extend_from_slice(&self.keys[idx + 1..]);
                join_keys.extend_from_slice(&r.keys);
                return RemoveResult::Rebalanced {
                    left: left.map(|l| Rc::new(Node::Leaf(l.clone()))),
                    center: Rc::new(Node::Leaf(Leaf::with_keys(join_keys, settings.clone()))),
                    right: None,
                };
            }
        }

        // Borrow from left
        if let Some(l) = left {
            if right.is_none() || left_len >= right_len {
                let total_len = left_len + new_len;
                let new_left_len = total_len / 2;
                let new_center_len = total_len - new_left_len;

                let new_left_keys = l.keys[..new_left_len].to_vec();

                let mut new_center_keys = Vec::with_capacity(new_center_len);
                new_center_keys.extend_from_slice(&l.keys[new_left_len..]);
                new_center_keys.extend_from_slice(&self.keys[..idx]);
                new_center_keys.extend_from_slice(&self.keys[idx + 1..]);

                return RemoveResult::Rebalanced {
                    left: Some(Rc::new(Node::Leaf(Leaf::with_keys(
                        new_left_keys,
                        settings.clone(),
                    )))),
                    center: Rc::new(Node::Leaf(Leaf::with_keys(
                        new_center_keys,
                        settings.clone(),
                    ))),
                    right: right.map(|r| Rc::new(Node::Leaf(r.clone()))),
                };
            }
        }

        // Borrow from right
        if let Some(r) = right {
            let total_len = new_len + right_len;
            let new_center_len = total_len / 2;
            let new_right_len = total_len - new_center_len;
            let right_head = right_len - new_right_len;

            let mut new_center_keys = Vec::with_capacity(new_center_len);
            new_center_keys.extend_from_slice(&self.keys[..idx]);
            new_center_keys.extend_from_slice(&self.keys[idx + 1..]);
            new_center_keys.extend_from_slice(&r.keys[..right_head]);

            let new_right_keys = r.keys[right_head..].to_vec();

            return RemoveResult::Rebalanced {
                left: left.map(|l| Rc::new(Node::Leaf(l.clone()))),
                center: Rc::new(Node::Leaf(Leaf::with_keys(
                    new_center_keys,
                    settings.clone(),
                ))),
                right: Some(Rc::new(Node::Leaf(Leaf::with_keys(
                    new_right_keys,
                    settings.clone(),
                )))),
            };
        }

        unreachable!("leaf remove: no rebalance path matched")
    }
}
