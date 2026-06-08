// Copyright 2025–2026 Fernando Borretti
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

use std::collections::BTreeMap;

use crate::collection::Collection;
use crate::error::Fallible;
use crate::types::date::Date;

pub fn print_due(directory: Option<String>) -> Fallible<()> {
    let coll = Collection::new(directory)?;
    let today = Date::today();
    let due_hashes = coll.db.due_today(today)?;
    // Count due cards per deck.
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for card in &coll.cards {
        if due_hashes.contains(&card.hash()) {
            *counts.entry(card.deck_name().clone()).or_default() += 1;
        }
    }
    // Print decks with non-zero due cards in alphabetical order.
    let mut total: usize = 0;
    for (deck_name, count) in &counts {
        println!("{}: {}", deck_name, count);
        total += count;
    }
    println!("Total: {}", total);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helper::create_tmp_copy_of_test_directory;

    #[test]
    fn test_print_due() -> Fallible<()> {
        let directory = create_tmp_copy_of_test_directory()?;
        print_due(Some(directory))?;
        Ok(())
    }
}
