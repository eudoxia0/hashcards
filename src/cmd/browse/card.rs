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
use maud::Markup;
use maud::PreEscaped;
use maud::html;

use crate::cmd::browse::entries::EntryKey;
use crate::cmd::browse::layout::Selection;
use crate::cmd::browse::layout::columns_page;
use crate::cmd::browse::render::render_config;
use crate::cmd::browse::render::render_family_revealed;
use crate::cmd::browse::state::BrowseState;
use crate::cmd::browse::template::error_response;
use crate::cmd::browse::template::internal_error_response;
use crate::cmd::browse::template::ok_response;
use crate::error::ErrorReport;
use crate::error::Fallible;
use crate::error::fail;
use crate::fsrs::Grade;
use crate::markdown::MarkdownRenderConfig;
use crate::markdown::markdown_to_html_inline;
use crate::types::card::Card;
use crate::types::card_hash::CardHash;
use crate::types::performance::Performance;

pub async fn basic_card_handler(
    State(state): State<BrowseState>,
    Path(hash): Path<String>,
) -> (StatusCode, Html<String>) {
    let hash = match CardHash::from_hex(&hash) {
        Ok(hash) => hash,
        Err(_) => {
            return error_response(StatusCode::NOT_FOUND, &format!("Invalid card hash '{hash}'."));
        }
    };
    let card = state.cards.iter().find(|card| card.hash() == hash);
    let card = match card {
        Some(card) => card,
        None => {
            return error_response(
                StatusCode::NOT_FOUND,
                &format!("No card with hash '{hash}' in this collection."),
            );
        }
    };
    let detail = match render_basic_detail(&state, card) {
        Ok(detail) => detail,
        Err(e) => return internal_error_response(e),
    };
    let selection = Selection {
        deck: Some(card.deck_name()),
        entry: Some(EntryKey::Basic(hash)),
    };
    ok_response(columns_page(&state, selection, Some(detail)))
}

pub async fn cloze_family_handler(
    State(state): State<BrowseState>,
    Path(hash): Path<String>,
) -> (StatusCode, Html<String>) {
    let family = match CardHash::from_hex(&hash) {
        Ok(hash) => hash,
        Err(_) => {
            return error_response(
                StatusCode::NOT_FOUND,
                &format!("Invalid family hash '{hash}'."),
            );
        }
    };
    let siblings = match state.families.get(&family) {
        Some(siblings) => siblings,
        None => {
            return error_response(
                StatusCode::NOT_FOUND,
                &format!("No cloze card with family hash '{family}' in this collection."),
            );
        }
    };
    let first = match siblings.first() {
        Some(first) => first,
        None => {
            return internal_error_response(ErrorReport::new("cloze family has no cards."));
        }
    };
    let detail = match render_cloze_detail(&state, family, siblings) {
        Ok(detail) => detail,
        Err(e) => return internal_error_response(e),
    };
    let selection = Selection {
        deck: Some(first.deck_name()),
        entry: Some(EntryKey::Family(family)),
    };
    ok_response(columns_page(&state, selection, Some(detail)))
}

fn render_basic_detail(state: &BrowseState, card: &Card) -> Fallible<Markup> {
    let config = render_config(state, card)?;
    Ok(html! {
        h2 { "Front" }
        div .browse-card {
            div .card-content {
                div .prompt .rich-text {
                    (card.html_front(&config)?)
                }
            }
        }
        h2 { "Back" }
        div .browse-card {
            div .card-content {
                div .prompt .rich-text {
                    (card.html_back(&config)?)
                }
            }
        }
        h2 { "Details" }
        div .stats {
            table {
                tbody {
                    (source_rows(state, card, "Basic")?)
                    tr {
                        td .key { "Hash" }
                        td .val { code { (card.hash()) } }
                    }
                    (performance_rows(state.performance_of(card.hash())))
                }
            }
        }
        h2 { "History" }
        (render_history(state, card.hash()))
    })
}

fn render_cloze_detail(
    state: &BrowseState,
    family: CardHash,
    siblings: &[Card],
) -> Fallible<Markup> {
    let first = match siblings.first() {
        Some(first) => first,
        None => return fail("cloze family has no cards."),
    };
    let config = render_config(state, first)?;
    Ok(html! {
        h2 { "Text" }
        div .browse-card {
            (render_family_revealed(siblings, &config)?)
        }
        h2 { "Details" }
        div .stats {
            table {
                tbody {
                    (source_rows(state, first, "Cloze")?)
                    tr {
                        td .key { "Family Hash" }
                        td .val { code { (family) } }
                    }
                    tr {
                        td .key { "Clozes" }
                        td .val { (siblings.len()) }
                    }
                }
            }
        }
        h2 { "Clozes" }
        div .card-list {
            @for (index, sibling) in siblings.iter().enumerate() {
                (render_sibling(state, sibling, index, &config)?)
            }
        }
    })
}

/// One section per cloze card in the family: the deleted text, and the card's
/// own review stats and history.
fn render_sibling(
    state: &BrowseState,
    sibling: &Card,
    index: usize,
    config: &MarkdownRenderConfig,
) -> Fallible<Markup> {
    let deleted = match sibling.content().deletion_text()? {
        Some(deleted) => deleted,
        None => return fail("cloze family contains a non-cloze card."),
    };
    let deleted = markdown_to_html_inline(config, &deleted)?;
    Ok(html! {
        div .browse-card .cloze-sibling {
            div .cloze-sibling-header {
                span .badge { "Cloze " (index + 1) }
                span .deletion { (PreEscaped(deleted)) }
            }
            div .stats {
                table {
                    tbody {
                        tr {
                            td .key { "Hash" }
                            td .val { code { (sibling.hash()) } }
                        }
                        (performance_rows(state.performance_of(sibling.hash())))
                    }
                }
            }
            div .history-container {
                (render_history(state, sibling.hash()))
            }
        }
    })
}

/// The deck, type, and source rows shared by both detail pages.
fn source_rows(state: &BrowseState, card: &Card, card_type: &str) -> Fallible<Markup> {
    let source_path = card.relative_file_path(&state.directory)?;
    let (start_line, end_line) = card.range();
    let source = format!("{}, lines {}–{}", source_path.display(), start_line, end_line);
    Ok(html! {
        tr {
            td .key { "Type" }
            td .val { (card_type) }
        }
        tr {
            td .key { "Source" }
            td .val { (source) }
        }
    })
}

const TS_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

/// A card's review history: when it was reviewed, and with what grade, most
/// recent review first.
fn render_history(state: &BrowseState, hash: CardHash) -> Markup {
    let reviews = state.reviews_of(hash);
    if reviews.is_empty() {
        return html! {
            p .empty { "No reviews yet." }
        };
    }
    html! {
        table .history {
            thead {
                tr {
                    th { "Reviewed" }
                    th { "Grade" }
                }
            }
            tbody {
                @for review in reviews.iter().rev() {
                    tr {
                        td { (review.reviewed_at.into_inner().format(TS_FORMAT).to_string()) }
                        td { (grade_chip(review.grade)) }
                    }
                }
            }
        }
    }
}

fn grade_chip(grade: Grade) -> Markup {
    let label = match grade {
        Grade::Forgot => "Forgot",
        Grade::Hard => "Hard",
        Grade::Good => "Good",
        Grade::Easy => "Easy",
    };
    let class = format!("grade-{}", grade.as_str());
    html! {
        span .grade .(class) { (label) }
    }
}

fn performance_rows(performance: Performance) -> Markup {
    match performance {
        Performance::New => html! {
            tr {
                td .key { "Status" }
                td .val { "New (never reviewed)" }
            }
        },
        Performance::Reviewed(rp) => html! {
            tr {
                td .key { "Last Reviewed" }
                td .val { (rp.last_reviewed_at) }
            }
            tr {
                td .key { "Review Count" }
                td .val { (rp.review_count) }
            }
            tr {
                td .key { "Stability" }
                td .val { (format!("{:.2}", rp.stability)) }
            }
            tr {
                td .key { "Difficulty" }
                td .val { (format!("{:.2}", rp.difficulty)) }
            }
            tr {
                td .key { "Interval (days)" }
                td .val { (rp.interval_days) }
            }
            tr {
                td .key { "Due Date" }
                td .val { (rp.due_date) }
            }
        },
    }
}
