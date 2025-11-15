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
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

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
use tokio::select;
use tokio::signal;
use tokio::sync::oneshot::Receiver;
use tokio::sync::oneshot::channel;

use crate::cmd::drill::cache::Cache;
use crate::cmd::drill::get::get_handler;
use crate::cmd::drill::katex::KATEX_AUTO_RENDER_JS_URL;
use crate::cmd::drill::katex::KATEX_CSS_URL;
use crate::cmd::drill::katex::KATEX_JS_URL;
use crate::cmd::drill::katex::katex_auto_render_handler;
use crate::cmd::drill::katex::katex_css_handler;
use crate::cmd::drill::katex::katex_font_handler;
use crate::cmd::drill::katex::katex_js_handler;
use crate::cmd::drill::post::post_handler;
use crate::cmd::drill::state::MutableState;
use crate::cmd::drill::state::ServerState;
use crate::collection::Collection;
use crate::db::Database;
use crate::error::Fallible;
use crate::error::fail;
use crate::media::load::MediaLoader;
use crate::rng::TinyRng;
use crate::rng::shuffle;
use crate::types::card::Card;
use crate::types::card_hash::CardHash;
use crate::types::date::Date;
use crate::types::timestamp::Timestamp;
use crate::utils::CACHE_CONTROL_IMMUTABLE;

pub struct ServerConfig {
    pub directory: Option<String>,
    pub port: u16,
    pub session_started_at: Timestamp,
    pub card_limit: Option<usize>,
    pub new_card_limit: Option<usize>,
    pub deck_filter: Option<String>,
    pub shuffle: bool,
}

pub async fn start_server(config: ServerConfig) -> Fallible<()> {
    let Collection {
        directory,
        db,
        cards,
        macros,
    } = Collection::new(config.directory)?;

    let today: Date = config.session_started_at.date();

    let db_hashes: HashSet<CardHash> = db.card_hashes()?;
    // If a card is in the directory, but not in the DB, it is new. Add it to
    // the database.
    for card in cards.iter() {
        if !db_hashes.contains(&card.hash()) {
            db.insert_card(card.hash(), config.session_started_at)?;
        }
    }

    // Find cards due today.
    let due_today = db.due_today(today)?;
    let due_today: Vec<Card> = cards
        .into_iter()
        .filter(|card| due_today.contains(&card.hash()))
        .collect::<Vec<_>>();

    let due_today = filter_deck(
        &db,
        due_today,
        config.card_limit,
        config.new_card_limit,
        config.deck_filter,
    )?;

    if due_today.is_empty() {
        println!("No cards due today.");
        return Ok(());
    }

    // Finally, shuffle the cards.
    let due_today: Vec<Card> = if config.shuffle {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        let mut rng = TinyRng::from_seed(seed);
        shuffle(due_today, &mut rng)
    } else {
        due_today
    };

    // For all cards due today, fetch their performance from the database and store it in the cache.
    let mut cache = Cache::new();
    for card in due_today.iter() {
        let performance = db.get_card_performance(card.hash())?;
        cache.insert(card.hash(), performance)?;
    }

    // Create shutdown channel
    let (shutdown_tx, shutdown_rx) = channel();

    let state = ServerState {
        port: config.port,
        directory,
        macros,
        total_cards: due_today.len(),
        session_started_at: config.session_started_at,
        mutable: Arc::new(Mutex::new(MutableState {
            reveal: false,
            db,
            cache,
            cards: due_today,
            reviews: Vec::new(),
            finished_at: None,
        })),
        shutdown_tx: Arc::new(Mutex::new(Some(shutdown_tx))),
    };
    let app = Router::new();
    let app = app.route("/", get(get_handler));
    let app = app.route("/", post(post_handler));
    let app = app.route("/script.js", get(script_handler));
    let app = app.route("/style.css", get(style_handler));
    let app = app.route(KATEX_CSS_URL, get(katex_css_handler));
    let app = app.route(KATEX_JS_URL, get(katex_js_handler));
    let app = app.route(KATEX_AUTO_RENDER_JS_URL, get(katex_auto_render_handler));
    let app = app.route("/katex/fonts/{*path}", get(katex_font_handler));
    let app = app.route("/file/{*path}", get(file_handler));
    let app = app.fallback(not_found_handler);
    let app = app.with_state(state.clone());
    let bind = format!("0.0.0.0:{}", config.port);

    // Start the server with graceful shutdown on Ctrl+C or shutdown button.
    log::debug!("Starting server on {bind}");
    let listener = TcpListener::bind(bind).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(shutdown_rx))
        .await?;

    // Check if session was complete when server shut down
    let mutable = state.mutable.lock().unwrap();
    if mutable.finished_at.is_some() {
        // Session was complete, exit with code 0
        Ok(())
    } else {
        // Session was not complete, exit with error code
        fail("Session interrupted before completion")
    }
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
            (CACHE_CONTROL, CACHE_CONTROL_IMMUTABLE),
        ],
        bytes,
    )
}

async fn not_found_handler() -> (StatusCode, Html<String>) {
    (StatusCode::NOT_FOUND, Html("Not Found".to_string()))
}

async fn file_handler(
    State(state): State<ServerState>,
    Path(path): Path<String>,
) -> (StatusCode, [(HeaderName, &'static str); 1], Vec<u8>) {
    let loader = MediaLoader::new(state.directory.clone());
    let validated_path: PathBuf = match loader.validate(&path) {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                [(CONTENT_TYPE, "text/plain")],
                b"Not Found".to_vec(),
            );
        }
    };
    let extension = validated_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_lowercase();
    let content_type: &str = match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    };
    let content = tokio::fs::read(validated_path).await;
    match content {
        Ok(bytes) => (StatusCode::OK, [(CONTENT_TYPE, content_type)], bytes),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(CONTENT_TYPE, "text/plain")],
            b"Internal Server Error".to_vec(),
        ),
    }
}

async fn shutdown_signal(shutdown_rx: Receiver<()>) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    let shutdown = async {
        shutdown_rx.await.ok();
    };

    select! {
        _ = ctrl_c => {
            log::debug!("Received Ctrl+C, shutting down gracefully");
        },
        _ = shutdown => {
            log::debug!("Received shutdown signal, shutting down gracefully");
        },
    }
}

fn filter_deck(
    db: &Database,
    deck: Vec<Card>,
    card_limit: Option<usize>,
    new_card_limit: Option<usize>,
    deck_filter: Option<String>,
) -> Fallible<Vec<Card>> {
    // Apply the deck filter.
    let deck = match deck_filter {
        Some(filter) => deck
            .into_iter()
            .filter(|card| card.deck_name() == &filter)
            .collect(),
        None => deck,
    };

    // Bury sibling cards.
    let deck = bury_siblings(deck);

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
                if db.get_card_performance(card.hash())?.is_new() {
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

fn bury_siblings(deck: Vec<Card>) -> Vec<Card> {
    let mut seen_families = HashSet::new();
    let mut result = Vec::new();
    for card in deck.into_iter() {
        if let Some(family) = card.family_hash() {
            if seen_families.contains(&family) {
                continue;
            }
            seen_families.insert(family);
        }
        result.push(card);
    }
    result
}
