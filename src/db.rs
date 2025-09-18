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
use crate::fsrs::Grade;
use crate::fsrs::S;
use crate::fsrs::d_0;
use crate::fsrs::interval;
use crate::fsrs::new_difficulty;
use crate::fsrs::new_stability;
use crate::fsrs::retrievability;
use crate::fsrs::s_0;

const TARGET_RECALL: f64 = 0.9;

pub struct Database {
    inner: HashMap<Hash, Performance>,
}

#[derive(Clone)]
pub enum Performance {
    New,
    Reviewed {
        last_review: NaiveDate,
        stability: S,
        difficulty: D,
        due_date: NaiveDate,
    },
}

impl Performance {
    pub fn update(self, grade: Grade, today: NaiveDate) -> Self {
        match self {
            Performance::New => {
                let stability = s_0(grade);
                let difficulty = d_0(grade);
                let interval = f64::max(interval(TARGET_RECALL, stability).round(), 1.0);
                let interval_duration = chrono::Duration::days(interval as i64);
                let due_date = today + interval_duration;
                Performance::Reviewed {
                    last_review: today,
                    stability,
                    difficulty,
                    due_date,
                }
            }
            Performance::Reviewed {
                last_review,
                stability,
                difficulty,
                ..
            } => {
                let today = Local::now().naive_local().date();
                let time = (today - last_review).num_days() as f64;
                let retr = retrievability(time, stability);
                let stability = new_stability(difficulty, stability, retr, grade);
                let difficulty = new_difficulty(difficulty, grade);
                let interval = f64::max(interval(TARGET_RECALL, stability).round(), 1.0);
                let interval_duration = chrono::Duration::days(interval as i64);
                let due_date = today + interval_duration;
                Performance::Reviewed {
                    last_review: today,
                    stability,
                    difficulty,
                    due_date,
                }
            }
        }
    }
}

#[derive(Deserialize)]
struct DatabaseRow {
    hash: String,
    last_review: Option<String>,
    stability: Option<S>,
    difficulty: Option<D>,
    due_date: Option<String>,
}

impl DatabaseRow {
    pub fn parse(self) -> Fallible<(Hash, Performance)> {
        let performance = match (
            self.last_review,
            self.stability,
            self.difficulty,
            self.due_date,
        ) {
            (Some(lr), Some(s), Some(d), Some(dd)) => Ok(Performance::Reviewed {
                last_review: NaiveDate::parse_from_str(&lr, "%Y-%m-%d")
                    .map_err(|_| ErrorReport::new("invalid last review date"))?,
                stability: s,
                difficulty: d,
                due_date: NaiveDate::parse_from_str(&dd, "%Y-%m-%d")
                    .map_err(|_| ErrorReport::new("invalid due date"))?,
            }),
            (None, None, None, None) => Ok(Performance::New),
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

    pub fn get(&self, hash: Hash) -> Option<Performance> {
        self.inner.get(&hash).cloned()
    }

    pub fn update(&mut self, hash: Hash, performance: Performance) {
        self.inner.insert(hash, performance);
    }

    pub fn to_csv(&self, path: &PathBuf) -> Fallible<()> {
        let mut writer = csv::Writer::from_path(path)?;
        writer.write_record(["hash", "last_review", "stability", "difficulty", "due_date"])?;
        for (hash, performance) in &self.inner {
            match performance {
                Performance::New => {
                    writer.write_record([hash.to_hex().as_str(), "", "", "", ""])?;
                }
                Performance::Reviewed {
                    last_review,
                    stability,
                    difficulty,
                    due_date,
                } => {
                    writer.write_record([
                        hash.to_hex().as_str(),
                        &last_review.format("%Y-%m-%d").to_string(),
                        &stability.to_string(),
                        &difficulty.to_string(),
                        &due_date.format("%Y-%m-%d").to_string(),
                    ])?;
                }
            }
        }
        writer.flush()?;
        Ok(())
    }
}
