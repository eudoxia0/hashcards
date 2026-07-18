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
use std::slice::from_ref;

use axum::extract::Path;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use maud::Markup;
use maud::html;

use crate::cmd::browse::render::render_card_revealed;
use crate::cmd::browse::render::render_config;
use crate::cmd::browse::render::render_family_revealed;
use crate::cmd::browse::state::BrowseState;
use crate::cmd::browse::template::error_response;
use crate::cmd::browse::template::internal_error_response;
use crate::cmd::browse::template::ok_response;
use crate::cmd::browse::template::page_template;
use crate::cmd::browse::template::pluralize;
use crate::cmd::browse::url::basic_card_url;
use crate::cmd::browse::url::cloze_family_url;
use crate::error::Fallible;
use crate::error::fail;
use crate::types::card::Card;
use crate::types::card_hash::CardHash;
use crate::types::date::Date;
use crate::types::performance::Performance;

/// A card as written in a deck file: either a basic card, or a family of
/// cloze cards sharing the same text.
enum DeckEntry<'a> {
    Basic(&'a Card),
    ClozeFamily(CardHash, &'a [Card]),
}

pub async fn deck_handler(
    State(state): State<BrowseState>,
    Path(name): Path<String>,
) -> (StatusCode, Html<String>) {
    let mut cards: Vec<&Card> = state
        .cards
        .iter()
        .filter(|card| card.deck_name() == &name)
        .collect();
    if cards.is_empty() {
        return error_response(
            StatusCode::NOT_FOUND,
            &format!("No deck named '{name}' in this collection."),
        );
    }
    // Show cards in the order they appear in their source files.
    cards.sort_by_key(|card| (card.file_path().clone(), card.range().0));
    // Group cloze siblings into one entry per family.
    let mut entries: Vec<DeckEntry> = Vec::new();
    let mut seen_families: HashSet<CardHash> = HashSet::new();
    for card in &cards {
        match card.family_hash() {
            None => entries.push(DeckEntry::Basic(*card)),
            Some(family) => {
                if seen_families.insert(family) {
                    if let Some(siblings) = state.families.get(&family) {
                        entries.push(DeckEntry::ClozeFamily(family, siblings));
                    }
                }
            }
        }
    }
    match render_deck(&state, &name, cards.len(), &entries) {
        Ok(markup) => ok_response(markup),
        Err(e) => internal_error_response(e),
    }
}

fn render_deck(
    state: &BrowseState,
    name: &str,
    card_count: usize,
    entries: &[DeckEntry],
) -> Fallible<Markup> {
    let body = html! {
        nav .breadcrumbs {
            a href="/" { "← Collection" }
        }
        h1 { (name) }
        p .summary {
            @if entries.len() == card_count {
                (pluralize(card_count, "card")) "."
            } @else {
                (pluralize(entries.len(), "card")) " (" (card_count) " drillable)."
            }
        }
        div .card-list {
            @for entry in entries {
                (render_entry(state, entry)?)
            }
        }
    };
    Ok(page_template(&format!("{name} — hashcards"), body))
}

fn render_entry(state: &BrowseState, entry: &DeckEntry) -> Fallible<Markup> {
    let (content, badge, schedule, details_url) = match entry {
        DeckEntry::Basic(card) => {
            let config = render_config(state, card)?;
            let content = render_card_revealed(card, &config)?;
            let schedule = schedule_summary(state, from_ref(*card));
            (
                content,
                "Basic".to_string(),
                schedule,
                basic_card_url(card.hash()),
            )
        }
        DeckEntry::ClozeFamily(family, siblings) => {
            let first = match siblings.first() {
                Some(first) => first,
                None => return fail("cloze family has no cards."),
            };
            let config = render_config(state, first)?;
            let content = render_family_revealed(siblings, &config)?;
            let badge = if siblings.len() == 1 {
                "Cloze".to_string()
            } else {
                format!("Cloze × {}", siblings.len())
            };
            let schedule = schedule_summary(state, siblings);
            (content, badge, schedule, cloze_family_url(*family))
        }
    };
    Ok(html! {
        div .browse-card {
            (content)
            div .card-meta {
                span .badge { (badge) }
                span { (schedule) }
                div .spacer {}
                a href=(details_url) { "Details" }
            }
        }
    })
}

/// Summarize the schedule of a group of cards: "New" if any card has never
/// been reviewed (new cards are due immediately), otherwise the earliest due
/// date.
fn schedule_summary(state: &BrowseState, cards: &[Card]) -> String {
    let mut earliest: Option<Date> = None;
    for card in cards {
        match state.performance_of(card.hash()) {
            Performance::New => return "New".to_string(),
            Performance::Reviewed(rp) => {
                earliest = match earliest {
                    Some(date) => Some(date.min(rp.due_date)),
                    None => Some(rp.due_date),
                };
            }
        }
    }
    match earliest {
        Some(date) => format!("Due {date}"),
        None => "New".to_string(),
    }
}
