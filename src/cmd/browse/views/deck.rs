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

use axum::extract::Path;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use axum::response::IntoResponse;
use maud::Markup;
use maud::html;

use crate::cmd::browse::server::BrowseState;
use crate::cmd::browse::templates::page_template;
use crate::error::Fallible;
use crate::markdown::MarkdownRenderConfig;
use crate::media::resolve::MediaResolverBuilder;
use crate::types::card::Card;
use crate::types::card::CardType;
use crate::types::card_hash::CardHash;

pub async fn deck_handler(
    State(state): State<BrowseState>,
    Path(deck_name): Path<String>,
) -> impl IntoResponse {
    match deck_view(state, deck_name) {
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

fn deck_view(state: BrowseState, deck_name: String) -> Fallible<Markup> {
    let deck_cards: Vec<&Card> = state
        .cards
        .iter()
        .filter(|card| *card.deck_name() == deck_name)
        .collect();
    let visible_cards: Vec<&Card> = visible_cards(deck_cards);
    let body = render(&state, &deck_name, visible_cards)?;
    Ok(page_template(
        &format!("{deck_name} - hashcards"),
        Some("/deck.css"),
        body,
    ))
}

/// Reduce a deck's cards to the ones that should be shown to the user: one
/// entry per basic card, and one entry per cloze family.
fn visible_cards(cards: Vec<&Card>) -> Vec<&Card> {
    let mut seen: HashSet<CardHash> = HashSet::new();
    let mut visible: Vec<&Card> = Vec::new();
    for card in cards {
        let visible_hash = card.family_hash().unwrap_or_else(|| card.hash());
        if seen.insert(visible_hash) {
            visible.push(card);
        }
    }
    visible.sort_by_key(|card| card.family_hash().unwrap_or_else(|| card.hash()));
    visible
}

fn render(state: &BrowseState, deck_name: &str, cards: Vec<&Card>) -> Fallible<Markup> {
    let mut rows: Vec<Markup> = Vec::new();
    for card in cards {
        rows.push(render_card_row(state, card)?);
    }
    let body = html! {
        nav .breadcrumbs {
            a href="/" { "Home" }
            span .crumb-sep { "»" }
            span .crumb-current { (deck_name) }
        }
        main .deck-page {
            h1 {
                (deck_name)
            }
            ul .card-list {
                @for row in &rows {
                    (row)
                }
            }
        }
    };
    Ok(body)
}

fn render_card_row(state: &BrowseState, card: &Card) -> Fallible<Markup> {
    let deck_path = card.relative_file_path(&state.directory)?;
    let config = MarkdownRenderConfig {
        resolver: MediaResolverBuilder::new()
            .with_collection_path(state.directory.clone())?
            .with_deck_path(deck_path)?
            .build()?,
        resource_hostname: state.resource_hostname.clone(),
        port: state.port,
    };
    let html: Markup = match card.card_type() {
        CardType::Basic => {
            let front: Markup = card.html_front(&config)?;
            let back: Markup = card.html_back(&config)?;
            html! {
                div .card {
                    div .front {
                        .rich-text {
                            (front)
                        }
                    }
                    div .back {
                        .rich-text {
                            (back)
                        }
                    }
                }
            }
        }
        CardType::Cloze => {
            let back: Markup = card.html_back(&config)?;
            html! {
                div .card {
                    div .cloze {
                        .rich-text {
                            (back)
                        }
                    }
                }
            }
        }
    };
    let html: Markup = html! {
        li {
            a .card-content href=(card_url(card)) {
                (html)
            }
        }
    };
    Ok(html)
}

fn card_url(card: &Card) -> String {
    match card.card_type() {
        CardType::Basic => format!("/card/basic/{}", card.hash()),
        CardType::Cloze => format!(
            "/card/cloze/{}",
            card.family_hash().unwrap_or_else(|| card.hash())
        ),
    }
}
