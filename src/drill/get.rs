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
use maud::PreEscaped;
use maud::html;

use crate::drill::state::ServerState;
use crate::drill::template::page_template;
use crate::markdown::markdown_to_html;
use crate::types::card::CardContent;

const CLOZE_TAG: &str = "CLOZE_DELETION";

pub async fn get_handler(State(state): State<ServerState>) -> (StatusCode, Html<String>) {
    let mutable = state.mutable.lock().unwrap();
    let undo_disabled = mutable.reviewed.is_empty();
    let body = if mutable.finished {
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
