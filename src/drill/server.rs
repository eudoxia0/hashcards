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

use axum::Router;
use axum::extract::Path;
use axum::extract::State;
use axum::http::HeaderName;
use axum::http::StatusCode;
use axum::http::header::CACHE_CONTROL;
use axum::http::header::CONTENT_TYPE;
use axum::response::Html;
use axum::routing::get;
use axum::routing::post;
use tokio::net::TcpListener;

use crate::db::Database;
use crate::db::Stage;
use crate::drill::get::get_handler;
use crate::drill::post::post_handler;
use crate::drill::state::MutableState;
use crate::drill::state::ServerState;
use crate::error::ErrorReport;
use crate::error::Fallible;
use crate::error::fail;
use crate::parser::parse_deck;
use crate::types::card::Card;
use crate::types::hash::Hash;
use crate::types::timestamp::Timestamp;

pub async fn start_server(
    directory: PathBuf,
    session_started_at: Timestamp,
    card_limit: Option<usize>,
    new_card_limit: Option<usize>,
) -> Fallible<()> {
    let today = session_started_at.local_date();

    if !directory.exists() {
        return fail("directory does not exist.");
    }

    let db_path = directory.join("db.sqlite3");
    let db = Database::new(
        db_path
            .to_str()
            .ok_or_else(|| ErrorReport::new("invalid path"))?,
    )?;

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
    let all_cards = parse_deck(&directory)?;
    let end = Instant::now();
    let duration = end.duration_since(start).as_millis();
    log::debug!("Deck loaded in {duration}ms.");

    let db_hashes: HashSet<Hash> = db.card_hashes()?;

    // If a card is in the directory, but not in the DB, it is new. Add it to
    // the database.
    for card in all_cards.iter() {
        if !db_hashes.contains(&card.hash()) {
            db.add_card(card, session_started_at)?;
        }
    }

    // Find cards due today.
    let due_today = db.due_today(today)?;
    let due_today: Vec<Card> = all_cards
        .into_iter()
        .filter(|card| due_today.contains(&card.hash()))
        .collect::<Vec<_>>();

    let due_today = filter_deck(&db, due_today, card_limit, new_card_limit)?;

    if due_today.is_empty() {
        println!("No cards due today.");
        return Ok(());
    }

    let state = ServerState {
        directory,
        macros,
        total_cards: due_today.len(),
        session_started_at,
        mutable: Arc::new(Mutex::new(MutableState {
            reveal: false,
            db,
            cards: due_today,
            reviews: Vec::new(),
            finished_at: None,
        })),
    };
    let app = Router::new();
    let app = app.route("/", get(get_handler));
    let app = app.route("/", post(post_handler));
    let app = app.route("/script.js", get(script_handler));
    let app = app.route("/style.css", get(style_handler));
    let app = app.route("/image/{*path}", get(image_handler));
    let app = app.fallback(not_found_handler);
    let app = app.with_state(state);
    let bind = "0.0.0.0:8000";

    // Start the server.
    log::debug!("Starting server on {bind}");
    let listener = TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn script_handler(
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

async fn style_handler() -> (StatusCode, [(HeaderName, &'static str); 2], &'static [u8]) {
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
    (StatusCode::NOT_FOUND, Html("Not Found".to_string()))
}

async fn image_handler(
    State(state): State<ServerState>,
    Path(path): Path<String>,
) -> (StatusCode, [(HeaderName, &'static str); 1], Vec<u8>) {
    let path = PathBuf::from(path);
    let path = state.directory.join(path);
    if !path.exists() {
        return (
            StatusCode::NOT_FOUND,
            [(CONTENT_TYPE, "text/plain")],
            b"Not Found".to_vec(),
        );
    }
    let content = tokio::fs::read(path).await;
    match content {
        Ok(bytes) => (
            StatusCode::OK,
            [(CONTENT_TYPE, "application/octet-stream")],
            bytes,
        ),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(CONTENT_TYPE, "text/plain")],
            b"Internal Server Error".to_vec(),
        ),
    }
}

fn filter_deck(
    db: &Database,
    deck: Vec<Card>,
    card_limit: Option<usize>,
    new_card_limit: Option<usize>,
) -> Fallible<Vec<Card>> {
    // Apply the card limit.
    let deck = match card_limit {
        Some(limit) => deck.into_iter().take(limit).collect(),
        None => deck,
    };

    // Apply the new card limit.
    let deck = match new_card_limit {
        Some(limit) => {
            let mut new_count = 0;
            let mut result = Vec::new();
            for card in deck.into_iter() {
                if db.get_card_stage(card.hash())? == Stage::New {
                    if new_count < limit {
                        result.push(card);
                        new_count += 1;
                    }
                } else {
                    result.push(card);
                }
            }
            result
        }
        None => deck,
    };

    Ok(deck)
}
