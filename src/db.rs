use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;

use blake3::Hash;
use chrono::NaiveDate;
use serde::Deserialize;

use crate::error::ErrorReport;
use crate::error::Fallible;
use crate::error::fail;
use crate::fsrs::D;
use crate::fsrs::S;

pub struct Database {
    inner: HashMap<Hash, Performance>,
}

pub enum Performance {
    New,
    Reviewed {
        stability: S,
        difficulty: D,
        due_date: NaiveDate,
    },
}

#[derive(Deserialize)]
struct DatabaseRow {
    hash: String,
    stability: Option<S>,
    difficulty: Option<D>,
    due_date: Option<String>,
}

impl DatabaseRow {
    pub fn parse(self) -> Fallible<(Hash, Performance)> {
        let performance = match (self.stability, self.difficulty, self.due_date) {
            (Some(s), Some(d), Some(dd)) => Ok(Performance::Reviewed {
                stability: s,
                difficulty: d,
                due_date: NaiveDate::parse_from_str(&dd, "%Y-%m-%d")
                    .map_err(|_| ErrorReport::new("invalid due date"))?,
            }),
            (None, None, None) => Ok(Performance::New),
            _ => fail("broken performance database"),
        };
        let hash = Hash::from_hex(&self.hash)
            .map_err(|_| ErrorReport::new("invalid hash in performance database"))?;
        Ok((hash, performance?))
    }
}

impl Database {
    pub fn empty() -> Self {
        Database {
            inner: HashMap::new(),
        }
    }

    pub fn from_csv(path: &PathBuf) -> Fallible<Self> {
        let mut reader = csv::Reader::from_path(path)?;
        let mut db = HashMap::new();
        for record in reader.records() {
            let row: DatabaseRow = record?.deserialize(None)?;
            let (hash, performance) = row.parse()?;
            db.insert(hash, performance);
        }
        Ok(Database { inner: db })
    }

    pub fn keys(&self) -> HashSet<Hash> {
        self.inner.keys().cloned().collect()
    }

    pub fn insert(&mut self, hash: Hash, performance: Performance) {
        self.inner.insert(hash, performance);
    }

    pub fn remove(&mut self, hash: &Hash) {
        self.inner.remove(hash);
    }
}
