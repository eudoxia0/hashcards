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

// Schema:
//
// pragma foreign_keys = on;
//
// create table cards (
//     card_hash text primary key,
//     added_at text not null,
//     last_reviewed_at text,
//     stability real,
//     difficulty real,
//     due_date text,
//     review_count integer not null
// ) strict;
//
// create table sessions (
//     session_id integer primary key,
//     started_at text not null,
//     ended_at text not null
// ) strict;
//
// create table reviews (
//     review_id integer primary key,
//     session_id integer not null
//         references sessions (session_id)
//         on update cascade
//         on delete cascade,
//     card_hash text not null
//         references cards (card_hash)
//         on update cascade
//         on delete cascade,
//     reviewed_at text not null,
//     grade text not null,
//     stability real not null,
//     difficulty real not null,
//     due_date text not null
// ) strict;

use std::collections::HashMap;
use std::collections::HashSet;

use rusqlite::Connection;
use rusqlite::Transaction;
use rusqlite::config::DbConfig;
use rusqlite::params;

use crate::error::ErrorReport;
use crate::error::Fallible;
use crate::error::fail;
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

#[derive(Clone)]
pub struct CardPerformance {
    pub last_reviewed_at: Timestamp,
    pub stability: Stability,
    pub difficulty: Difficulty,
    pub due_date: Date,
    pub review_count: usize,
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
        let mut conn = Connection::open(database_path)?;
        conn.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_FKEY, true)?;
        {
            let tx = conn.transaction()?;
            if !probe_schema_exists(&tx)? {
                tx.execute_batch(include_str!("schema.sql"))?;
                tx.commit()?;
            }
        }
        Ok(Self {
            conn,
            changes: HashMap::new(),
        })
    }

    /// Add a new card to the database.
    pub fn add_card(&self, card_hash: CardHash, now: Timestamp) -> Fallible<()> {
        let sql = "insert into cards (card_hash, added_at, review_count) values (?, ?, 0);";
        self.conn.execute(sql, params![card_hash, now])?;
        Ok(())
    }

    /// Update a card's performance data in the in-memory cache. This change
    /// will be persisted in the [`Self::save_session`] method.
    pub fn update_card(
        &mut self,
        card_hash: CardHash,
        reviewed_at: Timestamp,
        stability: Stability,
        difficulty: Difficulty,
        due_date: Date,
    ) -> Fallible<()> {
        // If the card's performance data is in the cache, update it.
        // Otherwise, load it from the database, update it, and store it
        // in the cache.
        match self.changes.get_mut(&card_hash) {
            Some(perf) => {
                perf.last_reviewed_at = reviewed_at;
                perf.stability = stability;
                perf.difficulty = difficulty;
                perf.due_date = due_date;
                perf.review_count += 1;
                Ok(())
            }
            None => {
                let perf = self.get_card_performance_directly(card_hash)?;
                match perf {
                    None => {
                        // The card has never been reviewed before. Create a new
                        // performance record for it.
                        let perf = CardPerformance {
                            last_reviewed_at: reviewed_at,
                            stability,
                            difficulty,
                            due_date,
                            review_count: 1,
                        };
                        self.changes.insert(card_hash, perf);
                    }
                    Some(perf) => {
                        // The card has been reviewed before. Update its
                        // performance data.
                        let perf = CardPerformance {
                            last_reviewed_at: reviewed_at,
                            stability,
                            difficulty,
                            due_date,
                            review_count: perf.review_count + 1,
                        };
                        self.changes.insert(card_hash, perf);
                    }
                }
                Ok(())
            }
        }
    }

    /// Retrieve a card's performance data.
    pub fn get_card_performance(
        &mut self,
        card_hash: CardHash,
    ) -> Fallible<Option<CardPerformance>> {
        // If the card's hash is in the in-memory cache, return that.
        // Otherwise, read from the database, and store the record in the
        // cache.
        match self.changes.get(&card_hash) {
            Some(perf) => Ok(Some(perf.clone())),
            None => {
                let perf = self.get_card_performance_directly(card_hash)?;
                if let Some(perf) = &perf {
                    self.changes.insert(card_hash, perf.clone());
                }
                Ok(perf)
            }
        }
    }

    /// Read a card's performance data directly from the database, bypassing the in-memory cache.
    fn get_card_performance_directly(
        &self,
        card_hash: CardHash,
    ) -> Fallible<Option<CardPerformance>> {
        let sql = "select last_reviewed_at, stability, difficulty, due_date, review_count from cards where card_hash = ?;";
        let mut stmt = self.conn.prepare(sql)?;
        let mut rows = stmt.query(params![card_hash])?;
        if let Some(row) = rows.next()? {
            // Data in the database may be null, in which case we return None.
            let last_reviewed_at: Option<Timestamp> = row.get(0)?;
            let stability: Option<Stability> = row.get(1)?;
            let difficulty: Option<Difficulty> = row.get(2)?;
            let due_date: Option<Date> = row.get(3)?;
            let review_count: i64 = row.get(4)?;
            if let (Some(last_reviewed_at), Some(stability), Some(difficulty), Some(due_date)) =
                (last_reviewed_at, stability, difficulty, due_date)
            {
                let perf = CardPerformance {
                    last_reviewed_at,
                    stability,
                    difficulty,
                    due_date,
                    review_count: review_count as usize,
                };
                Ok(Some(perf))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// Delete the card with the given hash, and all its reviews.
    pub fn delete_card(&mut self, card_hash: CardHash) -> Fallible<()> {
        let sql = "delete from cards where card_hash = ?;";
        self.conn.execute(sql, params![card_hash])?;
        Ok(())
    }

    /// Return the set of all card hashes in the database.
    pub fn card_hashes(&self) -> Fallible<HashSet<CardHash>> {
        let sql = "select card_hash from cards;";
        let mut stmt = self.conn.prepare(sql)?;
        let mut rows = stmt.query([])?;
        let mut hashes = HashSet::new();
        while let Some(row) = rows.next()? {
            let hash: CardHash = row.get(0)?;
            hashes.insert(hash);
        }
        Ok(hashes)
    }

    /// Find the hashes of the cards due today.
    pub fn due_today(&self, today: Date) -> Fallible<HashSet<CardHash>> {
        let mut due = HashSet::new();
        let sql = "select card_hash from cards where due_date <= ?;";
        let mut stmt = self.conn.prepare(sql)?;
        let mut rows = stmt.query(params![today])?;
        while let Some(row) = rows.next()? {
            let hash: CardHash = row.get(0)?;
            due.insert(hash);
        }
        Ok(due)
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
        let tx = self.conn.transaction()?;

        // Insert the session.
        let sql = "insert into sessions (started_at, ended_at) values (?, ?) returning session_id;";
        let session_id: i64 = tx.query_row(sql, params![started_at, ended_at], |row| row.get(0))?;

        // Insert the reviews.
        let sql = "insert into reviews (session_id, card_hash, reviewed_at, grade, stability, difficulty, due_date) values (?, ?, ?, ?, ?, ?, ?)";
        for review in &reviews {
            tx.execute(
                sql,
                params![
                    session_id,
                    review.card_hash,
                    review.reviewed_at,
                    review.grade,
                    review.stability,
                    review.difficulty,
                    review.due_date
                ],
            )?;
        }

        // Update the cards with their new performance data.
        let sql = "update cards set last_reviewed_at = ?, stability = ?, difficulty = ?, due_date = ?, review_count = ? where card_hash = ?";
        for (card_hash, perf) in &self.changes {
            tx.execute(
                sql,
                params![
                    perf.last_reviewed_at,
                    perf.stability,
                    perf.difficulty,
                    perf.due_date,
                    perf.review_count as i64,
                    card_hash
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
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

    #[test]
    fn test_empty_database() -> Fallible<()> {
        let mut db = Database::new(":memory:")?;
        assert_eq!(db.card_hashes()?, HashSet::new());
        assert_eq!(db.due_today(Timestamp::now().local_date())?, HashSet::new());
        let hash = CardHash::hash_bytes(b"a");
        assert!(db.get_card_performance(hash)?.is_none());
        Ok(())
    }

    #[test]
    fn test_add_card() -> Fallible<()> {
        let mut db = Database::new(":memory:")?;
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

    #[test]
    fn test_update_card_and_delete_card() -> Fallible<()> {
        let mut db = Database::new(":memory:")?;
        let hash = CardHash::hash_bytes(b"a");
        let now = Timestamp::now();
        db.add_card(hash, now)?;
        let stability = 1.0;
        let difficulty = 1.0;
        let due_date = now.local_date();
        db.update_card(hash, now, stability, difficulty, due_date)?;
        db.save_session(now, now, vec![])?;
        let perf = db.get_card_performance(hash)?;
        assert!(perf.is_some());
        let perf = perf.unwrap();
        assert_eq!(perf.stability, stability);
        assert_eq!(perf.difficulty, difficulty);
        assert_eq!(perf.due_date, due_date);
        assert_eq!(perf.review_count, 1);
        assert_eq!(perf.last_reviewed_at, now);
        db.delete_card(hash)?;
        let hashes = db.card_hashes()?;
        assert!(!hashes.contains(&hash));
        Ok(())
    }

    #[test]
    fn test_due_today() -> Fallible<()> {
        let mut db = Database::new(":memory:")?;
        let hash = CardHash::hash_bytes(b"a");
        let now = Timestamp::now();
        db.add_card(hash, now)?;
        let today = now.local_date();
        db.update_card(hash, now, 1.0, 1.0, today)?;
        db.save_session(now, now, vec![])?;
        let due_today = db.due_today(today)?;
        assert_eq!(due_today.len(), 1);
        assert!(due_today.contains(&hash));
        Ok(())
    }

    #[test]
    fn test_save_session() -> Fallible<()> {
        let mut db = Database::new(":memory:")?;
        let hash = CardHash::hash_bytes(b"a");
        let now = Timestamp::now();
        db.add_card(hash, now)?;
        let reviews = vec![ReviewRecord {
            card_hash: hash,
            reviewed_at: now,
            grade: Grade::Good,
            stability: 1.0,
            difficulty: 1.0,
            due_date: now.local_date(),
        }];
        db.save_session(now, now, reviews)?;
        Ok(())
    }
}
