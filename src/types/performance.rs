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

use chrono::Duration;
use chrono::NaiveDate;

use crate::fsrs::Difficulty;
use crate::fsrs::Grade;
use crate::fsrs::Interval;
use crate::fsrs::Recall;
use crate::fsrs::Stability;
use crate::fsrs::initial_difficulty;
use crate::fsrs::initial_stability;
use crate::fsrs::interval;
use crate::fsrs::new_difficulty;
use crate::fsrs::new_stability;
use crate::fsrs::retrievability;
use crate::types::date::Date;
use crate::types::timestamp::Timestamp;

/// The desired recall probability.
const TARGET_RECALL: f64 = 0.9;

/// The minimum review interval in days.
const MIN_INTERVAL: f64 = 1.0;

/// The maximum review interval in days.
const MAX_INTERVAL: f64 = 256.0;

/// Represents performance information for a card.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Performance {
    /// The card is new, and has never been reviewed.
    New,
    /// The card has been reviewed at least once.
    Reviewed(ReviewedPerformance),
}

impl Performance {
    pub fn is_new(&self) -> bool {
        matches!(self, Performance::New)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReviewedPerformance {
    /// The timestamp when the card was last reviewed.
    pub last_reviewed_at: Timestamp,
    /// The card's stability (an FSRS parameter).
    pub stability: Stability,
    /// The card's difficulty (an FSRS parameter).
    pub difficulty: Difficulty,
    /// The FSRS-calculated interval in hours until the next review. This is
    /// the raw interval, before any rounding and clamping.
    pub interval_raw: Interval,
    /// The FSRS interval as an integer number of days.
    pub interval_days: usize,
    /// The card's next due date.
    pub due_date: Date,
    /// The number of times the card has been reviewed.
    pub review_count: usize,
}

pub fn update_performance(
    perf: Performance,
    grade: Grade,
    reviewed_at: Timestamp,
) -> ReviewedPerformance {
    let today: NaiveDate = reviewed_at.local_date().into_inner();
    let (stability, difficulty, review_count): (Stability, Difficulty, usize) = match perf {
        Performance::New => (initial_stability(grade), initial_difficulty(grade), 0),
        Performance::Reviewed(ReviewedPerformance {
            last_reviewed_at,
            stability,
            difficulty,
            review_count,
            ..
        }) => {
            let last_reviewed_at: NaiveDate = last_reviewed_at.local_date().into_inner();
            let time: Interval = (today - last_reviewed_at).num_days() as f64;
            let retr: Recall = retrievability(time, stability);
            let stability: Stability = new_stability(difficulty, stability, retr, grade);
            let difficulty: Difficulty = new_difficulty(difficulty, grade);
            (stability, difficulty, review_count)
        }
    };
    let interval_raw: Interval = interval(TARGET_RECALL, stability);
    let interval_rounded: Interval = interval_raw.round();
    let interval_clamped: Interval = interval_rounded.clamp(MIN_INTERVAL, MAX_INTERVAL);
    let interval_days: usize = interval_clamped as usize;
    let interval_duration: Duration = Duration::days(interval_clamped as i64);
    let due_date: Date = Date::new(today + interval_duration);
    ReviewedPerformance {
        last_reviewed_at: reviewed_at,
        stability,
        difficulty,
        interval_raw,
        interval_days,
        due_date,
        review_count: review_count + 1,
    }
}
