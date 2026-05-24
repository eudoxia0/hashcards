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

use std::collections::HashSet;

use crate::cmd::drill::state::Review;
use crate::fsrs::Grade;
use crate::types::card_hash::CardHash;

/// Percentage of unique cards whose first review of the session was not
/// `Forgot`. Re-reviews of failed (or hard) cards later in the session are
/// ignored. Returns `0.0` when there are no reviews.
pub fn retention_rate(reviews: &[Review]) -> f64 {
    let mut seen: HashSet<CardHash> = HashSet::new();
    let mut total: usize = 0;
    let mut remembered: usize = 0;
    for review in reviews {
        if seen.insert(review.card.hash()) {
            total += 1;
            if review.grade != Grade::Forgot {
                remembered += 1;
            }
        }
    }
    if total == 0 {
        0.0
    } else {
        (remembered as f64 / total as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::types::card::Card;
    use crate::types::card::CardContent;
    use crate::types::date::Date;
    use crate::types::timestamp::Timestamp;

    fn make_card(question: &str) -> Card {
        Card::new(
            "deck".to_string(),
            PathBuf::from("deck.md"),
            (0, 0),
            CardContent::new_basic(question, "answer"),
        )
    }

    fn make_review(card: Card, grade: Grade) -> Review {
        Review {
            card,
            reviewed_at: Timestamp::now(),
            grade,
            stability: 1.0,
            difficulty: 1.0,
            interval_raw: 0.0,
            interval_days: 0,
            due_date: Date::today(),
        }
    }

    #[test]
    fn test_retention_rate_empty() {
        assert_eq!(retention_rate(&[]), 0.0);
    }

    #[test]
    fn test_retention_rate_all_remembered() {
        let reviews = vec![
            make_review(make_card("a"), Grade::Good),
            make_review(make_card("b"), Grade::Easy),
            make_review(make_card("c"), Grade::Hard),
        ];
        assert_eq!(retention_rate(&reviews), 100.0);
    }

    #[test]
    fn test_retention_rate_all_forgot() {
        let reviews = vec![
            make_review(make_card("a"), Grade::Forgot),
            make_review(make_card("b"), Grade::Forgot),
        ];
        assert_eq!(retention_rate(&reviews), 0.0);
    }

    #[test]
    fn test_retention_rate_mixed() {
        // 1 forgot, 3 remembered out of 4 unique cards = 75%.
        let reviews = vec![
            make_review(make_card("a"), Grade::Good),
            make_review(make_card("b"), Grade::Forgot),
            make_review(make_card("c"), Grade::Easy),
            make_review(make_card("d"), Grade::Hard),
        ];
        assert_eq!(retention_rate(&reviews), 75.0);
    }

    #[test]
    fn test_retention_rate_ignores_re_reviews() {
        // Card "a" was forgot then remembered on re-review; only the first
        // attempt should count, so retention is 0%.
        let card_a = make_card("a");
        let reviews = vec![
            make_review(card_a.clone(), Grade::Forgot),
            make_review(card_a, Grade::Good),
        ];
        assert_eq!(retention_rate(&reviews), 0.0);
    }

    #[test]
    fn test_retention_rate_ignores_re_reviews_after_hard() {
        // Card "a" was marked Hard (remembered, but repeated) then Good. The
        // first attempt counts as remembered, giving 100%.
        let card_a = make_card("a");
        let card_b = make_card("b");
        let reviews = vec![
            make_review(card_a.clone(), Grade::Hard),
            make_review(card_b, Grade::Good),
            make_review(card_a, Grade::Good),
        ];
        assert_eq!(retention_rate(&reviews), 100.0);
    }
}
