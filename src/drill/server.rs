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
use std::time::Duration;
use std::time::Instant;

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
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::time::sleep;

use crate::db::Database;
use crate::db::Performance;
use crate::drill::state::MutableState;
use crate::drill::state::ServerState;
use crate::drill::view::action;
use crate::drill::view::root;
use crate::error::Fallible;
use crate::error::fail;
use crate::hash::Hash;
use crate::parser::Card;
use crate::parser::parse_deck;

pub async fn start_server(directory: PathBuf, today: NaiveDate) -> Fallible<()> {
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
        db_path,
        macros,
        mutable: Arc::new(Mutex::new(MutableState {
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

    // Start a separate task to open the browser.
    let url = format!("http://{bind}/");
    tokio::spawn(async move {
        loop {
            if let Ok(stream) = TcpStream::connect(bind).await {
                drop(stream);
                break;
            }
            sleep(Duration::from_millis(1)).await;
        }
        let _ = open::that(url);
    });

    // Start the server.
    log::debug!("Starting server on {bind}");
    let listener = TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn script(
    State(state): State<ServerState>,
) -> (StatusCode, [(HeaderName, &'static str); 1], String) {
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
