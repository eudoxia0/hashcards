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
use crate::fsrs::retrievability;
use crate::markdown::MarkdownRenderConfig;
use crate::markdown::markdown_to_html_inline;
use crate::types::card::Card;
use crate::types::card_hash::CardHash;
use crate::types::date::Date;
use crate::types::performance::Performance;
use crate::types::performance::ReviewedPerformance;

pub async fn basic_card_handler(
    State(state): State<BrowseState>,
    Path(hash): Path<String>,
) -> (StatusCode, Html<String>) {
    let hash = match CardHash::from_hex(&hash) {
        Ok(hash) => hash,
        Err(_) => {
            return error_response(
                StatusCode::NOT_FOUND,
                &format!("Invalid card hash '{hash}'."),
            );
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
    match columns_page(&state, selection, Some(detail)) {
        Ok(markup) => ok_response(markup),
        Err(e) => internal_error_response(e),
    }
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
    match columns_page(&state, selection, Some(detail)) {
        Ok(markup) => ok_response(markup),
        Err(e) => internal_error_response(e),
    }
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
                    (performance_rows(state.performance_of(card.hash()), state.today))
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
                        (performance_rows(state.performance_of(sibling.hash()), state.today))
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
    let source = format!(
        "{}, lines {}–{}",
        source_path.display(),
        start_line,
        end_line
    );
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

/// A card's review history, in chronological order: when it was reviewed,
/// how many days had passed since the first review, and with what grade.
fn render_history(state: &BrowseState, hash: CardHash) -> Markup {
    let reviews = state.reviews_of(hash);
    let first = match reviews.first() {
        Some(first) => first,
        None => {
            return html! {
                p .empty { "No reviews yet." }
            };
        }
    };
    let first_date = first.reviewed_at.date();
    html! {
        table .history {
            thead {
                tr {
                    th { "Reviewed" }
                    th .day { "Day" }
                    th { "Grade" }
                }
            }
            tbody {
                @for review in reviews {
                    tr {
                        td { (review.reviewed_at.into_inner().format(TS_FORMAT).to_string()) }
                        td .day {
                            (days_between(first_date, review.reviewed_at.date()))
                        }
                        td { (grade_chip(review.grade)) }
                    }
                }
            }
        }
    }
}

/// The number of days from the first date to the second.
fn days_between(from: Date, to: Date) -> i64 {
    (to.into_inner() - from.into_inner()).num_days()
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

const STABILITY_EXPLANATION: &str =
    "The number of days until recall probability falls to the target retention.";

const RECALL_EXPLANATION: &str =
    "The estimated probability of recalling this card if it were reviewed today.";

fn performance_rows(performance: Performance, today: Date) -> Markup {
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
                td .key title=(STABILITY_EXPLANATION) { "Stability" }
                td .val { (format!("{:.2}", rp.stability)) " days" }
            }
            tr {
                td .key { "Difficulty" }
                td .val { (difficulty_chip(rp.difficulty)) }
            }
            tr {
                td .key title=(RECALL_EXPLANATION) { "Predicted Recall" }
                td .val { (predicted_recall(&rp, today)) }
            }
            tr {
                td .key { "Interval (days)" }
                td .val { (rp.interval_days) }
            }
            tr {
                td .key { "Due Date" }
                td .val { (rp.due_date) " (" (relative_due(rp.due_date, today)) ")" }
            }
        },
    }
}

/// FSRS difficulty ranges from 1 (easiest) to 10 (hardest). Colour-code it
/// from green to red.
fn difficulty_chip(difficulty: f64) -> Markup {
    let clamped = difficulty.clamp(1.0, 10.0);
    let hue = 120.0 * (10.0 - clamped) / 9.0;
    let style = format!("background: hsl({hue:.0}, 70%, 85%); color: hsl({hue:.0}, 90%, 20%);");
    html! {
        span .difficulty style=(style) { (format!("{:.2}", difficulty)) }
    }
}

/// The FSRS-estimated probability of recalling a card today, as a
/// percentage.
fn predicted_recall(rp: &ReviewedPerformance, today: Date) -> String {
    let elapsed = days_between(rp.last_reviewed_at.date(), today).max(0) as f64;
    let recall = retrievability(elapsed, rp.stability);
    format!("{:.1}%", recall * 100.0)
}

/// A due date relative to today, e.g. "today", "in 2 days", "3 days ago".
fn relative_due(due: Date, today: Date) -> String {
    let days = days_between(today, due);
    match days {
        0 => "today".to_string(),
        1 => "tomorrow".to_string(),
        -1 => "yesterday".to_string(),
        d if d > 1 => format!("in {d} days"),
        d => format!("{} days ago", -d),
    }
}
