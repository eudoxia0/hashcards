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

use std::path::Path;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use maud::Markup;
use maud::html;

use crate::cmd::drill::server::AnswerControls;
use crate::cmd::drill::state::MutableState;
use crate::cmd::drill::state::ServerState;
use crate::cmd::drill::template::page_template;
use crate::error::Fallible;
use crate::markdown::MarkdownRenderConfig;
use crate::media::resolve::MediaResolverBuilder;
use crate::types::card::Card;
use crate::types::card::CardType;
use crate::types::timestamp::Timestamp;

/// What to show on the completion page.
pub enum CompletionAction {
    /// Show a "Shutdown" button (drill mode).
    Shutdown,
    /// Show a "Back to Collections" link (serve mode).
    BackToCollections,
}

/// Everything the rendering functions need, decoupled from ServerState.
pub struct RenderContext<'a> {
    pub directory: &'a Path,
    pub total_cards: usize,
    pub session_started_at: Timestamp,
    pub answer_controls: AnswerControls,
    pub form_action: &'a str,
    pub file_url_prefix: &'a str,
    pub completion_action: CompletionAction,
}

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
    let file_url_prefix = format!("http://localhost:{}/file", state.port);
    let ctx = RenderContext {
        directory: &state.directory,
        total_cards: state.total_cards,
        session_started_at: state.session_started_at,
        answer_controls: state.answer_controls,
        form_action: "/",
        file_url_prefix: &file_url_prefix,
        completion_action: CompletionAction::Shutdown,
    };
    let body = if mutable.finished_at.is_some() {
        render_completion_page(&ctx, &mutable)?
    } else {
        render_session_page(&ctx, &mutable)?
    };
    let html = page_template(body);
    Ok(html)
}

pub fn render_session_page(ctx: &RenderContext, mutable: &MutableState) -> Fallible<Markup> {
    let undo_disabled = mutable.reviews.is_empty();
    let total_cards = ctx.total_cards;
    let cards_done = ctx.total_cards - mutable.cards.len();
    let percent_done = if total_cards == 0 {
        100
    } else {
        (cards_done * 100) / total_cards
    };
    let progress_bar_style = format!("width: {}%;", percent_done);
    let card = mutable.cards[0].clone();
    let coll_path = ctx.directory.to_path_buf();
    let deck_path = card.relative_file_path(&coll_path)?;
    let config = MarkdownRenderConfig {
        resolver: MediaResolverBuilder::new()
            .with_collection_path(coll_path)?
            .with_deck_path(deck_path)?
            .build()?,
        file_url_prefix: ctx.file_url_prefix.to_string(),
    };
    let card_content = render_card(&card, mutable.reveal, &config)?;
    let form_action = ctx.form_action;
    let card_controls = if mutable.reveal {
        let grades = match ctx.answer_controls {
            AnswerControls::Binary => html! {
                input id="forgot" type="submit" name="action" value="Forgot" title="Mark card as forgotten.";
                input id="good" type="submit" name="action" value="Good" title="Mark card as remembered.";
            },
            AnswerControls::Full => html! {
                input id="forgot" type="submit" name="action" value="Forgot" title="Mark card as forgotten. Shortcut: 1.";
                input id="hard" type="submit" name="action" value="Hard" title="Mark card as difficult. Shortcut: 2.";
                input id="good" type="submit" name="action" value="Good" title="Mark card as remembered well. Shortcut: 3.";
                input id="easy" type="submit" name="action" value="Easy" title="Mark card as very easy. Shortcut: 4.";
            },
        };
        html! {
            form action=(form_action) method="post" {
                (undo_button(undo_disabled))
                div.spacer {}
                div.grades {
                    (grades)
                }
                div.spacer {}
                (end_button())
            }
        }
    } else {
        html! {
            form action=(form_action) method="post" {
                (undo_button(undo_disabled))
                div.spacer {}
                input id="reveal" type="submit" name="action" value="Reveal" title="Show the answer. Shortcut: space.";
                div.spacer {}
                (end_button())
            }
        }
    };
    let html = html! {
        div.root {
            div.header {
                div.progress-bar
                    role="progressbar"
                    aria-label="Study progress"
                    aria-valuenow=(percent_done)
                    aria-valuemin="0"
                    aria-valuemax="100"
                {
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

const REDIRECT_SCRIPT: &str = r#"
(function() {
    var secs = 5;
    var el = document.getElementById('countdown');
    var timer = setInterval(function() {
        secs--;
        if (el) el.textContent = secs;
        if (secs <= 0) {
            clearInterval(timer);
            var form = document.getElementById('home-form');
            if (form) form.submit();
        }
    }, 1000);
    var cancel = document.getElementById('cancel-redirect');
    if (cancel) {
        cancel.addEventListener('click', function(e) {
            e.preventDefault();
            clearInterval(timer);
            var notice = document.querySelector('.redirect-notice');
            if (notice) notice.style.display = 'none';
        });
    }
})();
"#;

pub fn render_completion_page(ctx: &RenderContext, mutable: &MutableState) -> Fallible<Markup> {
    let total_cards = ctx.total_cards;
    let cards_reviewed = ctx.total_cards - mutable.cards.len();
    let start = ctx.session_started_at.into_inner();
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

    let pace_rounded = pace.parse::<f64>().unwrap_or(0.0).round() as i64;
    let duration_min = duration_s / 60;
    let duration_display = if duration_min >= 1 {
        format!("{duration_min} min")
    } else {
        format!("{duration_s} s")
    };
    let summary_line = format!(
        "Done — {cards_reviewed} card{} in {duration_display} ({pace_rounded} s/card).",
        if cards_reviewed == 1 { "" } else { "s" }
    );

    let (action_button, redirect_notice) = match &ctx.completion_action {
        CompletionAction::Shutdown => (
            html! {
                div.shutdown-container {
                    form action=(ctx.form_action) method="post" {
                        input #shutdown .shutdown-button.btn.btn-danger type="submit" name="action" value="Shutdown" title="Shut down the server";
                    }
                }
            },
            html! {},
        ),
        CompletionAction::BackToCollections => (
            html! {
                div.shutdown-container {
                    form #home-form action=(ctx.form_action) method="post" style="display:inline" {
                        input type="hidden" name="action" value="Home";
                        button #home .home-button.btn.btn-primary type="submit" { "Home" }
                    }
                }
            },
            html! {
                p.redirect-notice {
                    "Returning to collections in "
                    span #countdown { "5" }
                    "s. "
                    a #cancel-redirect href="#" { "Cancel" }
                }
                script { (maud::PreEscaped(REDIRECT_SCRIPT)) }
            },
        ),
    };

    let html = html! {
        div.finished {
            h1 { "Session Completed" }
            div.summary { (summary_line) }
            (redirect_notice)
            details {
                summary { "Session Stats" }
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
            }
            (action_button)
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
            input id="undo" type="submit" name="action" value="Undo" title="Undo last action. Shortcut: u.";
        }
    }
}

fn end_button() -> Markup {
    html! {
        input id="end" type="submit" name="action" value="End" title="End the session (changes are saved)";
    }
}
