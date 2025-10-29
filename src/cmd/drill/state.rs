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

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use tokio::sync::oneshot;

use crate::cmd::drill::cache::Cache;
use crate::db::Database;
use crate::db::ReviewRecord;
use crate::fsrs::Difficulty;
use crate::fsrs::Grade;
use crate::fsrs::Stability;
use crate::types::card::Card;
use crate::types::date::Date;
use crate::types::timestamp::Timestamp;

#[derive(Clone)]
pub struct ServerState {
    pub port: u16,
    pub directory: PathBuf,
    pub macros: Vec<(String, String)>,
    pub total_cards: usize,
    pub session_started_at: Timestamp,
    pub mutable: Arc<Mutex<MutableState>>,
    pub shutdown_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

pub struct MutableState {
    pub reveal: bool,
    pub db: Database,
    pub cache: Cache,
    pub cards: Vec<Card>,
    pub reviews: Vec<Review>,
    pub finished_at: Option<Timestamp>,
}

#[derive(Clone)]
pub struct Review {
    pub card: Card,
    pub reviewed_at: Timestamp,
    pub grade: Grade,
    pub stability: Stability,
    pub difficulty: Difficulty,
    pub interval_raw: f64,
    pub interval_days: usize,
    pub due_date: Date,
}

impl Review {
    pub fn should_repeat(&self) -> bool {
        self.grade == Grade::Forgot || self.grade == Grade::Hard
    }

    pub fn into_record(self) -> ReviewRecord {
        ReviewRecord {
            card_hash: self.card.hash(),
            reviewed_at: self.reviewed_at,
            grade: self.grade,
            stability: self.stability,
            difficulty: self.difficulty,
            interval_raw: self.interval_raw,
            interval_days: self.interval_days,
            due_date: self.due_date,
        }
    }
}
