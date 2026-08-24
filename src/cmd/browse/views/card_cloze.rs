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

use axum::extract::Path;
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
use crate::cmd::browse::views::card_basic::render_history;
use crate::cmd::browse::views::card_basic::render_performance_rows;
use crate::db::ReviewRow;
use crate::error::Fallible;
use crate::error::fail;
use crate::markdown::MarkdownRenderConfig;
use crate::media::resolve::MediaResolverBuilder;
use crate::types::card::Card;
use crate::types::card_hash::CardHash;
use crate::types::performance::Performance;

pub async fn card_cloze_handler(
    State(state): State<BrowseState>,
    Path(family_hash): Path<String>,
) -> impl IntoResponse {
    match card_cloze_view(state, family_hash) {
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

fn card_cloze_view(state: BrowseState, family_hash: String) -> Fallible<Markup> {
    let family_hash = CardHash::from_hex(&family_hash)?;
    let clozes: Vec<&Card> = state
        .cards
        .iter()
        .filter(|card| card.family_hash() == Some(family_hash))
        .collect();
    let Some(first) = clozes.first() else {
        return fail(format!("No cloze family found with hash {family_hash}"));
    };
    let mut sections: Vec<(usize, Option<Performance>, Vec<ReviewRow>)> = Vec::new();
    for (i, card) in clozes.iter().enumerate() {
        let performance = {
            let db = state.db.lock().unwrap();
            db.get_card_performance_opt(card.hash())?
        };
        let reviews = {
            let db = state.db.lock().unwrap();
            db.get_reviews_for_card(card.hash())?
        };
        sections.push((i + 1, performance, reviews));
    }
    let body = render(&state, first, &clozes, family_hash, sections)?;
    Ok(page_template(
        &format!("{} - hashcards", first.deck_name()),
        Some("/card.css"),
        body,
    ))
}

fn render(
    state: &BrowseState,
    first: &Card,
    clozes: &[&Card],
    family_hash: CardHash,
    sections: Vec<(usize, Option<Performance>, Vec<ReviewRow>)>,
) -> Fallible<Markup> {
    let deck_path = first.relative_file_path(&state.directory)?;
    let config = MarkdownRenderConfig {
        resolver: MediaResolverBuilder::new()
            .with_collection_path(state.directory.clone())?
            .with_deck_path(deck_path)?
            .build()?,
        resource_hostname: state.resource_hostname.clone(),
        port: state.port,
    };
    let back: Markup = Card::html_back_family(clozes, &config)?;
    let body = html! {
        nav .breadcrumbs {
            a href="/" { "Home" }
            span .crumb-sep { "»" }
            a href=(deck_url(first.deck_name())) { (first.deck_name()) }
            span .crumb-sep { "»" }
            span .crumb-current { "Card" }
        }
        main .card-page {
            div .card {
                div .cloze {
                    .rich-text {
                        (back)
                    }
                }
            }
            h1 { "Properties" }
            table .properties {
                tbody {
                    tr {
                        th { "Type" }
                        td { "Cloze" }
                    }
                    tr {
                        th { "Family Hash" }
                        td .hash-cell { (family_hash.to_hex()) }
                    }
                    tr {
                        th { "Deck" }
                        td { (first.deck_name()) }
                    }
                    tr {
                        th { "Cloze Count" }
                        td { (sections.len()) }
                    }
                }
            }
            @for (i, performance, reviews) in &sections {
                section .cloze-section {
                    h1 { "Cloze " (i) }
                    table .properties {
                        tbody {
                            (render_performance_rows(*performance))
                        }
                    }
                    (render_history("Review History", reviews))
                }
            }
        }
    };
    Ok(body)
}

fn deck_url(deck_name: &str) -> String {
    format!("/deck/{}", utf8_percent_encode(deck_name, NON_ALPHANUMERIC))
}
