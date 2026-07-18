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

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use maud::Markup;
use maud::html;

use crate::cmd::browse::state::BrowseState;
use crate::cmd::browse::template::ok_response;
use crate::cmd::browse::template::page_template;
use crate::cmd::browse::template::pluralize;
use crate::cmd::browse::url::deck_url;
use crate::types::aliases::DeckName;

/// Per-deck card counts shown on the index page.
#[derive(Default)]
struct DeckStats {
    total: usize,
    new: usize,
    due: usize,
}

pub async fn index_handler(State(state): State<BrowseState>) -> (StatusCode, Html<String>) {
    ok_response(render_index(&state))
}

fn render_index(state: &BrowseState) -> Markup {
    let mut decks: BTreeMap<&DeckName, DeckStats> = BTreeMap::new();
    for card in state.cards.iter() {
        let stats = decks.entry(card.deck_name()).or_default();
        stats.total += 1;
        if state.performance_of(card.hash()).is_new() {
            stats.new += 1;
        }
        if state.is_due(card.hash()) {
            stats.due += 1;
        }
    }
    let total_cards: usize = state.cards.len();
    let total_due: usize = decks.values().map(|stats| stats.due).sum();
    let body = html! {
        h1 { "Collection" }
        @if decks.is_empty() {
            p .empty { "No cards in this collection." }
        } @else {
            p .summary {
                (pluralize(total_cards, "card")) " in " (pluralize(decks.len(), "deck")) ". "
                (total_due) " due today."
            }
            table .deck-table {
                thead {
                    tr {
                        th { "Deck" }
                        th .num { "Cards" }
                        th .num { "New" }
                        th .num { "Due" }
                    }
                }
                tbody {
                    @for (name, stats) in &decks {
                        tr {
                            td { a href=(deck_url(name)) { (name) } }
                            td .num { (stats.total) }
                            td .num { (stats.new) }
                            td .num { (stats.due) }
                        }
                    }
                }
            }
        }
    };
    page_template("hashcards", body)
}
