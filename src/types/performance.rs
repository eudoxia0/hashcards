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
use crate::fsrs::Stability;
use crate::types::date::Date;
use crate::types::timestamp::Timestamp;

/// Represents performance information for a card.
#[derive(Clone)]
pub enum Performance {
    /// The card is new, and has never been reviewed.
    New,
    /// The card has been reviewed at least once.
    Reviewed(ReviewedPerformance),
}

#[derive(Clone)]
pub struct ReviewedPerformance {
    /// The timestamp when the card was last reviewed.
    pub last_reviewed_at: Timestamp,
    /// The card's stability (an FSRS parameter).
    pub stability: Stability,
    /// The card's difficulty (an FSRS parameter).
    pub difficulty: Difficulty,
    /// The card's next due date.
    pub due_date: Date,
    /// The number of times the card has been reviewed.
    pub review_count: usize,
}
