use std::collections::HashSet;
use std::path::PathBuf;

use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use axum::routing::get;
use blake3::Hash;
use chrono::NaiveDate;
use csv::Reader;
use maud::DOCTYPE;
use maud::html;
use tokio::net::TcpListener;
use walkdir::WalkDir;

use crate::db::Database;
use crate::db::Performance;
use crate::error::Fallible;
use crate::error::fail;
use crate::parser::Card;
use crate::parser::parse_cards;

#[derive(Clone)]
pub struct ServerState {
    cards: Vec<Card>,
}

pub async fn drill_web(directory: PathBuf, today: NaiveDate) -> Fallible<()> {
    if !directory.exists() {
        return fail("directory does not exist.");
    }
    let db_path = directory.join("performance.csv");
    let mut db = if db_path.exists() {
        let mut reader = Reader::from_path(&db_path)?;
        Database::from_csv(&mut reader)?
    } else {
        Database::empty()
    };
    let mut all_cards = Vec::new();
    for entry in WalkDir::new(directory) {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "md") {
            let contents = std::fs::read_to_string(path)?;
            let cards = parse_cards(&contents);
            all_cards.extend(cards);
        }
    }
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

    let state = ServerState { cards: due_today };
    let app = Router::new();
    let app = app.route("/", get(root));
    let app = app.fallback(not_found_handler);
    let app = app.with_state(state);
    let listener = TcpListener::bind("0.0.0.0:8000").await?;
    axum::serve(listener, app).await?;
    todo!()
}

async fn root(State(state): State<ServerState>) -> (StatusCode, Html<String>) {
    let card_count = state.cards.len();
    let html = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "hashcards" }
            }
            body {
                p {
                    (format!("Cards due today: {}", card_count))
                }
            }
        }
    };
    (StatusCode::OK, Html(html.into_string()))
}

async fn not_found_handler() -> (StatusCode, Html<String>) {
    (StatusCode::OK, Html("Not Found".to_string()))
}
