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

use rusqlite::Connection;

use crate::error::Fallible;
use crate::fsrs::Difficulty;
use crate::fsrs::Grade;
use crate::fsrs::Stability;
use crate::types::card_hash::CardHash;
use crate::types::date::Date;
use crate::types::timestamp::Timestamp;

pub struct Database {
    /// The SQLite database connection.
    conn: Connection,
    /// An in-memory cache of card performance changes made during the current
    /// session. We use this so that updates are only persisted to the database
    /// when the session ends. This makes undo simpler to implement, and allows
    /// a user to abort a study session without persisting their changes.
    changes: HashMap<CardHash, CardPerformance>,
}

pub struct CardPerformance {
    pub stability: Stability,
    pub difficulty: Difficulty,
    pub due_date: Date,
    pub review_count: usize,
    pub last_reviewed_at: Timestamp,
}

pub struct ReviewRecord {
    pub card_hash: CardHash,
    pub reviewed_at: Timestamp,
    pub grade: Grade,
    pub stability: Stability,
    pub difficulty: Difficulty,
    pub due_date: Date,
}

impl Database {
    pub fn new(database_path: &str) -> Fallible<Self> {
        todo!()
    }

    /// Add a new card to the database.
    pub fn add_card(&self, card_hash: CardHash, now: Timestamp) -> Fallible<()> {
        todo!()
    }

    /// Update a card's performance data.
    pub fn update_card(
        &self,
        card_hash: CardHash,
        reviewed_at: Timestamp,
        stability: Stability,
        difficulty: Difficulty,
        due_date: Date,
    ) -> Fallible<()> {
        todo!()
    }

    /// Retrieve a card's performance data.
    pub fn get_card_performance(&self, card_hash: CardHash) -> Fallible<Option<CardPerformance>> {
        todo!()
    }

    /// Delete the card with the given hash, and all its reviews.
    pub fn delete_card(&mut self, card_hash: CardHash) -> Fallible<()> {
        todo!()
    }

    /// Return the set of all card hashes in the database.
    pub fn card_hashes(&self) -> Fallible<HashSet<CardHash>> {
        todo!()
    }

    /// Find the hashes of the cards due today.
    pub fn due_today(&self, today: Date) -> Fallible<HashSet<CardHash>> {
        todo!()
    }

    /// Save a study session and its reviews to the database.
    ///
    /// This persists changes to card performance made during the session.
    pub fn save_session(
        &mut self,
        started_at: Timestamp,
        ended_at: Timestamp,
        reviews: Vec<ReviewRecord>,
    ) -> Fallible<()> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_database() -> Fallible<()> {
        let db = Database::new(":memory:")?;
        assert_eq!(db.card_hashes()?, HashSet::new());
        assert_eq!(db.due_today(Timestamp::now().local_date())?, HashSet::new());
        let hash = CardHash::hash_bytes(b"a");
        assert!(db.get_card_performance(hash)?.is_none());
        Ok(())
    }

    #[test]
    fn test_add_card() -> Fallible<()> {
        let db = Database::new(":memory:")?;
        let hash = CardHash::hash_bytes(b"a");
        let now = Timestamp::now();
        db.add_card(hash, now)?;
        let hashes = db.card_hashes()?;
        assert_eq!(hashes.len(), 1);
        assert!(hashes.contains(&hash));
        let perf = db.get_card_performance(hash)?;
        assert!(perf.is_none());
        Ok(())
    }
}
