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
use maud::Markup;
use maud::PreEscaped;
use maud::html;
use serde::Deserialize;

use crate::drill::state::ServerState;
use crate::drill::template::page_template;
use crate::fsrs::Grade;
use crate::parser::CardContent;

pub async fn root(State(state): State<ServerState>) -> (StatusCode, Html<String>) {
    render_page(state, None)
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

pub async fn action(
    State(state): State<ServerState>,
    Form(form): Form<FormData>,
) -> (StatusCode, Html<String>) {
    render_page(state, Some(form.action))
}

fn render_page(state: ServerState, action: Option<Action>) -> (StatusCode, Html<String>) {
    let mut mutable = state.mutable.lock().unwrap();

    if let Some(action) = action {
        match action {
            Action::Reveal => {
                if mutable.reveal {
                    log::error!("Revealing a card that is already revealed.");
                } else {
                    mutable.reveal = true;
                }
            }
            _ => {
                if !mutable.reveal {
                    log::error!("Answering a card that is not revealed.");
                } else {
                    let card = mutable.cards.remove(0);
                    let hash = card.hash();
                    let performance = mutable.db.get(hash).unwrap();
                    let grade: Grade = match action {
                        Action::Forgot => Grade::Forgot,
                        Action::Hard => Grade::Hard,
                        Action::Good => Grade::Good,
                        Action::Easy => Grade::Easy,
                        _ => unreachable!(),
                    };
                    let performance = performance.update(grade, state.today);
                    mutable.db.update(hash, performance);
                    // Was the card forgotten? Put it at the back.
                    if grade == Grade::Forgot {
                        mutable.cards.push(card);
                    }
                    mutable.reveal = false;
                }
            }
        }
    }

    let body = if mutable.cards.is_empty() {
        let mut writer = csv::Writer::from_path(&mutable.db_path).unwrap();
        log::debug!("Writing performance database");
        mutable.db.to_csv(&mut writer).unwrap();
        html! {
            p { "Finished!" }
        }
    } else {
        let card = mutable.cards[0].clone();
        let card_content: Markup = match card.content() {
            CardContent::Basic { question, answer } => {
                let question = markdown::to_html(question);
                let answer = markdown::to_html(answer);
                if mutable.reveal {
                    html! {
                        div.question {
                            p {
                                (PreEscaped(question))
                            }
                        }
                        div.answer {
                            p {
                                (PreEscaped(answer))
                            }
                        }
                    }
                } else {
                    html! {
                        div.question {
                            p {
                                (PreEscaped(question))
                            }
                        }
                        div.answer {}
                    }
                }
            }
            CardContent::Cloze { text, start, end } => {
                let cloze_text = &text[*start..*end + 1];
                let mut prompt = text.clone();
                prompt.replace_range(*start..*end + 1, "[.............](cloze)");
                let prompt = markdown::to_html(&prompt);
                let mut answer = text.clone();
                answer.replace_range(*start..*end + 1, &format!("[{cloze_text}](cloze_reveal)"));
                let answer = markdown::to_html(&answer);
                if mutable.reveal {
                    html! {
                        div.question {
                            p {
                                (PreEscaped(prompt))
                            }
                        }
                        div.answer {
                            p {
                                (PreEscaped(answer))
                            }
                        }
                    }
                } else {
                    html! {
                        div.question {
                            p {
                                (PreEscaped(prompt))
                            }
                        }
                        div.answer {}
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
                    div.deck {
                        h1 {
                            (card.deck_name())
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
