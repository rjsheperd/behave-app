/// Configuration for the persistent sorted set.
#[derive(Clone, Debug)]
pub struct Settings {
    branching_factor: usize,
    edit: Option<bool>,
    cache_size: usize,
}

impl Settings {
    pub fn new(branching_factor: usize) -> Self {
        let bf = if branching_factor == 0 { 512 } else { branching_factor };
        Self {
            branching_factor: bf,
            edit: None,
            cache_size: 1024,
        }
    }

    pub fn with_cache_size(mut self, cache_size: usize) -> Self {
        self.cache_size = if cache_size == 0 { 1024 } else { cache_size };
        self
    }

    pub fn cache_size(&self) -> usize {
        self.cache_size
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
            cache_size: self.cache_size,
        }
    }

    pub fn make_persistent(&mut self) {
        assert!(self.edit.is_some(), "Not editable");
        self.edit = Some(false);
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self::new(512) // cache_size defaults to 1024 via new()
    }
}
