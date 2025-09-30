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
use crate::types::card::Card;
use crate::types::card::CardContent;
use crate::types::card_type::CardType;
use crate::types::date::Date;
use crate::types::hash::Hash;
use crate::types::review::Review;
use crate::types::timestamp::Timestamp;

pub struct Database {
    conn: Connection,
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

    /// Return the set of all card hashes in the database.
    pub fn card_hashes(&self) -> Fallible<HashSet<Hash>> {
        let mut hashes = HashSet::new();
        let mut stmt = self.conn.prepare("select card_hash from cards;")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let hash: Hash = row.get(0)?;
            hashes.insert(hash);
        }
        Ok(hashes)
    }

    /// Add a new card to the database.
    pub fn add_card(&self, card: &Card, now: Timestamp) -> Fallible<()> {
        log::debug!("Adding new card: {}", card.hash());
        let card_row = match card.content() {
            CardContent::Basic { question, answer } => CardRow {
                card_hash: card.hash(),
                card_type: CardType::Basic,
                deck_name: card.deck_name().to_string(),
                question: question.to_string(),
                answer: answer.to_string(),
                cloze_start: 0,
                cloze_end: 0,
                added_at: now,
            },
            CardContent::Cloze { text, start, end } => CardRow {
                card_hash: card.hash(),
                card_type: CardType::Cloze,
                deck_name: card.deck_name().to_string(),
                question: text.to_string(),
                answer: "".to_string(),
                cloze_start: *start,
                cloze_end: *end,
                added_at: now,
            },
        };
        insert_card(&self.conn, &card_row)?;
        Ok(())
    }

    /// Find the set of cards due today.
    pub fn due_today(&self, today: Date) -> Fallible<HashSet<Hash>> {
        let mut due = HashSet::new();
        let mut stmt = self.conn.prepare("select c.card_hash, max(r.due_date) from cards c left outer join reviews r on r.card_hash = c.card_hash group by c.card_hash;")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let hash: Hash = row.get(0)?;
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

    /// Get the latest review for a given card.
    pub fn get_latest_review(&self, card_hash: Hash) -> Fallible<Option<Review>> {
        let sql = "select reviewed_at, grade, stability, difficulty, due_date from reviews where card_hash = ? order by reviewed_at desc limit 1;";
        let mut stmt = self.conn.prepare(sql)?;
        let mut rows = stmt.query([card_hash])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Review {
                card_hash,
                reviewed_at: row.get(0)?,
                grade: row.get(1)?,
                stability: row.get(2)?,
                difficulty: row.get(3)?,
                due_date: row.get(4)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Save a study session and its reviews to the database.
    pub fn save_session(
        &mut self,
        started_at: Timestamp,
        ended_at: Timestamp,
        reviews: Vec<Review>,
    ) -> Fallible<()> {
        let tx = self.conn.transaction()?;
        let sql = "insert into sessions (started_at, ended_at) values (?, ?) returning session_id;";
        let session_id: i64 = tx.query_row(sql, (started_at, ended_at), |row| row.get(0))?;
        for review in reviews.into_iter() {
            let sql = "insert into reviews (session_id, card_hash, reviewed_at, grade, stability, difficulty, due_date) values (?, ?, ?, ?, ?, ?, ?);";
            tx.execute(
                sql,
                (
                    session_id,
                    review.card_hash,
                    review.reviewed_at,
                    review.grade,
                    review.stability,
                    review.difficulty,
                    review.due_date,
                ),
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}

struct CardRow {
    card_hash: Hash,
    card_type: CardType,
    deck_name: String,
    question: String,
    answer: String,
    cloze_start: usize,
    cloze_end: usize,
    added_at: Timestamp,
}

fn insert_card(conn: &Connection, card: &CardRow) -> Fallible<()> {
    let sql = "insert into cards (card_hash, card_type, deck_name, question, answer, cloze_start, cloze_end, added_at) values (?, ?, ?, ?, ?, ?, ?, ?);";
    conn.execute(
        sql,
        (
            card.card_hash,
            &card.card_type,
            &card.deck_name,
            &card.question,
            &card.answer,
            card.cloze_start,
            card.cloze_end,
            &card.added_at,
        ),
    )?;
    Ok(())
}

fn probe_schema_exists(tx: &Transaction) -> Fallible<bool> {
    let sql = "select count(*) from sqlite_master where type='table' AND name=?;";
    let count: i64 = tx.query_row(sql, ["cards"], |row| row.get(0))?;
    Ok(count > 0)
}

#[cfg(test)]
mod tests {
    use std::env::temp_dir;
    use std::fs::create_dir_all;
    use std::path::PathBuf;

    use chrono::Duration;

    use super::*;
    use crate::fsrs::Grade;

    #[test]
    fn test_empty_db() -> Fallible<()> {
        let db = Database::new(":memory:")?;
        assert!(db.card_hashes()?.is_empty());
        assert!(db.due_today(Timestamp::now().local_date())?.is_empty());
        Ok(())
    }

    #[test]
    fn test_add_basic_card() -> Fallible<()> {
        let db = Database::new(":memory:")?;
        let now = Timestamp::now();
        let card = Card::new(
            "My Deck".to_string(),
            PathBuf::new(),
            (0, 1),
            CardContent::new_basic("Q", "A"),
        );
        db.add_card(&card, now)?;

        let hashes = db.card_hashes()?;
        assert_eq!(hashes.len(), 1);
        assert!(hashes.contains(&card.hash()));

        let due_today = db.due_today(now.local_date())?;
        assert_eq!(due_today.len(), 1);
        assert!(due_today.contains(&card.hash()));

        let latest_review = db.get_latest_review(card.hash())?;
        assert!(latest_review.is_none());

        Ok(())
    }

    #[test]
    fn test_add_cloze_card() -> Fallible<()> {
        let db = Database::new(":memory:")?;
        let now = Timestamp::now();
        let card = Card::new(
            "My Deck".to_string(),
            PathBuf::new(),
            (0, 1),
            CardContent::Cloze {
                text: "Foo bar baz.".to_string(),
                start: 0,
                end: 3,
            },
        );
        db.add_card(&card, now)?;

        let hashes = db.card_hashes()?;
        assert_eq!(hashes.len(), 1);
        assert!(hashes.contains(&card.hash()));

        let due_today = db.due_today(now.local_date())?;
        assert_eq!(due_today.len(), 1);
        assert!(due_today.contains(&card.hash()));

        let latest_review = db.get_latest_review(card.hash())?;
        assert!(latest_review.is_none());

        Ok(())
    }

    #[test]
    fn test_session() -> Fallible<()> {
        let mut db = Database::new(":memory:")?;
        let now = Timestamp::now();
        let today = now.local_date();
        let card = Card::new(
            "My Deck".to_string(),
            PathBuf::new(),
            (0, 1),
            CardContent::Cloze {
                text: "Foo bar baz.".to_string(),
                start: 0,
                end: 3,
            },
        );
        db.add_card(&card, now)?;

        let review = Review {
            card_hash: card.hash(),
            reviewed_at: now,
            grade: Grade::Easy,
            stability: 2.5,
            difficulty: 2.0,
            due_date: Date::new(today.into_inner() + Duration::days(5)),
        };

        db.save_session(now, now, vec![review.clone()])?;

        let latest_review = db.get_latest_review(card.hash())?;
        assert!(latest_review.is_some());
        let latest_review = latest_review.unwrap();
        assert_eq!(latest_review.card_hash, review.card_hash);
        assert_eq!(latest_review.reviewed_at, review.reviewed_at);
        assert_eq!(latest_review.grade, review.grade);
        assert_eq!(latest_review.stability, review.stability);
        assert_eq!(latest_review.difficulty, review.difficulty);
        assert_eq!(latest_review.due_date, review.due_date);

        Ok(())
    }

    #[test]
    fn test_open_db_twice() -> Fallible<()> {
        let dir = temp_dir();
        let dir = dir.join("test_open_db_twice");
        create_dir_all(&dir)?;
        let db_path = dir.join("test.db");
        let _a = Database::new(db_path.to_str().unwrap())?;
        let _b = Database::new(db_path.to_str().unwrap())?;
        Ok(())
    }
}
