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

use crate::deck::Deck;
use crate::error::Fallible;
use crate::types::card_hash::CardHash;

pub fn list_orphans(directory: Option<String>) -> Fallible<()> {
    let deck = Deck::new(directory)?;
    let orphans: Vec<CardHash> = get_orphans(&deck)?;
    // Print.
    for hash in orphans {
        println!("{}", hash);
    }
    Ok(())
}

pub fn delete_orphans(directory: Option<String>) -> Fallible<()> {
    let mut deck = Deck::new(directory)?;
    let orphans: Vec<CardHash> = get_orphans(&deck)?;
    for hash in &orphans {
        deck.db.delete_card(hash)?;
        println!("{}", hash);
    }
    Ok(())
}

fn get_orphans(deck: &Deck) -> Fallible<Vec<CardHash>> {
    // Collect hashes.
    let db_hashes: HashSet<CardHash> = deck.db.card_hashes()?;
    let deck_hashes: HashSet<CardHash> = {
        let mut hashes = HashSet::new();
        for card in deck.cards.iter() {
            hashes.insert(card.hash());
        }
        hashes
    };
    // If a card is in the database, but not in the deck, it is an orphan.
    let mut orphans: Vec<CardHash> = db_hashes.difference(&deck_hashes).cloned().collect();
    // Sort the orphans for consistent output.
    orphans.sort();
    Ok(orphans)
}
