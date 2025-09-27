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

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

use chrono::DateTime;
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
use crate::hash::Hash;

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

    pub fn acquire(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap()
    }
}

// create table cards (
//     card_hash text primary key,
//     card_type text not null,
//     deck_name text not null,
//     question text not null,
//     answer text not null,
//     cloze_start integer not null,
//     cloze_end integer not null
// ) strict;

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

impl From<DateTime<Utc>> for Timestamp {
    fn from(dt: DateTime<Utc>) -> Self {
        Timestamp(dt)
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
        Ok(Timestamp::from(ts))
    }
}

fn insert_session(tx: &Transaction, started_at: Timestamp, ended_at: Timestamp) -> Fallible<()> {
    let sql = "insert into sessions (started_at, ended_at) values (?, ?);";
    tx.execute(sql, (started_at, ended_at))?;
    Ok(())
}

fn probe_schema_exists(tx: &Transaction) -> Fallible<bool> {
    let sql = "select count(*) from sqlite_master where type='table' AND name=?;";
    let count: i64 = tx.query_row(sql, ["cards"], |row| row.get(0))?;
    Ok(count > 0)
}
