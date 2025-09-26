use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use axum::routing::get;
use blake3::Hash;
use chrono::NaiveDate;
use csv::Reader;
use maud::DOCTYPE;
use maud::Markup;
use maud::html;
use tokio::net::TcpListener;
use walkdir::WalkDir;

use crate::db::Database;
use crate::db::Performance;
use crate::error::Fallible;
use crate::error::fail;
use crate::parser::Card;
use crate::parser::CardContent;
use crate::parser::parse_cards;

#[derive(Clone)]
pub struct ServerState {
    cards: Arc<Mutex<Vec<Card>>>,
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
    let mut all_cards = Vec::new();
    log::debug!("Loading deck...");
    let start = Instant::now();
    for entry in WalkDir::new(directory) {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "md") {
            let contents = std::fs::read_to_string(path)?;
            let deck_name: String = path
                .file_stem()
                .and_then(|os_str| os_str.to_str())
                .unwrap_or("None")
                .to_string();
            let cards = parse_cards(deck_name, &contents);
            all_cards.extend(cards);
        }
    }
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
        cards: Arc::new(Mutex::new(due_today)),
    };
    let app = Router::new();
    let app = app.route("/", get(root));
    let app = app.fallback(not_found_handler);
    let app = app.with_state(state);
    let listener = TcpListener::bind("0.0.0.0:8000").await?;
    axum::serve(listener, app).await?;
    todo!()
}

async fn root(State(state): State<ServerState>) -> (StatusCode, Html<String>) {
    let mut cards = state.cards.lock().unwrap();
    let body = if cards.is_empty() {
        html! {
            p { "Finished!" }
        }
    } else {
        let card = cards.remove(0);
        match &card.content {
            CardContent::Basic { question, .. } => {
                html! {
                    p {
                        "Q: " (question)
                    }
                    button {
                        "Reveal"
                    }
                }
            }
            CardContent::Cloze { text, start, end } => {
                let mut prompt = text.clone();
                prompt.replace_range(*start..*end + 1, "[...]");
                html! {
                    p {
                        "Q: " (prompt)
                    }
                    button {
                        "Reveal"
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
            }
            body {
                (body)
            }
        }
    }
}

async fn not_found_handler() -> (StatusCode, Html<String>) {
    (StatusCode::OK, Html("Not Found".to_string()))
}
