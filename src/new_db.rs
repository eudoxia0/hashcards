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

use std::collections::HashSet;

use rusqlite::Connection;
use rusqlite::Transaction;
use rusqlite::config::DbConfig;

use crate::error::Fallible;
use crate::fsrs::Grade;
use crate::types::card_hash::CardHash;
use crate::types::date::Date;
use crate::types::performance::Performance;
use crate::types::timestamp::Timestamp;

pub struct Database {
    conn: Connection,
}

pub struct ReviewRecord {
    pub card_hash: CardHash,
    pub reviewed_at: Timestamp,
    pub grade: Grade,
    pub stability: f64,
    pub difficulty: f64,
    pub due_date: Date,
}

impl Database {
    pub fn new(database_path: &str) -> Fallible<Self> {
        let mut conn = Connection::open(database_path)?;
        conn.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_FKEY, true)?;
        {
            let tx = conn.transaction()?;
            if !probe_schema_exists(&tx)? {
                tx.execute_batch(include_str!("schema.sql"))?;
                tx.commit()?;
            }
        }
        Ok(Self { conn })
    }

    /// Insert a new card in the database.
    ///
    /// If a card with the given hash exists, returns an error.
    pub fn insert_card(&self, card_hash: CardHash, added_at: Timestamp) -> Fallible<Self> {
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

    /// Get a card's performance information.
    ///
    /// If no card with the given hash exists, returns an error.
    pub fn get_card_performance(&self, card_hash: CardHash) -> Fallible<Performance> {
        todo!()
    }

    /// Update a card's performance information.
    ///
    /// If no card with the given hash exists, returns an error.
    pub fn update_card_performance(
        &self,
        card_hash: CardHash,
        performance: Performance,
    ) -> Fallible<()> {
        todo!()
    }

    /// Save a session.
    pub fn save_session(
        &mut self,
        started_at: Timestamp,
        ended_at: Timestamp,
        reviews: Vec<ReviewRecord>,
    ) -> Fallible<()> {
        todo!()
    }
}

fn probe_schema_exists(tx: &Transaction) -> Fallible<bool> {
    let sql = "select count(*) from sqlite_master where type='table' AND name=?;";
    let count: i64 = tx.query_row(sql, ["cards"], |row| row.get(0))?;
    Ok(count > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsrs::Grade;
    use crate::types::performance::ReviewedPerformance;

    #[test]
    fn test_probe_schema_exists() -> Fallible<()> {
        let mut conn = Connection::open_in_memory()?;
        let tx = conn.transaction()?;
        assert!(!probe_schema_exists(&tx)?);
        Ok(())
    }

    /// Insert a card, and see that its hash is returned by `card_hashes`, and
    /// that `get_card_performance` returns an initial empty performance, and
    /// `due_today` does not return it.
    #[test]
    fn test_insert_card() -> Fallible<()> {
        let db = Database::new(":memory:")?;
        let card_hash = CardHash::hash_bytes(b"a");
        let now = Timestamp::now();
        db.insert_card(card_hash, now)?;
        let hashes = db.card_hashes()?;
        assert!(hashes.contains(&card_hash));
        let performance = db.get_card_performance(card_hash)?;
        assert_eq!(performance, Performance::New);
        let due_today = db.due_today(now.local_date())?;
        assert!(!due_today.contains(&card_hash));
        Ok(())
    }

    /// Inserting a card twice returns an error.
    #[test]
    fn test_insert_twice() -> Fallible<()> {
        let db = Database::new(":memory:")?;
        let card_hash = CardHash::hash_bytes(b"a");
        let now = Timestamp::now();
        db.insert_card(card_hash, now)?;
        let result = db.insert_card(card_hash, now);
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert_eq!(err.to_string(), "Card already exists");
        Ok(())
    }

    /// Updating a card's performance, and checking that `get_card_performance`
    /// works and that `due_today` returns the card.
    #[test]
    fn test_update_performance() -> Fallible<()> {
        let db = Database::new(":memory:")?;
        let card_hash = CardHash::hash_bytes(b"a");
        let now = Timestamp::now();
        db.insert_card(card_hash, now)?;
        let performance = Performance::Reviewed(ReviewedPerformance {
            last_reviewed_at: now,
            stability: 2.0,
            difficulty: 2.0,
            due_date: now.local_date(),
            review_count: 1,
        });
        db.update_card_performance(card_hash, performance.clone())?;
        let fetched_performance = db.get_card_performance(card_hash)?;
        assert_eq!(fetched_performance, performance);
        let due_today = db.due_today(now.local_date())?;
        assert!(due_today.contains(&card_hash));
        Ok(())
    }

    /// `get_card_performance` fails if the card does not exist.
    #[test]
    fn test_get_performance_nonexistent() -> Fallible<()> {
        let db = Database::new(":memory:")?;
        let card_hash = CardHash::hash_bytes(b"a");
        let result = db.get_card_performance(card_hash);
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert_eq!(err.to_string(), "Card not found");
        Ok(())
    }

    /// `update_card_performance` fails if the card does not exist.
    #[test]
    fn test_update_performance_nonexistent() -> Fallible<()> {
        let db = Database::new(":memory:")?;
        let card_hash = CardHash::hash_bytes(b"a");
        let performance = Performance::New;
        let result = db.update_card_performance(card_hash, performance);
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert_eq!(err.to_string(), "Card not found");
        Ok(())
    }

    /// Save a session.
    #[test]
    fn test_save_session() -> Fallible<()> {
        let mut db = Database::new(":memory:")?;
        let card_hash = CardHash::hash_bytes(b"a");
        let now = Timestamp::now();
        db.insert_card(card_hash, now)?;
        let review = ReviewRecord {
            card_hash,
            reviewed_at: now,
            grade: Grade::Good,
            stability: 2.0,
            difficulty: 2.0,
            due_date: now.local_date(),
        };
        db.save_session(now, now, vec![review])?;
        Ok(())
    }
}
