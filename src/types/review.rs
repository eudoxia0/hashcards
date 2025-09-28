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

use crate::fsrs::Difficulty;
use crate::fsrs::Grade;
use crate::fsrs::Stability;
use crate::fsrs::initial_difficulty;
use crate::fsrs::initial_stability;
use crate::fsrs::interval;
use crate::fsrs::new_difficulty;
use crate::fsrs::new_stability;
use crate::fsrs::retrievability;
use crate::types::date::Date;
use crate::types::hash::Hash;
use crate::types::timestamp::Timestamp;

/// The desired recall probability.
const TARGET_RECALL: f64 = 0.9;

/// The minimum review interval in days.
const MIN_INTERVAL: f64 = 1.0;

/// The maximum review interval in days.
const MAX_INTERVAL: f64 = 128.0;

pub struct Review {
    pub card_hash: Hash,
    pub reviewed_at: Timestamp,
    pub grade: Grade,
    pub stability: Stability,
    pub difficulty: Difficulty,
    pub due_date: Date,
}

pub struct Parameters {
    pub stability: Stability,
    pub difficulty: Difficulty,
    pub due_date: Date,
}

pub fn update_card(review: Option<Review>, grade: Grade, today: Date) -> Parameters {
    let today = today.into_inner();
    let (stability, difficulty) = match review {
        Some(Review {
            reviewed_at,
            grade,
            stability,
            difficulty,
            ..
        }) => {
            let last_review = reviewed_at.local_date().into_inner();
            let time = (today - last_review).num_days() as f64;
            let retr = retrievability(time, stability);
            let stability = new_stability(difficulty, stability, retr, grade);
            let difficulty = new_difficulty(difficulty, grade);
            (stability, difficulty)
        }
        None => (initial_stability(grade), initial_difficulty(grade)),
    };
    let interval = interval(TARGET_RECALL, stability)
        .round()
        .clamp(MIN_INTERVAL, MAX_INTERVAL);
    let interval_duration = chrono::Duration::days(interval as i64);
    let due_date = today + interval_duration;
    Parameters {
        stability,
        difficulty,
        due_date: Date::new(due_date),
    }
}
