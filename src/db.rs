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
use rusqlite::params;

use crate::error::Fallible;
use crate::error::fail;
use crate::fsrs::Difficulty;
use crate::fsrs::Grade;
use crate::fsrs::Stability;
use crate::types::card_hash::CardHash;
use crate::types::date::Date;
use crate::types::performance::Performance;
use crate::types::performance::ReviewedPerformance;
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
    pub interval_raw: f64,
    pub interval_days: usize,
    pub due_date: Date,
}

pub struct SessionRow {
    pub session_id: i64,
    pub started_at: Timestamp,
    pub ended_at: Timestamp,
}

pub struct ReviewRow {
    pub review_id: i64,
    pub data: ReviewRecord,
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
    pub fn insert_card(&self, card_hash: CardHash, added_at: Timestamp) -> Fallible<()> {
        if self.card_exists(card_hash)? {
            return fail("Card already exists");
        }
        let sql = "insert into cards (card_hash, added_at, review_count) values (?, ?, 0);";
        self.conn.execute(sql, params![card_hash, added_at])?;
        Ok(())
    }

    /// Return the set of all card hashes in the database.
    pub fn card_hashes(&self) -> Fallible<HashSet<CardHash>> {
        let sql = "select card_hash from cards;";
        let mut stmt = self.conn.prepare(sql)?;
        let card_iter = stmt.query_map([], |row| {
            let card_hash: CardHash = row.get(0)?;
            Ok(card_hash)
        })?;
        let mut card_hashes = HashSet::new();
        for card in card_iter {
            card_hashes.insert(card?);
        }
        Ok(card_hashes)
    }

    /// Find the hashes of the cards due today.
    pub fn due_today(&self, today: Date) -> Fallible<HashSet<CardHash>> {
        let mut due = HashSet::new();
        let sql = "select card_hash, due_date from cards;";
        let mut stmt = self.conn.prepare(sql)?;
        let mut rows = stmt.query(params![])?;
        while let Some(row) = rows.next()? {
            let hash: CardHash = row.get(0)?;
            let due_date: Option<Date> = row.get(1)?;
            match due_date {
                None => {
                    // Never reviewed, so it's due.
                    due.insert(hash);
                }
                Some(due_date) => {
                    if due_date <= today {
                        due.insert(hash);
                    }
                }
            }
        }
        Ok(due)
    }

    /// Get a card's performance information.
    pub fn get_card_performance_opt(&self, card_hash: CardHash) -> Fallible<Option<Performance>> {
        let sql = "select last_reviewed_at, stability, difficulty, interval_raw, interval_days, due_date, review_count from cards where card_hash = ?;";
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params![card_hash], |row| {
            let last_reviewed_at: Option<Timestamp> = row.get(0)?;
            let stability: Option<Stability> = row.get(1)?;
            let difficulty: Option<Difficulty> = row.get(2)?;
            let interval_raw: Option<f64> = row.get(3)?;
            let interval_days: Option<usize> = row.get(4)?;
            let due_date: Option<Date> = row.get(5)?;
            let review_count: i32 = row.get(6)?;
            if let (
                Some(last_reviewed_at),
                Some(stability),
                Some(difficulty),
                Some(interval_raw),
                Some(interval_days),
                Some(due_date),
            ) = (
                last_reviewed_at,
                stability,
                difficulty,
                interval_raw,
                interval_days,
                due_date,
            ) {
                Ok(Performance::Reviewed(ReviewedPerformance {
                    last_reviewed_at,
                    stability,
                    difficulty,
                    interval_raw,
                    interval_days,
                    due_date,
                    review_count: review_count as usize,
                }))
            } else {
                Ok(Performance::New)
            }
        })?;
        if let Some(row) = rows.into_iter().next() {
            Ok(Some(row?))
        } else {
            Ok(None)
        }
    }

    /// Get a card's performance information. If the card does not exist,
    /// returns an error.
    pub fn get_card_performance(&self, card_hash: CardHash) -> Fallible<Performance> {
        match self.get_card_performance_opt(card_hash)? {
            Some(performance) => Ok(performance),
            None => fail(format!(
                "No performance data found for card with hash {card_hash}"
            )),
        }
    }

    /// Update a card's performance information.
    ///
    /// If no card with the given hash exists, returns an error.
    pub fn update_card_performance(
        &self,
        card_hash: CardHash,
        performance: Performance,
    ) -> Fallible<()> {
        if !self.card_exists(card_hash)? {
            return fail("Card not found");
        }
        let (
            last_reviewed_at,
            stability,
            difficulty,
            interval_raw,
            interval_days,
            due_date,
            review_count,
        ) = match performance {
            Performance::New => (None, None, None, None, None, None, 0),
            Performance::Reviewed(rp) => (
                Some(rp.last_reviewed_at),
                Some(rp.stability),
                Some(rp.difficulty),
                Some(rp.interval_raw),
                Some(rp.interval_days as i32),
                Some(rp.due_date),
                rp.review_count as i32,
            ),
        };
        let sql = "update cards set last_reviewed_at = ?, stability = ?, difficulty = ?, interval_raw = ?, interval_days = ?, due_date = ?, review_count = ? where card_hash = ?;";
        self.conn.execute(
            sql,
            params![
                last_reviewed_at,
                stability,
                difficulty,
                interval_raw,
                interval_days,
                due_date,
                review_count,
                card_hash
            ],
        )?;
        Ok(())
    }

    /// Save a session.
    pub fn save_session(
        &mut self,
        started_at: Timestamp,
        ended_at: Timestamp,
        reviews: Vec<ReviewRecord>,
    ) -> Fallible<()> {
        let tx = self.conn.transaction()?;
        let sql = "insert into sessions (started_at, ended_at) values (?, ?) returning session_id;";
        let session_id: i64 = tx.query_row(sql, params![started_at, ended_at], |row| row.get(0))?;
        for review in reviews {
            let sql = "insert into reviews (session_id, card_hash, reviewed_at, grade, stability, difficulty, interval_raw, interval_days, due_date) values (?, ?, ?, ?, ?, ?, ?, ?, ?);";
            tx.execute(
                sql,
                params![
                    session_id,
                    review.card_hash,
                    review.reviewed_at,
                    review.grade,
                    review.stability,
                    review.difficulty,
                    review.interval_raw,
                    review.interval_days as i32,
                    review.due_date
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Delete a card and its reviews.
    ///
    /// If no card with the given hash exists, returns an error.
    pub fn delete_card(&self, card_hash: CardHash) -> Fallible<()> {
        if !self.card_exists(card_hash)? {
            return fail("Card not found");
        }
        let sql = "delete from reviews where card_hash = ?;";
        self.conn.execute(sql, params![card_hash])?;
        let sql = "delete from cards where card_hash = ?;";
        self.conn.execute(sql, params![card_hash])?;
        Ok(())
    }

    /// Does a card with the given hash exist?
    fn card_exists(&self, card_hash: CardHash) -> Fallible<bool> {
        let sql = "select count(*) from cards where card_hash = ?;";
        let count: i64 = self.conn.query_row(sql, [card_hash], |row| row.get(0))?;
        Ok(count > 0)
    }

    /// Count the number of reviews performed in the given date.
    pub fn count_reviews_in_date(&self, date: Date) -> Fallible<usize> {
        let sql = "select count(*) from reviews where substr(reviewed_at, 1, 10) = ?;";
        let count: i64 = self.conn.query_row(sql, params![date], |row| row.get(0))?;
        Ok(count as usize)
    }

    /// Get the list of all sessions in the database.
    pub fn get_all_sessions(&self) -> Fallible<Vec<SessionRow>> {
        let sql = "select session_id, started_at, ended_at from sessions order by started_at;";
        let mut stmt = self.conn.prepare(sql)?;
        let session_iter = stmt.query_map([], |row| {
            Ok(SessionRow {
                session_id: row.get(0)?,
                started_at: row.get(1)?,
                ended_at: row.get(2)?,
            })
        })?;
        let mut sessions = Vec::new();
        for session in session_iter {
            sessions.push(session?);
        }
        Ok(sessions)
    }

    /// Get the list of all reviews for a given session.
    pub fn get_reviews_for_session(&self, session_id: i64) -> Fallible<Vec<ReviewRow>> {
        let sql = "select review_id, card_hash, reviewed_at, grade, stability, difficulty, interval_raw, interval_days, due_date from reviews where session_id = ? order by reviewed_at;";
        let mut stmt = self.conn.prepare(sql)?;
        let review_iter = stmt.query_map(params![session_id], |row| {
            Ok(ReviewRow {
                review_id: row.get(0)?,
                data: ReviewRecord {
                    card_hash: row.get(1)?,
                    reviewed_at: row.get(2)?,
                    grade: row.get(3)?,
                    stability: row.get(4)?,
                    difficulty: row.get(5)?,
                    interval_raw: row.get(6)?,
                    interval_days: row.get(7)?,
                    due_date: row.get(8)?,
                },
            })
        })?;
        let mut reviews = Vec::new();
        for review in review_iter {
            reviews.push(review?);
        }
        Ok(reviews)
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
    /// `due_today` returns it since it's new.
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
        let due_today = db.due_today(now.date())?;
        assert!(due_today.contains(&card_hash));
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
        assert_eq!(err.to_string(), "error: Card already exists");
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
            interval_raw: 1.0,
            interval_days: 1,
            due_date: now.date(),
            review_count: 1,
        });
        db.update_card_performance(card_hash, performance)?;
        let fetched_performance = db.get_card_performance(card_hash)?;
        assert_eq!(fetched_performance, performance);
        let due_today = db.due_today(now.date())?;
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
        assert_eq!(
            err.to_string(),
            format!("error: No performance data found for card with hash {card_hash}")
        );
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
        assert_eq!(err.to_string(), "error: Card not found");
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
            interval_raw: 1.0,
            interval_days: 1,
            due_date: now.date(),
        };
        db.save_session(now, now, vec![review])?;

        let sessions = db.get_all_sessions()?;
        assert_eq!(sessions.len(), 1);
        let session = &sessions[0];
        assert_eq!(session.started_at, now);
        assert_eq!(session.ended_at, now);
        let reviews = db.get_reviews_for_session(session.session_id)?;
        assert_eq!(reviews.len(), 1);
        let fetched_review = &reviews[0];
        assert_eq!(fetched_review.data.card_hash, card_hash);
        assert_eq!(fetched_review.data.reviewed_at, now);
        assert_eq!(fetched_review.data.grade, Grade::Good);
        assert_eq!(fetched_review.data.stability, 2.0);
        assert_eq!(fetched_review.data.difficulty, 2.0);
        assert_eq!(fetched_review.data.interval_raw, 1.0);
        assert_eq!(fetched_review.data.interval_days, 1);
        assert_eq!(fetched_review.data.due_date, now.date());
        Ok(())
    }

    /// Trying to delete a non-existent card returns an error.
    #[test]
    fn test_delete_nonexistent_card() -> Fallible<()> {
        let db = Database::new(":memory:")?;
        let card_hash = CardHash::hash_bytes(b"a");
        let result = db.delete_card(card_hash);
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert_eq!(err.to_string(), "error: Card not found");
        Ok(())
    }

    /// Delete a card and see that it is gone.
    #[test]
    fn test_delete_card() -> Fallible<()> {
        let db = Database::new(":memory:")?;
        let card_hash = CardHash::hash_bytes(b"a");
        let now = Timestamp::now();
        db.insert_card(card_hash, now)?;
        db.delete_card(card_hash)?;
        let result = db.get_card_performance(card_hash);
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert_eq!(
            err.to_string(),
            format!("error: No performance data found for card with hash {card_hash}")
        );
        Ok(())
    }
}
