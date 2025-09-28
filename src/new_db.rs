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
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

use chrono::DateTime;
use chrono::Local;
use chrono::NaiveDate;
use chrono::Utc;
use rusqlite::Connection;
use rusqlite::ToSql;
use rusqlite::Transaction;
use rusqlite::config::DbConfig;
use rusqlite::types::FromSql;
use rusqlite::types::FromSqlError;
use rusqlite::types::FromSqlResult;
use rusqlite::types::ToSqlOutput;
use rusqlite::types::ValueRef;

use crate::error::ErrorReport;
use crate::error::Fallible;
use crate::error::fail;
use crate::fsrs::D;
use crate::fsrs::Grade;
use crate::fsrs::S;
use crate::hash::Hash;
use crate::parser::Card;
use crate::parser::CardContent;

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
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
        let conn = Arc::new(Mutex::new(conn));
        Ok(Self { conn })
    }

    /// Return the set of all card hashes in the database.
    pub fn card_hashes(&self) -> Fallible<HashSet<Hash>> {
        let mut hashes = HashSet::new();
        let conn = self.acquire();
        let mut stmt = conn.prepare("select card_hash from cards;")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let hash: Hash = row.get(0)?;
            hashes.insert(hash);
        }
        Ok(hashes)
    }

    /// Add a new card to the database.
    pub fn add_card(&self, card: &Card) -> Fallible<()> {
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
            },
            CardContent::Cloze { text, start, end } => CardRow {
                card_hash: card.hash(),
                card_type: CardType::Cloze,
                deck_name: card.deck_name().to_string(),
                question: text.to_string(),
                answer: "".to_string(),
                cloze_start: *start,
                cloze_end: *end,
            },
        };
        let mut conn = self.acquire();
        let tx = conn.transaction()?;
        insert_card(&tx, &card_row)?;
        tx.commit()?;
        Ok(())
    }

    /// Find the set of cards due today.
    pub fn due_today(&self, today: Date) -> Fallible<HashSet<Hash>> {
        let mut due = HashSet::new();
        let conn = self.acquire();
        let mut stmt = conn.prepare("select c.card_hash, max(r.due_date) from cards c left outer join reviews r on r.card_hash = c.card_hash group by c.card_hash;")?;
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

    fn acquire(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap()
    }
}

enum CardType {
    Basic,
    Cloze,
}

impl CardType {
    fn as_str(&self) -> &str {
        match self {
            CardType::Basic => "basic",
            CardType::Cloze => "cloze",
        }
    }
}

impl TryFrom<String> for CardType {
    type Error = ErrorReport;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "basic" => Ok(CardType::Basic),
            "cloze" => Ok(CardType::Cloze),
            _ => fail(format!("Invalid card type: {}", value)),
        }
    }
}

impl ToSql for CardType {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_str()))
    }
}

impl FromSql for CardType {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let string: String = FromSql::column_result(value)?;
        CardType::try_from(string).map_err(|e| FromSqlError::Other(Box::new(e)))
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
}

fn insert_card(tx: &Transaction, card: &CardRow) -> Fallible<()> {
    let sql = "insert into cards (card_hash, card_type, deck_name, question, answer, cloze_start, cloze_end) values (?, ?, ?, ?, ?, ?, ?);";
    tx.execute(
        sql,
        (
            card.card_hash,
            &card.card_type,
            &card.deck_name,
            &card.question,
            &card.answer,
            card.cloze_start,
            card.cloze_end,
        ),
    )?;
    Ok(())
}

pub struct Timestamp(DateTime<Utc>);

impl Timestamp {
    pub fn now() -> Self {
        Self(Utc::now())
    }
}

impl ToSql for Timestamp {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        let str = self.0.to_rfc3339();
        Ok(ToSqlOutput::from(str))
    }
}

impl FromSql for Timestamp {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let string: String = FromSql::column_result(value)?;
        let ts =
            DateTime::parse_from_rfc3339(&string).map_err(|e| FromSqlError::Other(Box::new(e)))?;
        let ts = ts.with_timezone(&Utc);
        Ok(Timestamp(ts))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date(NaiveDate);

impl Date {
    pub fn today() -> Self {
        Self(Local::now().naive_local().date())
    }
}

impl ToSql for Date {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        let str = self.0.format("%Y-%m-%d").to_string();
        Ok(ToSqlOutput::from(str))
    }
}

impl FromSql for Date {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let string: String = FromSql::column_result(value)?;
        let date = NaiveDate::parse_from_str(&string, "%Y-%m-%d")
            .map_err(|_| ErrorReport::new(format!("invalid date: {}", string)))
            .map_err(|e| FromSqlError::Other(Box::new(e)))?;
        Ok(Date(date))
    }
}

type SessionId = i64;

fn insert_session(
    tx: &Transaction,
    started_at: Timestamp,
    ended_at: Timestamp,
) -> Fallible<SessionId> {
    let sql = "insert into sessions (started_at, ended_at) values (?, ?) returning session_id;";
    let session_id: SessionId = tx.query_row(sql, (started_at, ended_at), |row| row.get(0))?;
    Ok(session_id)
}

struct InsertReview {
    session_id: SessionId,
    card_hash: Hash,
    reviewed_at: Timestamp,
    grade: Grade,
    stability: S,
    difficulty: D,
    due_date: Date,
}

type ReviewId = i64;

fn insert_review(tx: &Transaction, review: &InsertReview) -> Fallible<ReviewId> {
    let sql = "insert into reviews (session_id, card_hash, reviewed_at, grade, stability, difficulty, due_date) values (?, ?, ?, ?, ?, ?, ?) returning review_id;";
    let review_id: ReviewId = tx.query_row(
        sql,
        (
            review.session_id,
            &review.card_hash,
            &review.reviewed_at,
            review.grade,
            review.stability,
            review.difficulty,
            &review.due_date,
        ),
        |row| row.get(0),
    )?;
    Ok(review_id)
}

fn probe_schema_exists(tx: &Transaction) -> Fallible<bool> {
    let sql = "select count(*) from sqlite_master where type='table' AND name=?;";
    let count: i64 = tx.query_row(sql, ["cards"], |row| row.get(0))?;
    Ok(count > 0)
}
