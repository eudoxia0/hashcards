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

use crate::drill::state::MutableState;
use crate::drill::state::ServerState;
use crate::drill::template::page_template;
use crate::error::Fallible;
use crate::types::card::Card;
use crate::types::card_type::CardType;

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
    let body = if mutable.finished {
        render_completion_page()?
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
    let card_content = render_card(&card, mutable.reveal)?;
    let card_controls = if mutable.reveal {
        html! {
            form action="/" method="post" {
                @if undo_disabled {
                    input id="undo" type="submit" name="action" value="Undo" disabled;
                } @else {
                    input id="undo" type="submit" name="action" value="Undo";
                }
                div.spacer {}
                input id="forgot" type="submit" name="action" value="Forgot";
                input id="hard" type="submit" name="action" value="Hard";
                input id="good" type="submit" name="action" value="Good";
                input id="easy" type="submit" name="action" value="Easy";
                div.spacer {}
                input id="end" type="submit" name="action" value="End";
            }
        }
    } else {
        html! {
            form action="/" method="post" {
                @if undo_disabled {
                    input id="undo" type="submit" name="action" value="Undo" disabled;
                } @else {
                    input id="undo" type="submit" name="action" value="Undo";
                }
                div.spacer {}
                input id="reveal" type="submit" name="action" value="Reveal";
                div.spacer {}
                input id="end" type="submit" name="action" value="End";
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

fn render_card(card: &Card, reveal: bool) -> Fallible<Markup> {
    let html = match card.card_type() {
        CardType::Basic => {
            if reveal {
                html! {
                    div .question .rich-text {
                        (card.html_front()?)
                    }
                    div .answer .rich-text {
                        (card.html_back()?)
                    }
                }
            } else {
                html! {
                    div .question .rich-text {
                        (card.html_front()?)
                    }
                    div .answer .rich-text {}
                }
            }
        }
        CardType::Cloze => {
            if reveal {
                html! {
                    div .prompt .rich-text {
                        (card.html_back()?)
                    }
                }
            } else {
                html! {
                    div .prompt .rich-text {
                        (card.html_front()?)
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

fn render_completion_page() -> Fallible<Markup> {
    let html = html! {
        div.finished {
            h1 {
                "Session Completed 🎉"
            }
        }
    };
    Ok(html)
}
