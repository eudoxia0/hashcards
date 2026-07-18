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

use maud::Markup;
use maud::html;
use percent_encoding::NON_ALPHANUMERIC;
use percent_encoding::utf8_percent_encode;

use crate::cmd::browse::shared::BrowseState;
use crate::cmd::browse::templates::EntryKey;
use crate::cmd::browse::templates::deck_entries;
use crate::cmd::browse::templates::entry_key;
use crate::cmd::browse::templates::entry_label_html;
use crate::cmd::browse::templates::entry_url;
use crate::cmd::browse::templates::page_template;
use crate::error::Fallible;
use crate::types::aliases::DeckName;

/// Which deck and card, if any, are currently selected.
pub struct Selection<'a> {
    pub deck: Option<&'a str>,
    pub entry: Option<EntryKey>,
}

/// Render the three-column browse page: the deck list, the card list of the
/// selected deck, and the detail view of the selected card.
pub fn columns_page(
    state: &BrowseState,
    selection: Selection,
    detail: Option<Markup>,
) -> Fallible<Markup> {
    let title = match selection.deck {
        Some(deck) => format!("{deck} — hashcards"),
        None => "hashcards".to_string(),
    };
    let cards_pane_body = match selection.deck {
        Some(deck) => cards_pane(state, deck, selection.entry)?,
        None => placeholder("Select a deck."),
    };
    let detail_pane_body = match detail {
        Some(markup) => markup,
        None if selection.deck.is_some() => placeholder("Select a card."),
        None => placeholder(""),
    };
    let body = html! {
        main .browse-columns {
            div .pane .deck-pane {
                (deck_pane(state, selection.deck))
            }
            div .pane .cards-pane {
                (cards_pane_body)
            }
            div .pane .detail-pane {
                (detail_pane_body)
            }
        }
    };
    Ok(page_template(&title, body))
}

/// The deck list, grouped by starting letter like a dictionary.
fn deck_pane(state: &BrowseState, selected: Option<&str>) -> Markup {
    // Count due cards per deck. The `entry` call ensures every deck is
    // present, even with zero due cards.
    let mut due_counts: BTreeMap<&DeckName, usize> = BTreeMap::new();
    for card in state.cards.iter() {
        let count = due_counts.entry(card.deck_name()).or_default();
        if state.is_due(card.hash()) {
            *count += 1;
        }
    }
    let total_cards = state.cards.len();
    let total_due: usize = due_counts.values().sum();
    // Case-insensitive alphabetical order.
    let mut decks: Vec<(&DeckName, usize)> = due_counts.into_iter().collect();
    decks.sort_by_key(|(name, _)| name.to_lowercase());
    // Group by starting letter. Within a group, decks stay in sorted order.
    let mut groups: BTreeMap<char, Vec<(&DeckName, usize)>> = BTreeMap::new();
    for (name, due) in decks {
        groups
            .entry(first_letter(name))
            .or_default()
            .push((name, due));
    }
    html! {
        div .pane-header {
            div .pane-title { "Decks" }
            div .pane-sub { (pluralize(total_cards, "card")) ", " (total_due) " due" }
        }
        @if groups.is_empty() {
            div .pane-empty { "No cards in this collection." }
        }
        @for (letter, decks) in &groups {
            div .letter-group {
                div .letter { (letter) }
                ul {
                    @for (name, due) in decks {
                        li {
                            a .selected[selected == Some(name.as_str())] href=(deck_url(name)) {
                                span .name { (name) }
                                @if *due > 0 {
                                    span .due-badge { (due) }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The list of cards in a deck.
fn cards_pane(state: &BrowseState, deck: &str, selected: Option<EntryKey>) -> Fallible<Markup> {
    let entries = deck_entries(state, deck);
    let due_count = state
        .cards
        .iter()
        .filter(|card| card.deck_name() == deck && state.is_due(card.hash()))
        .count();
    Ok(html! {
        div .pane-header {
            div .pane-title { (deck) }
            div .pane-sub { (pluralize(entries.len(), "card")) ", " (due_count) " due" }
        }
        ul .card-items {
            @for entry in &entries {
                li {
                    a .selected[selected == Some(entry_key(entry))] href=(entry_url(entry)) {
                        div .label { (entry_label_html(state, entry)?) }
                    }
                }
            }
        }
    })
}

fn placeholder(message: &str) -> Markup {
    html! {
        div .placeholder {
            p { (message) }
        }
    }
}

/// The letter a deck is grouped under: the uppercased first letter of its
/// name, or '#' for names not starting with a letter.
fn first_letter(name: &str) -> char {
    match name.chars().next() {
        Some(c) if c.is_alphabetic() => c.to_uppercase().next().unwrap_or(c),
        _ => '#',
    }
}

/// Generate the URL of a deck from its name.
fn deck_url(name: &DeckName) -> String {
    format!("/deck/{}", utf8_percent_encode(name, NON_ALPHANUMERIC))
}

/// A count followed by the singular or plural form of a word, e.g. "1 card",
/// "3 cards".
fn pluralize(n: usize, word: &str) -> String {
    if n == 1 {
        format!("{n} {word}")
    } else {
        format!("{n} {word}s")
    }
}
