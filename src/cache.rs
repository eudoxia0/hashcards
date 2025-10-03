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

use crate::error::Fallible;
use crate::fsrs::Difficulty;
use crate::fsrs::Stability;
use crate::types::card_hash::CardHash;
use crate::types::date::Date;
use crate::types::timestamp::Timestamp;

/// An in-memory cache of card performance changes made during the current
/// session. We use this so that updates are only persisted to the database
/// when the session ends. This makes undo simpler to implement, and allows a
/// user to abort a study session without persisting their changes.
pub struct Cache {
    /// A map of card IDs to their performance changes.
    changes: HashMap<CardHash, Performance>,
}

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

impl Cache {
    /// Creates a new, empty cache.
    pub fn new() -> Self {
        Self {
            changes: HashMap::new(),
        }
    }

    /// Insert's a card performance information. If the hash is already in
    /// the cache, returns an error.
    pub fn insert(&self, card_hash: CardHash, performance: Performance) -> Fallible<()> {
        todo!()
    }

    /// Retrieve a card's performance information. If the hash is not in the
    /// cache, returns an error.
    pub fn get(&self, card_hash: CardHash) -> Fallible<&Performance> {
        todo!()
    }

    /// Update's a card's performance information. If the hash is not in the
    /// cache, returns an error.
    pub fn update(
        &self,
        card_hash: CardHash,
        last_reviewed_at: Timestamp,
        stability: Stability,
        difficulty: Difficulty,
        due_date: Date,
    ) -> Fallible<()> {
        todo!()
    }

    /// Consumes the cache, returning the underlying hash map.
    pub fn into_inner(self) -> HashMap<CardHash, Performance> {
        self.changes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
