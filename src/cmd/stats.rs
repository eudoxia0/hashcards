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

use std::path::PathBuf;

use serde::Serialize;

use crate::cli::StatsFormat;
use crate::db::Database;
use crate::error::ErrorReport;
use crate::error::Fallible;
use crate::error::fail;
use crate::parser::parse_deck;

pub fn print_deck_stats(directory: &PathBuf, format: StatsFormat) -> Fallible<()> {
    // Load everything.
    if !directory.exists() {
        return fail("directory does not exist.");
    }
    let deck = parse_deck(directory)?;

    let db_path = directory.join("db.sqlite3");
    let db = Database::new(
        db_path
            .to_str()
            .ok_or_else(|| ErrorReport::new("invalid path"))?,
    )?;

    let mut macros = Vec::new();
    let macros_path = directory.join("macros.tex");
    if macros_path.exists() {
        let content = std::fs::read_to_string(macros_path)?;
        for line in content.lines() {
            if let Some((name, definition)) = line.split_once(' ') {
                macros.push((name.to_string(), definition.to_string()));
            }
        }
    }

    // Construct stats.
    let stats = Stats {
        cards_in_deck_count: deck.len(),
        cards_in_db_count: db.card_count()?,
        tex_macro_count: macros.len(),
    };

    match format {
        StatsFormat::Html => {
            todo!()
        }
        StatsFormat::Json => {
            let stats_json = serde_json::to_string_pretty(&stats)?;
            println!("{}", stats_json);
        }
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    cards_in_deck_count: usize,
    cards_in_db_count: usize,
    tex_macro_count: usize,
}
