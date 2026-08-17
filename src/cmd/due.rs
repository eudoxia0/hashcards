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
use crate::error::ErrorReport;
use crate::error::Fallible;
use crate::types::date::Date;

/// Parse a date argument: "today", "tomorrow", or "YYYY-MM-DD".
pub fn parse_date_arg(s: &str) -> Fallible<Date> {
    match s {
        "today" => Ok(Date::today()),
        "tomorrow" => Ok(Date::tomorrow()),
        _ => Date::try_from(s.to_string()).map_err(|_| {
            ErrorReport::new(format!(
                "invalid date '{}'. Valid values are `today`, `tomorrow`, or a date in `YYYY-MM-DD` format.",
                s
            ))
        }),
    }
}

/// Compute the number of cards due by `date` in each deck, and the total.
///
/// For today, this matches what `drill` would pull into a session: overdue,
/// never-reviewed, and due-today cards. For any other date, it's an exact
/// match on that date's due cards only.
fn due_report(directory: Option<String>, date: Date) -> Fallible<(BTreeMap<String, usize>, usize)> {
    let coll = Collection::new(directory)?;
    let due_hashes = if date == Date::today() {
        coll.db.all_due(date)?
    } else {
        coll.db.due_on(date)?
    };
    // Count due cards per deck.
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for card in &coll.cards {
        if due_hashes.contains(&card.hash()) {
            *counts.entry(card.deck_name().clone()).or_default() += 1;
        }
    }
    let total: usize = counts.values().sum();
    Ok((counts, total))
}

/// Print the number of cards due on `date` in each deck, and the total.
pub fn print_due(directory: Option<String>, date: Date) -> Fallible<()> {
    let (counts, total) = due_report(directory, date)?;
    // Print decks with non-zero due cards in alphabetical order.
    for (deck_name, count) in &counts {
        println!("{}: {}", deck_name, count);
    }
    println!("Total: {}", total);
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::Days;

    use super::*;
    use crate::helper::create_tmp_copy_of_test_directory;
    use crate::types::performance::Performance;
    use crate::types::performance::ReviewedPerformance;
    use crate::types::timestamp::Timestamp;

    #[test]
    fn test_print_due() -> Fallible<()> {
        let directory = create_tmp_copy_of_test_directory()?;
        print_due(Some(directory), Date::today())?;
        Ok(())
    }

    /// Regression test: a never-reviewed card (due_date is `NULL`) and an
    /// overdue card (due_date before `date`) must both count as due. Using
    /// `due_on`'s exact-date match instead of `all_due`'s cumulative check
    /// would report 0 for both, understating what a drill session would
    /// actually pull in.
    #[test]
    fn test_due_report_counts_never_reviewed_and_overdue_cards() -> Fallible<()> {
        let directory = create_tmp_copy_of_test_directory()?;
        let coll = Collection::new(Some(directory.clone()))?;
        assert!(
            coll.cards.len() >= 2,
            "fixture directory must contain at least two cards"
        );
        let now = Timestamp::now();
        for card in &coll.cards {
            coll.db.insert_card(card.hash(), now)?;
        }
        // Leave the first card never-reviewed (due_date stays NULL). Mark
        // the second as overdue by a day.
        let yesterday = Date::new(
            Date::today()
                .into_inner()
                .checked_sub_days(Days::new(1))
                .unwrap(),
        );
        coll.db.update_card_performance(
            coll.cards[1].hash(),
            Performance::Reviewed(ReviewedPerformance {
                last_reviewed_at: now,
                stability: 1.0,
                difficulty: 1.0,
                interval_raw: 1.0,
                interval_days: 1,
                due_date: yesterday,
                review_count: 1,
            }),
        )?;
        drop(coll);

        let (_, total) = due_report(Some(directory), Date::today())?;
        assert_eq!(total, 2);
        Ok(())
    }

    /// Regression test: querying a specific future date must be an exact
    /// match on that date, and must NOT pull in never-reviewed or overdue
    /// cards the way `today` does.
    #[test]
    fn test_due_report_future_date_is_exact_match() -> Fallible<()> {
        let directory = create_tmp_copy_of_test_directory()?;
        let coll = Collection::new(Some(directory.clone()))?;
        assert!(
            coll.cards.len() >= 2,
            "fixture directory must contain at least two cards"
        );
        let now = Timestamp::now();
        for card in &coll.cards {
            coll.db.insert_card(card.hash(), now)?;
        }
        // Leave the first card never-reviewed. Mark the second as overdue.
        let yesterday = Date::new(
            Date::today()
                .into_inner()
                .checked_sub_days(Days::new(1))
                .unwrap(),
        );
        coll.db.update_card_performance(
            coll.cards[1].hash(),
            Performance::Reviewed(ReviewedPerformance {
                last_reviewed_at: now,
                stability: 1.0,
                difficulty: 1.0,
                interval_raw: 1.0,
                interval_days: 1,
                due_date: yesterday,
                review_count: 1,
            }),
        )?;
        drop(coll);

        // Neither the never-reviewed nor the overdue card is due "tomorrow",
        // so querying that date should report 0.
        let (_, total) = due_report(Some(directory), Date::tomorrow())?;
        assert_eq!(total, 0);
        Ok(())
    }

    #[test]
    fn test_parse_date_arg() -> Fallible<()> {
        assert!(parse_date_arg("today").is_ok());
        assert!(parse_date_arg("tomorrow").is_ok());
        assert!(parse_date_arg("2026-01-15").is_ok());
        assert!(parse_date_arg("not-a-date").is_err());
        Ok(())
    }
}
