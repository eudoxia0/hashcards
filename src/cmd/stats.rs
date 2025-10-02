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

use std::fmt::Display;
use std::fmt::Formatter;

use clap::ValueEnum;
use serde::Serialize;

use crate::deck::Deck;
use crate::error::Fallible;
use crate::types::timestamp::Timestamp;

#[derive(ValueEnum, Clone)]
pub enum StatsFormat {
    /// HTML output.
    Html,
    /// JSON output.
    Json,
}

impl Display for StatsFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            StatsFormat::Html => write!(f, "html"),
            StatsFormat::Json => write!(f, "json"),
        }
    }
}

pub fn print_deck_stats(directory: Option<String>, format: StatsFormat) -> Fallible<()> {
    let stats = get_stats(directory)?;
    // Print.
    match format {
        StatsFormat::Html => {
            eprintln!("HTML output is not implemented yet.");
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
    today_review_count: usize,
}

fn get_stats(directory: Option<String>) -> Fallible<Stats> {
    let deck = Deck::new(directory)?;
    let now = Timestamp::now();
    let stats = Stats {
        cards_in_deck_count: deck.cards.len(),
        cards_in_db_count: deck.db.card_count()?,
        tex_macro_count: deck.macros.len(),
        today_review_count: deck.db.today_review_count(now)?,
    };
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_stats() {
        let stats = get_stats(Some("./test".to_string())).unwrap();
        let Stats {
            cards_in_deck_count,
            cards_in_db_count,
            tex_macro_count,
            today_review_count,
        } = stats;
        assert_eq!(cards_in_deck_count, 2);
        assert_eq!(cards_in_db_count, 2);
        assert_eq!(tex_macro_count, 1);
        assert_eq!(today_review_count, 0);
    }
}
