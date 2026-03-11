/// Configuration for the persistent sorted set.
/// In Rust, no RefType/WeakRef needed — Rc<Node> handles structural sharing
/// and SQLiteStorage uses an explicit LRU cache.
#[derive(Clone, Debug)]
pub struct Settings {
    branching_factor: usize,
    edit: Option<bool>,
}

impl Settings {
    pub fn new(branching_factor: usize) -> Self {
        let bf = if branching_factor == 0 { 512 } else { branching_factor };
        Self {
            branching_factor: bf,
            edit: None,
        }
    }

    pub fn branching_factor(&self) -> usize {
        self.branching_factor
    }

    pub fn min_branching_factor(&self) -> usize {
        self.branching_factor >> 1
    }

    pub fn editable(&self) -> bool {
        self.edit == Some(true)
    }

    pub fn editable_settings(&self) -> Self {
        assert!(!self.editable(), "Already editable");
        Self {
            branching_factor: self.branching_factor,
            edit: Some(true),
        }
    }

    pub fn make_persistent(&mut self) {
        assert!(self.edit.is_some(), "Not editable");
        self.edit = Some(false);
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self::new(512)
    }
}
