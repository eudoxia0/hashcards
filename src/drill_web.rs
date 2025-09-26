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
use axum::response::Redirect;
use axum::routing::get;
use axum::routing::post;
use blake3::Hash;
use chrono::NaiveDate;
use csv::Reader;
use maud::DOCTYPE;
use maud::Markup;
use maud::html;
use serde::Deserialize;
use tokio::net::TcpListener;

use crate::db::Database;
use crate::db::Performance;
use crate::error::Fallible;
use crate::error::fail;
use crate::fsrs::Grade;
use crate::parser::Card;
use crate::parser::CardContent;
use crate::parser::parse_deck;

#[derive(Clone)]
pub struct StateContainer {
    inner: Arc<Mutex<ServerState>>,
}

pub struct ServerState {
    db_path: PathBuf,
    today: NaiveDate,
    reveal: bool,
    db: Database,
    cards: Vec<Card>,
}

pub async fn drill_web(directory: PathBuf, today: NaiveDate) -> Fallible<()> {
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

    let state = StateContainer {
        inner: Arc::new(Mutex::new(ServerState {
            db_path,
            today,
            reveal: false,
            db,
            cards: due_today,
        })),
    };
    let app = Router::new();
    let app = app.route("/", get(root));
    let app = app.route("/", post(action));
    let app = app.route("/style.css", get(stylesheet));
    let app = app.fallback(not_found_handler);
    let app = app.with_state(state);
    let bind = "0.0.0.0:8000";
    log::debug!("Starting server on {bind}");
    let listener = TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;
    todo!()
}

async fn root(State(state): State<StateContainer>) -> (StatusCode, Html<String>) {
    let state = state.inner.lock().unwrap();
    let body = if state.cards.is_empty() {
        let mut writer = csv::Writer::from_path(&state.db_path).unwrap();
        state.db.to_csv(&mut writer).unwrap();
        html! {
            p { "Finished!" }
        }
    } else {
        let card = state.cards[0].clone();
        let card_content: Markup = match &card.content {
            CardContent::Basic { question, answer } => {
                if state.reveal {
                    html! {
                        div.question {
                            p {
                                (question)
                            }
                        }
                        div.answer {
                            p {
                                (answer)
                            }
                        }
                    }
                } else {
                    html! {
                        div.question {
                            p {
                                (question)
                            }
                        }
                        div.answer {}
                    }
                }
            }
            CardContent::Cloze { text, start, end } => {
                let cloze_text = &text[*start..*end + 1];
                let mut prompt = text.clone();
                prompt.replace_range(*start..*end + 1, "[...]");
                let mut answer = text.clone();
                answer.replace_range(*start..*end + 1, &format!("[{cloze_text}]"));
                if state.reveal {
                    html! {
                        div.prompt {
                            p {
                                (prompt)
                            }
                        }
                        div.answer {
                            p {
                                (answer)
                            }
                        }
                    }
                } else {
                    html! {
                        div.prompt {
                            p {
                                (prompt)
                            }
                        }
                        div.answer {}
                    }
                }
            }
        };
        let card_controls = if state.reveal {
            html! {
                form action="/" method="post" {
                    input type="submit" value="Forgot" name="action";
                    input type="submit" value="Hard" name="action";
                    input type="submit" value="Good" name="action";
                    input type="submit" value="Easy" name="action";
                }
            }
        } else {
            html! {
                form action="/" method="post" {
                    input type="submit" value="Reveal" name="action";
                }
            }
        };
        html! {
            div.root {
                div.card {
                    div.deck {
                        h1 {
                            (card.deck_name)
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
struct FormData {
    action: Action,
}

async fn action(State(state): State<StateContainer>, Form(form): Form<FormData>) -> Redirect {
    let mut state = state.inner.lock().unwrap();
    match form.action {
        Action::Reveal => {
            if state.reveal {
                log::error!("Revealing a card that is already revealed.");
            } else {
                state.reveal = true;
            }
        }
        _ => {
            if !state.reveal {
                log::error!("Answering a card that is not revealed.");
            } else {
                let card = state.cards.remove(0);
                let hash = card.hash();
                let performance = state.db.get(hash).unwrap();
                let grade: Grade = match form.action {
                    Action::Forgot => Grade::Forgot,
                    Action::Hard => Grade::Hard,
                    Action::Good => Grade::Good,
                    Action::Easy => Grade::Easy,
                    _ => unreachable!(),
                };
                let performance = performance.update(grade, state.today);
                state.db.update(hash, performance);
                // Was the card forgotten? Put it at the back.
                if grade == Grade::Forgot {
                    state.cards.push(card);
                }
                state.reveal = false;
            }
        }
    }
    Redirect::to("/")
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
            }
            body {
                (body)
            }
        }
    }
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
