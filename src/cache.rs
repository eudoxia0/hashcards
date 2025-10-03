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

use std::collections::HashMap;

use crate::error::Fallible;
use crate::error::fail;
use crate::fsrs::Difficulty;
use crate::fsrs::Stability;
use crate::types::card_hash::CardHash;
use crate::types::date::Date;
use crate::types::performance::Performance;
use crate::types::performance::ReviewedPerformance;
use crate::types::timestamp::Timestamp;

/// An in-memory cache of card performance changes made during the current
/// session. We use this so that updates are only persisted to the database
/// when the session ends. This makes undo simpler to implement, and allows a
/// user to abort a study session without persisting their changes.
pub struct Cache {
    /// A map of card IDs to their performance changes.
    changes: HashMap<CardHash, Performance>,
}

impl Cache {
    /// Creates a new, empty cache.
    pub fn new() -> Self {
        Self {
            changes: HashMap::new(),
        }
    }

    /// Insert's a card performance information. If the hash is already in
    /// the cache, returns an error.
    pub fn insert(&mut self, card_hash: CardHash, performance: Performance) -> Fallible<()> {
        match self.changes.get(&card_hash) {
            Some(_) => fail(format!("Card with hash {card_hash} already in cache")),
            None => {
                self.changes.insert(card_hash, performance);
                Ok(())
            }
        }
    }

    /// Retrieve a card's performance information. If the hash is not in the
    /// cache, returns an error.
    pub fn get(&self, card_hash: CardHash) -> Fallible<Performance> {
        match self.changes.get(&card_hash) {
            Some(performance) => Ok(*performance),
            None => fail(format!("Card with hash {card_hash} not found in cache")),
        }
    }

    /// Update's a card's performance information. If the hash is not in the
    /// cache, returns an error.
    pub fn update(
        &mut self,
        card_hash: CardHash,
        performance: ReviewedPerformance,
    ) -> Fallible<()> {
        match self.changes.get_mut(&card_hash) {
            Some(p) => {
                *p = Performance::Reviewed(performance);
                Ok(())
            }
            None => fail(format!("Card with hash {card_hash} not found in cache")),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&CardHash, &Performance)> {
        self.changes.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::fail;

    #[test]
    fn test_cache_insert_and_get() -> Fallible<()> {
        let mut cache = Cache::new();
        let card_hash = CardHash::hash_bytes(b"a");
        let performance = Performance::New;
        cache.insert(card_hash, performance)?;
        let retrieved = cache.get(card_hash)?;
        match retrieved {
            Performance::New => Ok(()),
            _ => fail("Expected Performance::New"),
        }
    }

    #[test]
    fn test_cache_update() -> Fallible<()> {
        let mut cache = Cache::new();
        let card_hash = CardHash::hash_bytes(b"a");
        let performance = Performance::New;
        cache.insert(card_hash, performance)?;
        let last_reviewed_at = Timestamp::now();
        let stability = 1.0;
        let difficulty = 2.0;
        let interval_raw = 0.4;
        let interval_days = 1;
        let due_date = Timestamp::now().local_date();
        let review_count = 3;
        cache.update(
            card_hash,
            ReviewedPerformance {
                last_reviewed_at,
                stability,
                difficulty,
                interval_raw,
                interval_days,
                due_date,
                review_count,
            },
        )?;
        let retrieved = cache.get(card_hash)?;
        match retrieved {
            Performance::Reviewed(rp) => {
                assert_eq!(rp.last_reviewed_at, last_reviewed_at);
                assert_eq!(rp.stability, stability);
                assert_eq!(rp.difficulty, difficulty);
                assert_eq!(rp.interval_raw, 0.4);
                assert_eq!(rp.interval_days, interval_days);
                assert_eq!(rp.due_date, due_date);
                assert_eq!(rp.review_count, review_count);
                Ok(())
            }
            _ => fail("Expected Performance::Reviewed"),
        }
    }

    #[test]
    fn test_cache_insert_duplicate() -> Fallible<()> {
        let mut cache = Cache::new();
        let card_hash = CardHash::hash_bytes(b"a");
        let performance = Performance::New;
        cache.insert(card_hash, performance)?;
        assert!(cache.insert(card_hash, performance).is_err());
        Ok(())
    }

    #[test]
    fn test_cache_get_nonexistent() -> Fallible<()> {
        let cache = Cache::new();
        let card_hash = CardHash::hash_bytes(b"a");
        assert!(cache.get(card_hash).is_err());
        Ok(())
    }

    #[test]
    fn test_cache_update_nonexistent() -> Fallible<()> {
        let mut cache = Cache::new();
        let card_hash = CardHash::hash_bytes(b"a");
        let last_reviewed_at = Timestamp::now();
        let stability = 1.0;
        let difficulty = 2.0;
        let interval_raw = 0.4;
        let interval_days = 1;
        let due_date = Timestamp::now().local_date();
        let review_count = 3;
        let rp = ReviewedPerformance {
            last_reviewed_at,
            stability,
            difficulty,
            interval_raw,
            interval_days,
            due_date,
            review_count,
        };
        let res = cache.update(card_hash, rp);
        assert!(res.is_err());
        Ok(())
    }

    #[test]
    fn test_cache_iter() -> Fallible<()> {
        let mut cache = Cache::new();
        let card_hash = CardHash::hash_bytes(b"a");
        let performance = Performance::New;
        cache.insert(card_hash, performance)?;
        let mut iter = cache.iter();
        let (key, value) = iter.next().unwrap();
        assert_eq!(*key, card_hash);
        assert!(value.is_new());
        assert!(iter.next().is_none());
        Ok(())
    }
}
