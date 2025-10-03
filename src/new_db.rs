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

use std::collections::HashSet;

use rusqlite::Connection;
use rusqlite::Transaction;
use rusqlite::config::DbConfig;

use crate::error::Fallible;
use crate::types::card_hash::CardHash;
use crate::types::date::Date;
use crate::types::performance::Performance;
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
}

fn probe_schema_exists(tx: &Transaction) -> Fallible<bool> {
    let sql = "select count(*) from sqlite_master where type='table' AND name=?;";
    let count: i64 = tx.query_row(sql, ["cards"], |row| row.get(0))?;
    Ok(count > 0)
}
