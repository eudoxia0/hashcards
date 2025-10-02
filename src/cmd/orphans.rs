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
use std::path::PathBuf;

use crate::db::Database;
use crate::error::ErrorReport;
use crate::error::Fallible;
use crate::error::fail;
use crate::parser::parse_deck;
use crate::types::hash::Hash;

pub fn list_orphans(directory: &PathBuf) -> Fallible<()> {
    if !directory.exists() {
        return fail("directory does not exist.");
    }
    let db_path = directory.join("db.sqlite3");
    let db = Database::new(
        db_path
            .to_str()
            .ok_or_else(|| ErrorReport::new("invalid path"))?,
    )?;
    let deck = parse_deck(directory)?;
    // Collect hashes.
    let db_hashes: HashSet<Hash> = db.card_hashes()?;
    let deck_hashes: HashSet<Hash> = {
        let mut hashes = HashSet::new();
        for card in deck {
            hashes.insert(card.hash());
        }
        hashes
    };
    // If a card is in the database, but not in the deck, it is an orphan.
    let orphans: Vec<Hash> = db_hashes.difference(&deck_hashes).cloned().collect();
    // Sort the orphans for consistent output.
    // Print.
    for hash in orphans {
        println!("{}", hash);
    }
    Ok(())
}
