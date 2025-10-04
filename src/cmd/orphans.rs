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

use crate::collection::Collection;
use crate::error::Fallible;
use crate::types::card_hash::CardHash;

pub fn list_orphans(directory: Option<String>) -> Fallible<()> {
    let coll = Collection::new(directory)?;
    let orphans: Vec<CardHash> = get_orphans(&coll)?;
    // Print.
    for hash in orphans {
        println!("{}", hash);
    }
    Ok(())
}

pub fn delete_orphans(directory: Option<String>) -> Fallible<()> {
    let coll = Collection::new(directory)?;
    let orphans: Vec<CardHash> = get_orphans(&coll)?;
    for hash in &orphans {
        coll.db.delete_card(*hash)?;
        println!("{}", hash);
    }
    Ok(())
}

fn get_orphans(coll: &Collection) -> Fallible<Vec<CardHash>> {
    // Collect hashes.
    let db_hashes: HashSet<CardHash> = coll.db.card_hashes()?;
    let coll_hashes: HashSet<CardHash> = {
        let mut hashes = HashSet::new();
        for card in coll.cards.iter() {
            hashes.insert(card.hash());
        }
        hashes
    };
    // If a card is in the database, but not in the deck, it is an orphan.
    let mut orphans: Vec<CardHash> = db_hashes.difference(&coll_hashes).cloned().collect();
    // Sort the orphans for consistent output.
    orphans.sort();
    Ok(orphans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helper::create_tmp_copy_of_test_directory;
    use crate::types::timestamp::Timestamp;

    #[test]
    fn test_get_orphans() -> Fallible<()> {
        let dir: String = create_tmp_copy_of_test_directory()?;
        let coll = Collection::new(Some(dir))?;
        let hash = CardHash::hash_bytes(b"a");
        let now = Timestamp::now();
        coll.db.insert_card(hash, now)?;
        let orphans = get_orphans(&coll)?;
        assert_eq!(orphans, vec![hash]);
        Ok(())
    }

    #[test]
    fn test_list_and_delete_orphans() -> Fallible<()> {
        let dir: String = create_tmp_copy_of_test_directory()?;
        let coll = Collection::new(Some(dir.clone()))?;
        let hash = CardHash::hash_bytes(b"a");
        let now = Timestamp::now();
        coll.db.insert_card(hash, now)?;
        list_orphans(Some(dir.clone()))?;
        delete_orphans(Some(dir.clone()))?;
        assert!(coll.db.card_hashes()?.is_empty());
        Ok(())
    }
}
