// Copyright 2025 Fernando Borretti
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;

use blake3::Hash;
use chrono::Local;
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

#[allow(dead_code)]
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

    // Return new cards and cards due today.
    pub fn due_today(&self) -> HashSet<Hash> {
        let today = Local::now().naive_local().date();
        self.inner
            .iter()
            .filter_map(|(hash, performance)| match performance {
                Performance::New => Some(*hash),
                Performance::Reviewed { due_date, .. } if *due_date <= today => Some(*hash),
                _ => None,
            })
            .collect()
    }
}
