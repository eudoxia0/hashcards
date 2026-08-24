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

use std::collections::HashMap;
use std::collections::HashSet;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use axum::response::IntoResponse;
use maud::Markup;
use maud::html;
use percent_encoding::NON_ALPHANUMERIC;
use percent_encoding::utf8_percent_encode;

use crate::cmd::browse::server::BrowseState;
use crate::cmd::browse::templates::page_template;
use crate::error::Fallible;
use crate::types::card::Card;
use crate::types::card_hash::CardHash;
use crate::types::date::Date;

struct DeckStats {
    deck_name: String,
    total: usize,
    due: usize,
    new: usize,
}

pub async fn index_handler(State(state): State<BrowseState>) -> impl IntoResponse {
    match index_view(state) {
        Ok(html) => (StatusCode::OK, Html(html.into_string())),
        Err(e) => {
            let html = page_template(
                "error - hashcards",
                None,
                html! {
                    div.error {
                        h1 { "Error" }
                        p { (e) }
                    }
                },
            );
            (StatusCode::INTERNAL_SERVER_ERROR, Html(html.into_string()))
        }
    }
}

fn index_view(state: BrowseState) -> Fallible<Markup> {
    let today = Date::today();
    // Load due dates for all cards in the database.
    let due_dates: HashMap<CardHash, Option<Date>> = {
        let db = state.db.lock().unwrap();
        db.card_due_dates()?
    };
    // Compute stats for each deck.
    let deck_stats: Vec<DeckStats> = compute_deck_stats(&state.cards, due_dates, today);
    // Group deck stats by the initial character of each deck.
    let groups: Vec<(char, Vec<DeckStats>)> = group_deck_stats(deck_stats);
    // Compute totals.
    let total_cards: usize = groups
        .iter()
        .flat_map(|(_, g)| g.iter())
        .map(|d| d.total)
        .sum();
    let total_due: usize = groups
        .iter()
        .flat_map(|(_, g)| g.iter())
        .map(|d| d.due)
        .sum();
    let total_new: usize = groups
        .iter()
        .flat_map(|(_, g)| g.iter())
        .map(|d| d.new)
        .sum();
    let body = render(groups, total_cards, total_due, total_new);
    Ok(page_template("hashcards", Some("/home.css"), body))
}

fn compute_deck_stats(
    cards: &[Card],
    due_dates: HashMap<CardHash, Option<Date>>,
    today: Date,
) -> Vec<DeckStats> {
    // Build a map from the name of a deck to the set of card hashes in that deck.
    let mut deck_hashes: HashMap<String, Vec<CardHash>> = HashMap::new();
    for card in cards.iter() {
        deck_hashes
            .entry(card.deck_name().clone())
            .or_default()
            .push(card.hash());
    }

    // Build a map from deck names to the number of user-visible cards in that
    // deck. Each basic card counts as one user-visible card. All the cloze
    // cards of the same family count as one user-visible card.
    let mut deck_visible: HashMap<String, HashSet<CardHash>> = HashMap::new();
    for card in cards.iter() {
        let hash: CardHash = match card.family_hash() {
            Some(hash) => hash,
            None => card.hash(),
        };
        deck_visible
            .entry(card.deck_name().clone())
            .or_default()
            .insert(hash);
    }

    let mut output: Vec<DeckStats> = Vec::new();
    for (deck_name, hashes) in deck_hashes.into_iter() {
        let total: usize = deck_visible.get(&deck_name).map_or(0, HashSet::len);
        let mut due: usize = 0;
        let mut new: usize = 0;
        for h in &hashes {
            match due_dates.get(h) {
                // Not in db: never been seen, treat as new.
                None => new += 1,
                // In db but never reviewed.
                Some(None) => new += 1,
                // Reviewed and due today or earlier.
                Some(Some(d)) if *d <= today => due += 1,
                // Reviewed and not yet due.
                Some(Some(_)) => {}
            }
        }
        let deck_stats = DeckStats {
            deck_name,
            total,
            due,
            new,
        };
        output.push(deck_stats);
    }
    // Sort alphabetically, case-insensitive.
    output.sort_by_key(|a| a.deck_name.to_lowercase());
    output
}

fn group_deck_stats(deck_stats: Vec<DeckStats>) -> Vec<(char, Vec<DeckStats>)> {
    let mut groups: Vec<(char, Vec<DeckStats>)> = Vec::new();
    for stats in deck_stats.into_iter() {
        let letter = stats
            .deck_name
            .chars()
            .next()
            .map(|c| {
                if c.is_ascii_alphabetic() {
                    c.to_ascii_uppercase()
                } else {
                    '#'
                }
            })
            .unwrap_or('#');
        match groups.last_mut() {
            Some(last) if last.0 == letter => last.1.push(stats),
            _ => groups.push((letter, vec![stats])),
        }
    }
    groups
}

fn render(
    groups: Vec<(char, Vec<DeckStats>)>,
    total_cards: usize,
    total_due: usize,
    total_new: usize,
) -> Markup {
    html! {
        nav .breadcrumbs {
            span .crumb-current { "Home" }
        }
        main .deck-list-page {
            table .deck-list {
                thead {
                    tr {
                        th {
                            "Deck"
                        }
                        th {
                            "Cards"
                        }
                        th {
                            "Due"
                        }
                        th {
                            "New"
                        }
                    }
                }
                tbody {
                    @for (letter, group_decks) in &groups {
                        tr .group-row {
                            td colspan="4" {
                                (letter)
                            }
                        }
                        @for deck in group_decks {
                            tr .deck-row {
                                td .deck-cell {
                                    a href=(deck_url(&deck.deck_name)) {
                                        (deck.deck_name)
                                    }
                                }
                                td .stat-cell {
                                    (deck.total)
                                }
                                td .stat-cell .zero[deck.due == 0] {
                                    (deck.due)
                                }
                                td .stat-cell .zero[deck.new == 0] {
                                    (deck.new)
                                }
                            }
                        }
                    }
                }
                tfoot {
                    tr {
                        td .total-cell {
                            "Total"
                        }
                        td .stat-cell {
                            (total_cards)
                        }
                        td .stat-cell {
                            (total_due)
                        }
                        td .stat-cell {
                            (total_new)
                        }
                    }
                }
            }
        }
    }
}

fn deck_url(deck_name: &str) -> String {
    format!("/deck/{}", utf8_percent_encode(deck_name, NON_ALPHANUMERIC))
}
