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

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

use axum::Form;
use axum::Router;
use axum::extract::State;
use axum::http::HeaderName;
use axum::http::StatusCode;
use axum::http::header::CACHE_CONTROL;
use axum::http::header::CONTENT_TYPE;
use axum::response::Html;
use axum::routing::get;
use axum::routing::post;
use chrono::NaiveDate;
use csv::Reader;
use maud::DOCTYPE;
use maud::Markup;
use maud::PreEscaped;
use maud::html;
use serde::Deserialize;
use tokio::net::TcpListener;

use crate::db::Database;
use crate::db::Performance;
use crate::error::Fallible;
use crate::error::fail;
use crate::fsrs::Grade;
use crate::hash::Hash;
use crate::parser::Card;
use crate::parser::CardContent;
use crate::parser::parse_deck;

#[derive(Clone)]
pub struct ServerState {
    today: NaiveDate,
    mutable: Arc<Mutex<MutableState>>,
}

pub struct MutableState {
    macros: Vec<(String, String)>,
    db_path: PathBuf,
    reveal: bool,
    db: Database,
    cards: Vec<Card>,
}

pub async fn drill(directory: PathBuf, today: NaiveDate) -> Fallible<()> {
    if !directory.exists() {
        return fail("directory does not exist.");
    }

    let db_path = directory.join("performance.csv");
    let mut db = if db_path.exists() {
        log::debug!("Loading performance database...");
        let mut reader = Reader::from_path(&db_path)?;
        let db = Database::from_csv(&mut reader)?;
        log::debug!("Database loaded.");
        db
    } else {
        log::debug!("Using empty performance database.");
        Database::empty()
    };

    let mut macros = Vec::new();
    let macros_path = directory.join("macros.tex");
    if macros_path.exists() {
        let content = std::fs::read_to_string(macros_path)?;
        for line in content.lines() {
            if let Some((name, definition)) = line.split_once(' ') {
                macros.push((name.to_string(), definition.to_string()));
            }
        }
    }

    log::debug!("Loading deck...");
    let start = Instant::now();
    let all_cards = parse_deck(directory)?;
    let end = Instant::now();
    let duration = end.duration_since(start).as_millis();
    log::debug!("Deck loaded in {duration}ms.");

    let db_keys: HashSet<Hash> = db.keys();
    let dir_keys: HashSet<Hash> = all_cards.iter().map(|card| card.hash()).collect();
    // If a card is in the DB, but not in the directory, it was deleted. Therefore, remove it from the database.
    let to_remove: Vec<Hash> = db_keys.difference(&dir_keys).cloned().collect();
    for hash in to_remove {
        db.remove(&hash);
    }
    // If a card is in the directory, but not in the DB, it is new. Add it to the database.
    let to_add: Vec<Hash> = dir_keys.difference(&db_keys).cloned().collect();
    for hash in to_add {
        db.insert(hash, Performance::New);
    }

    // Find cards due today.
    let due_today = db.due_today(today);
    let due_today: Vec<Card> = all_cards
        .into_iter()
        .filter(|card| due_today.contains(&card.hash()))
        .collect::<Vec<_>>();
    if due_today.is_empty() {
        println!("No cards due today.");
        return Ok(());
    }

    let state = ServerState {
        today,
        mutable: Arc::new(Mutex::new(MutableState {
            macros,
            db_path,
            reveal: false,
            db,
            cards: due_today,
        })),
    };
    let app = Router::new();
    let app = app.route("/", get(root));
    let app = app.route("/", post(action));
    let app = app.route("/script.js", get(script));
    let app = app.route("/style.css", get(stylesheet));
    let app = app.fallback(not_found_handler);
    let app = app.with_state(state);
    let bind = "0.0.0.0:8000";
    log::debug!("Starting server on {bind}");
    let listener = TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;
    todo!()
}

async fn root(State(state): State<ServerState>) -> (StatusCode, Html<String>) {
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
struct FormData {
    action: Action,
}

async fn action(
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

fn page_template(body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "hashcards" }
                link rel="stylesheet" href="/style.css";
                link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.16.22/dist/katex.min.css" integrity="sha384-5TcZemv2l/9On385z///+d7MSYlvIEw9FuZTIdZ14vJLqWphw7e7ZPuOiCHJcFCP" crossorigin="anonymous";
                script defer src="https://cdn.jsdelivr.net/npm/katex@0.16.22/dist/katex.min.js" integrity="sha384-cMkvdD8LoxVzGF/RPUKAcvmm49FQ0oxwDF3BGKtDXcEc+T1b2N+teh/OJfpU0jr6" crossorigin="anonymous" {};
                script defer src="https://cdn.jsdelivr.net/npm/katex@0.16.22/dist/contrib/auto-render.min.js" integrity="sha384-hCXGrW6PitJEwbkoStFjeJxv+fSOOQKOPbJxSfM6G5sWZjAyWhXiTIIAmQqnlLlh" crossorigin="anonymous" {};
            }
            body {
                (body)
                script src="/script.js" {};
            }
        }
    }
}

async fn script(
    State(state): State<ServerState>,
) -> (StatusCode, [(HeaderName, &'static str); 1], String) {
    let state = state.mutable.lock().unwrap();
    let mut content = String::new();
    content.push_str("let MACROS = {};\n");
    for (name, definition) in &state.macros {
        content.push_str(&format!(
            "MACROS[String.raw`{name}`] = String.raw`{definition}`;\n"
        ));
    }
    content.push('\n');
    content.push_str(include_str!("script.js"));
    (StatusCode::OK, [(CONTENT_TYPE, "text/javascript")], content)
}

async fn stylesheet() -> (StatusCode, [(HeaderName, &'static str); 2], &'static [u8]) {
    let bytes = include_bytes!("style.css");
    (
        StatusCode::OK,
        [
            (CONTENT_TYPE, "text/css"),
            (CACHE_CONTROL, "public, max-age=604800, immutable"),
        ],
        bytes,
    )
}

async fn not_found_handler() -> (StatusCode, Html<String>) {
    (StatusCode::OK, Html("Not Found".to_string()))
}
