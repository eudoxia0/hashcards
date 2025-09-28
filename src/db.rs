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

use crate::fsrs::D;
use crate::fsrs::Grade;
use crate::fsrs::S;
use crate::fsrs::d_0;
use crate::fsrs::interval;
use crate::fsrs::new_difficulty;
use crate::fsrs::new_stability;
use crate::fsrs::retrievability;
use crate::fsrs::s_0;
use crate::new_db::Date;

const TARGET_RECALL: f64 = 0.9;

#[derive(Clone, Debug, PartialEq)]
pub struct Performance {
    pub last_review: Date,
    pub stability: S,
    pub difficulty: D,
    pub due_date: Date,
}

impl Performance {
    pub fn update(p: Option<Performance>, grade: Grade, today: Date) -> Self {
        let today = today.into_inner();
        let (stability, difficulty) = match p {
            Some(Performance {
                last_review,
                stability,
                difficulty,
                ..
            }) => {
                let last_review = last_review.into_inner();
                let time = (today - last_review).num_days() as f64;
                let retr = retrievability(time, stability);
                let stability = new_stability(difficulty, stability, retr, grade);
                let difficulty = new_difficulty(difficulty, grade);
                (stability, difficulty)
            }
            None => (s_0(grade), d_0(grade)),
        };
        let interval = f64::max(interval(TARGET_RECALL, stability).round(), 1.0);
        let interval_duration = chrono::Duration::days(interval as i64);
        let due_date = today + interval_duration;
        Performance {
            last_review: Date::new(today),
            stability,
            difficulty,
            due_date: Date::new(due_date),
        }
    }
}
