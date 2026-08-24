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
use crate::db::ReviewRow;
use crate::error::Fallible;
use crate::error::fail;
use crate::fsrs::Grade;
use crate::markdown::MarkdownRenderConfig;
use crate::media::resolve::MediaResolverBuilder;
use crate::types::card::Card;
use crate::types::card_hash::CardHash;
use crate::types::performance::Performance;

pub async fn card_basic_handler(
    State(state): State<BrowseState>,
    Path(card_hash): Path<String>,
) -> impl IntoResponse {
    match card_basic_view(state, card_hash) {
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

fn card_basic_view(state: BrowseState, card_hash: String) -> Fallible<Markup> {
    let card_hash = CardHash::from_hex(&card_hash)?;
    let card = match state.cards.iter().find(|card| card.hash() == card_hash) {
        Some(card) => card,
        None => return fail(format!("No card found with hash {card_hash}")),
    };
    let performance = {
        let db = state.db.lock().unwrap();
        db.get_card_performance_opt(card_hash)?
    };
    let reviews = {
        let db = state.db.lock().unwrap();
        db.get_reviews_for_card(card_hash)?
    };
    let body = render(&state, card, performance, reviews)?;
    Ok(page_template(
        &format!("{} - hashcards", card.deck_name()),
        Some("/card.css"),
        body,
    ))
}

fn render(
    state: &BrowseState,
    card: &Card,
    performance: Option<Performance>,
    reviews: Vec<ReviewRow>,
) -> Fallible<Markup> {
    let deck_path = card.relative_file_path(&state.directory)?;
    let config = MarkdownRenderConfig {
        resolver: MediaResolverBuilder::new()
            .with_collection_path(state.directory.clone())?
            .with_deck_path(deck_path)?
            .build()?,
        resource_hostname: state.resource_hostname.clone(),
        port: state.port,
    };
    let front: Markup = card.html_front(&config)?;
    let back: Markup = card.html_back(&config)?;
    let body = html! {
        nav .breadcrumbs {
            a href="/" { "Home" }
            span .crumb-sep { "»" }
            a href=(deck_url(card.deck_name())) { (card.deck_name()) }
            span .crumb-sep { "»" }
            span .crumb-current { "Card" }
        }
        main .card-page {
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
            (render_properties(card, "Basic", performance))
            (render_history("Review History", &reviews))
        }
    };
    Ok(body)
}

/// Render a table of card properties, shared with the cloze view.
pub fn render_properties(card: &Card, card_type: &str, performance: Option<Performance>) -> Markup {
    html! {
        h1 { "Properties" }
        table .properties {
            tbody {
                tr {
                    th { "Card Type" }
                    td { (card_type) }
                }
                tr {
                    th { "Hash" }
                    td .hash-cell { (card.hash().to_hex()) }
                }
                tr {
                    th { "Deck" }
                    td { (card.deck_name()) }
                }
                (render_performance_rows(performance))
            }
        }
    }
}

/// Render the FSRS-derived rows of a properties table.
pub fn render_performance_rows(performance: Option<Performance>) -> Markup {
    match performance {
        None | Some(Performance::New) => html! {
            tr {
                th { "Status" }
                td { "New — not yet reviewed" }
            }
        },
        Some(Performance::Reviewed(p)) => html! {
            tr {
                th { "Status" }
                td { "Reviewed" }
            }
            tr {
                th { "Due date" }
                td { (p.due_date) }
            }
            tr {
                th { "Stability" }
                td { (format!("{:.2}", p.stability)) }
            }
            tr {
                th { "Difficulty" }
                td { (format!("{:.2}", p.difficulty)) }
            }
            tr {
                th { "Interval" }
                td { (format!("{} days", p.interval_days)) }
            }
            tr {
                th { "Review count" }
                td { (p.review_count) }
            }
        },
    }
}

/// Render a card's review history as a table, shared with the cloze view.
pub fn render_history(title: &str, reviews: &[ReviewRow]) -> Markup {
    html! {
        h2 { (title) }
        @if reviews.is_empty() {
            p .empty { "No reviews yet." }
        } @else {
            table .history {
                thead {
                    tr {
                        th { "Date" }
                        th { "Grade" }
                        th { "Stability" }
                        th { "Difficulty" }
                        th { "Interval" }
                        th { "Due date" }
                    }
                }
                tbody {
                    @for review in reviews {
                        tr {
                            td .timestamp { (review.data.reviewed_at) }
                            td { (render_grade_badge(review.data.grade)) }
                            td { (format!("{:.2}", review.data.stability)) }
                            td { (format!("{:.2}", review.data.difficulty)) }
                            td { (format!("{} days", review.data.interval_days)) }
                            td { (review.data.due_date) }
                        }
                    }
                }
            }
        }
    }
}

/// Render a colored badge for a review grade, shared with the cloze view.
pub fn render_grade_badge(grade: Grade) -> Markup {
    let label = match grade {
        Grade::Forgot => "Forgot",
        Grade::Hard => "Hard",
        Grade::Good => "Good",
        Grade::Easy => "Easy",
    };
    let class = format!("grade-badge grade-{}", grade.as_str());
    html! {
        span class=(class) { (label) }
    }
}

fn deck_url(deck_name: &str) -> String {
    format!("/deck/{}", utf8_percent_encode(deck_name, NON_ALPHANUMERIC))
}
