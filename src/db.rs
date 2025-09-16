use std::collections::HashMap;

use blake3::Hash;

use crate::fsrs::D;
use crate::fsrs::S;

pub enum Performance {
    New,
    Reviewed { stability: S, difficulty: D },
}

struct PerformanceDto {
    stability: Option<S>,
    difficulty: Option<D>,
}

pub struct Database {
    inner: HashMap<Hash, Performance>,
}

impl Database {
    pub fn empty() -> Self {
        Database {
            inner: HashMap::new(),
        }
    }
}
