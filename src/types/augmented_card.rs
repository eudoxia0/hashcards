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
use crate::types::card::Card;
use crate::types::date::Date;

pub struct AugmentedCard {
    /// The card's text data from the filesystem.
    pub card: Card,
    /// The card's performance data from the database.
    pub perf: CardPerformance,
}

#[derive(Clone)]
pub struct CardPerformance {
    pub stability: Stability,
    pub difficulty: Difficulty,
    pub due_date: Date,
    pub review_count: usize,
}
