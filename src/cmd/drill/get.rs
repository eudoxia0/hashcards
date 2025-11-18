// Copyright 2025 Fernando Borretti
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

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use maud::Markup;
use maud::html;

use crate::cmd::drill::state::MutableState;
use crate::cmd::drill::state::ServerState;
use crate::cmd::drill::template::page_template;
use crate::error::Fallible;
use crate::markdown::MarkdownRenderConfig;
use crate::media::resolve::MediaResolverBuilder;
use crate::types::card::Card;
use crate::types::card::CardType;

pub async fn get_handler(State(state): State<ServerState>) -> (StatusCode, Html<String>) {
    let html = match inner(state).await {
        Ok(html) => html,
        Err(e) => page_template(html! {
            div.error {
                h1 { "Error" }
                p { (e) }
            }
        }),
    };
    (StatusCode::OK, Html(html.into_string()))
}

async fn inner(state: ServerState) -> Fallible<Markup> {
    let mutable = state.mutable.lock().unwrap();
    let body = if mutable.finished_at.is_some() {
        render_completion_page(&state, &mutable)?
    } else {
        render_session_page(&state, &mutable)?
    };
    let html = page_template(body);
    Ok(html)
}

fn render_session_page(state: &ServerState, mutable: &MutableState) -> Fallible<Markup> {
    let undo_disabled = mutable.reviews.is_empty();
    let total_cards = state.total_cards;
    let cards_done = state.total_cards - mutable.cards.len();
    let percent_done = if total_cards == 0 {
        100
    } else {
        (cards_done * 100) / total_cards
    };
    let progress_bar_style = format!("width: {}%;", percent_done);
    let card = mutable.cards[0].clone();
    let coll_path = state.directory.clone();
    let deck_path = card.relative_file_path(&coll_path)?;
    let config = MarkdownRenderConfig {
        resolver: MediaResolverBuilder::new()
            .with_collection_path(coll_path)?
            .with_deck_path(deck_path)?
            .build()?,
        port: state.port,
    };
    let card_content = render_card(&card, mutable.reveal, &config)?;
    let card_controls = if mutable.reveal {
        html! {
            form action="/" method="post" {
                (undo_button(undo_disabled))
                div.spacer {}
                div.grades {
                    input id="forgot" type="submit" name="action" value="Forgot";
                    input id="hard" type="submit" name="action" value="Hard";
                    input id="good" type="submit" name="action" value="Good";
                    input id="easy" type="submit" name="action" value="Easy";
                }
                div.spacer {}
                (end_button())
            }
        }
    } else {
        html! {
            form action="/" method="post" {
                (undo_button(undo_disabled))
                div.spacer {}
                input id="reveal" type="submit" name="action" value="Reveal" title="Show the answer";
                div.spacer {}
                (end_button())
            }
        }
    };
    let html = html! {
        div.root {
            div.header {
                div.progress-bar {
                    div.progress-fill style=(progress_bar_style) {}
                }
            }
            div.card-container {
                div.card {
                    div.card-header {
                        h1 {
                            (card.deck_name())
                        }
                    }
                    (card_content)
                }
            }
            div.controls {
                (card_controls)
            }
        }
    };
    Ok(html)
}

fn render_card(card: &Card, reveal: bool, config: &MarkdownRenderConfig) -> Fallible<Markup> {
    let html = match card.card_type() {
        CardType::Basic => {
            if reveal {
                html! {
                    div .question .rich-text {
                        (card.html_front(config)?)
                    }
                    div .answer .rich-text {
                        (card.html_back(config)?)
                    }
                }
            } else {
                html! {
                    div .question .rich-text {
                        (card.html_front(config)?)
                    }
                    div .answer .rich-text {}
                }
            }
        }
        CardType::Cloze => {
            if reveal {
                html! {
                    div .prompt .rich-text {
                        (card.html_back(config)?)
                    }
                }
            } else {
                html! {
                    div .prompt .rich-text {
                        (card.html_front(config)?)
                    }
                }
            }
        }
    };
    Ok(html! {
        div.card-content {
            (html)
        }
    })
}

const TS_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

fn render_completion_page(state: &ServerState, mutable: &MutableState) -> Fallible<Markup> {
    let total_cards = state.total_cards;
    let cards_reviewed = state.total_cards - mutable.cards.len();
    let start = state.session_started_at.into_inner();
    let end = mutable.finished_at.unwrap().into_inner();
    let duration_s = (end - start).num_seconds();
    let pace: f64 = if cards_reviewed == 0 {
        0.0
    } else {
        duration_s as f64 / cards_reviewed as f64
    };
    let pace = format!("{:.2}", pace);
    let start_ts = start.format(TS_FORMAT).to_string();
    let end_ts = end.format(TS_FORMAT).to_string();
    let html = html! {
        div.finished {
            h1 {
                "Session Completed 🎉"
            }
            div.summary {
                "Reviewed "
                (cards_reviewed)
                " cards in "
                (duration_s)
                " seconds."
            }
            h2 {
                "Session Stats"
            }
            div.stats {
                table {
                    tbody {
                        tr {
                            td .key { "Total Cards" }
                            td .val { (total_cards) }
                        }
                        tr {
                            td .key { "Cards Reviewed" }
                            td .val { (cards_reviewed) }
                        }
                        tr {
                            td .key { "Started" }
                            td .val { (start_ts) }
                        }
                        tr {
                            td .key { "Finished" }
                            td .val { (end_ts) }
                        }
                        tr {
                            td .key { "Duration (seconds)" }
                            td .val { (duration_s) }
                        }
                        tr {
                            td .key { "Pace (s/card)" }
                            td .val { (pace) }
                        }
                    }
                }
            }
            div.shutdown-container {
                form action="/" method="post" {
                    input #shutdown .shutdown-button type="submit" name="action" value="Shutdown" title="Shut down the server";
                }
            }
        }
    };
    Ok(html)
}

fn undo_button(disabled: bool) -> Markup {
    if disabled {
        html! {
            input id="undo" type="submit" name="action" value="Undo" disabled;
        }
    } else {
        html! {
            input id="undo" type="submit" name="action" value="Undo" title="Undo last action";
        }
    }
}

fn end_button() -> Markup {
    html! {
        input id="end" type="submit" name="action" value="End" title="End the session (changes are saved)";
    }
}
