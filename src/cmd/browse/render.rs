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

use maud::Markup;
use maud::html;

use crate::cmd::browse::state::BrowseState;
use crate::error::Fallible;
use crate::error::fail;
use crate::fsrs::Grade;
use crate::fsrs::retrievability;
use crate::markdown::MarkdownRenderConfig;
use crate::media::resolve::MediaResolverBuilder;
use crate::types::card::Card;
use crate::types::card::CardContent;
use crate::types::card::html_cloze_family;
use crate::types::card_hash::CardHash;
use crate::types::date::Date;
use crate::types::performance::Performance;
use crate::types::performance::ReviewedPerformance;

/// Build the Markdown render configuration for the given card, for the
/// detail pane. Media paths are resolved relative to the file the card was
/// parsed from.
pub fn render_config(state: &BrowseState, card: &Card) -> Fallible<MarkdownRenderConfig> {
    build_config(state, card, true)
}

/// Build the Markdown render configuration for a card-list label: images and
/// audio are stripped.
pub fn label_config(state: &BrowseState, card: &Card) -> Fallible<MarkdownRenderConfig> {
    build_config(state, card, false)
}

fn build_config(
    state: &BrowseState,
    card: &Card,
    render_media: bool,
) -> Fallible<MarkdownRenderConfig> {
    let coll_path = state.directory.clone();
    let deck_path = card.relative_file_path(&coll_path)?;
    Ok(MarkdownRenderConfig {
        resolver: MediaResolverBuilder::new()
            .with_collection_path(coll_path)?
            .with_deck_path(deck_path)?
            .build()?,
        resource_hostname: state.resource_hostname.clone(),
        port: state.port,
        autoplay_audio: false,
        render_media,
    })
}

/// The deck, type, and source rows shared by both detail pages.
pub fn source_rows(state: &BrowseState, card: &Card, card_type: &str) -> Fallible<Markup> {
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
pub fn render_history(state: &BrowseState, hash: CardHash) -> Markup {
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

pub fn performance_rows(performance: Performance, today: Date) -> Markup {
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

/// Render a cloze family's shared text with every deletion revealed. The
/// siblings must be sorted by deletion position.
pub fn render_family_revealed(
    siblings: &[Card],
    config: &MarkdownRenderConfig,
) -> Fallible<Markup> {
    let text = family_text(siblings)?;
    let deletions = family_deletions(siblings);
    let html = html_cloze_family(config, text, &deletions)?;
    Ok(html! {
        div .card-content {
            div .prompt .rich-text {
                (html)
            }
        }
    })
}

/// The text shared by the cards of a cloze family.
fn family_text(siblings: &[Card]) -> Fallible<&str> {
    match siblings.first().map(|card| card.content()) {
        Some(CardContent::Cloze { text, .. }) => Ok(text),
        _ => fail("cloze family has no cloze cards."),
    }
}

/// The deletion ranges of the cards of a cloze family.
fn family_deletions(siblings: &[Card]) -> Vec<(usize, usize)> {
    siblings
        .iter()
        .filter_map(|card| match card.content() {
            CardContent::Cloze { start, end, .. } => Some((*start, *end)),
            CardContent::Basic { .. } => None,
        })
        .collect()
}
