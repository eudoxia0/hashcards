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

use axum::Form;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use axum::response::Redirect;
use maud::Markup;
use maud::PreEscaped;
use maud::html;
use serde::Deserialize;

use crate::db::Review;
use crate::drill::state::ServerState;
use crate::drill::template::page_template;
use crate::error::Fallible;
use crate::fsrs::Grade;
use crate::markdown::markdown_to_html;
use crate::parser::CardContent;
use crate::types::perf::Performance;
use crate::types::timestamp::Timestamp;

const CLOZE_TAG: &str = "CLOZE_DELETION";

pub async fn get_handler(State(state): State<ServerState>) -> (StatusCode, Html<String>) {
    let mutable = state.mutable.lock().unwrap();
    let body = if mutable.cards.is_empty() {
        html! {
            div.finished {
                h1 {
                    "Session Completed"
                }
            }
        }
    } else {
        let progress = format!(
            "{} / {}",
            state.total_cards - mutable.cards.len(),
            state.total_cards
        );
        let card = mutable.cards[0].clone();
        let card_content: Markup = match card.content() {
            CardContent::Basic { question, answer } => {
                let question = markdown_to_html(question);
                let answer = markdown_to_html(answer);
                if mutable.reveal {
                    html! {
                        div.content {
                            div .question .rich-text {
                                p {
                                    (PreEscaped(question))
                                }
                            }
                            div .answer .rich-text {
                                p {
                                    (PreEscaped(answer))
                                }
                            }
                        }
                    }
                } else {
                    html! {
                        div.content {
                            div.question .rich-text {
                                p {
                                    (PreEscaped(question))
                                }
                            }
                            div.answer .rich-text {}
                        }
                    }
                }
            }
            CardContent::Cloze { text, start, end } => {
                if mutable.reveal {
                    let cloze_text = &text[*start..*end + 1];
                    let mut answer = text.clone();
                    answer.replace_range(*start..*end + 1, CLOZE_TAG);
                    let answer = markdown_to_html(&answer);
                    let answer = answer.replace(
                        CLOZE_TAG,
                        &format!("<span class='cloze-reveal'>{}</span>", cloze_text),
                    );
                    html! {
                        div.content{
                            div.prompt .rich-text {
                                p {
                                    (PreEscaped(answer))
                                }
                            }
                        }
                    }
                } else {
                    let mut prompt = text.clone();
                    prompt.replace_range(*start..*end + 1, CLOZE_TAG);
                    let prompt = markdown_to_html(&prompt);
                    let prompt =
                        prompt.replace(CLOZE_TAG, "<span class='cloze'>.............</span>");
                    html! {
                        div.content {
                            div.prompt .rich-text {
                                p {
                                    (PreEscaped(prompt))
                                }
                            }
                        }
                    }
                }
            }
        };
        let card_controls = if mutable.reveal {
            html! {
                form action="/" method="post" {
                    input id="forgot" type="submit" name="action" value="Forgot";
                    input id="hard" type="submit" name="action" value="Hard";
                    input id="good" type="submit" name="action" value="Good";
                    input id="easy" type="submit" name="action" value="Easy";
                }
            }
        } else {
            html! {
                form action="/" method="post" {
                    input id="reveal" type="submit" name="action" value="Reveal";
                }
            }
        };
        html! {
            div.root {
                div.card {
                    div.header {
                        h1 {
                            (card.deck_name())
                        }
                        div.progress {
                            (progress)
                        }
                    }
                    (card_content)
                    div.controls {
                        (card_controls)
                    }
                }
            }
        }
    };
    let html = page_template(body);
    (StatusCode::OK, Html(html.into_string()))
}

#[derive(Debug, Deserialize)]
enum Action {
    Reveal,
    Forgot,
    Hard,
    Good,
    Easy,
}

#[derive(Deserialize)]
pub struct FormData {
    action: Action,
}

pub async fn post_handler(
    State(state): State<ServerState>,
    Form(form): Form<FormData>,
) -> Redirect {
    match action_handler(state, form.action).await {
        Ok(_) => {}
        Err(e) => {
            log::error!("error: {e}");
        }
    }
    Redirect::to("/")
}

async fn action_handler(state: ServerState, action: Action) -> Fallible<()> {
    let mut mutable = state.mutable.lock().unwrap();
    match action {
        Action::Reveal => {
            if mutable.reveal {
                log::error!("Revealing a card that is already revealed.");
            } else {
                mutable.reveal = true;
            }
        }
        Action::Forgot | Action::Hard | Action::Good | Action::Easy => {
            if !mutable.reveal {
                log::error!("Answering a card that is not revealed.");
            } else {
                let card = mutable.cards.remove(0);
                let hash = card.hash();
                let performance = mutable.db.get_card_performance(hash)?;
                let grade: Grade = match action {
                    Action::Forgot => Grade::Forgot,
                    Action::Hard => Grade::Hard,
                    Action::Good => Grade::Good,
                    Action::Easy => Grade::Easy,
                    _ => unreachable!(),
                };
                let performance = Performance::update(performance, grade, state.today);
                let review = Review {
                    card_hash: hash,
                    reviewed_at: Timestamp::now(),
                    grade,
                    stability: performance.stability,
                    difficulty: performance.difficulty,
                    due_date: performance.due_date,
                };
                mutable.reviews.push(review);

                // Was the card forgotten? Put it at the back.
                if grade == Grade::Forgot {
                    mutable.cards.push(card);
                }
                mutable.reveal = false;

                // Was this the last card?
                if mutable.cards.is_empty() {
                    log::debug!("Session completed");
                    let session_ended_at = Timestamp::now();
                    mutable.db.save_session(
                        state.session_started_at,
                        session_ended_at,
                        &mutable.reviews,
                    )?;
                }
            }
        }
    }
    Ok(())
}
