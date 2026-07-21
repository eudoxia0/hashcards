// Copyright 2025–2026 Fernando Borretti
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
use std::fmt::Display;
use std::fmt::Formatter;
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
use clap::ValueEnum;
use tokio::net::TcpListener;
use tokio::select;
use tokio::signal;
use tokio::sync::oneshot::Receiver;
use tokio::sync::oneshot::channel;

use crate::cmd::drill::cache::Cache;
use crate::cmd::drill::get::get_handler;
use crate::cmd::drill::highlight::HIGHLIGHT_CSS_URL;
use crate::cmd::drill::highlight::HIGHLIGHT_JS_URL;
use crate::cmd::drill::highlight::highlight_css_handler;
use crate::cmd::drill::highlight::highlight_js_handler;
use crate::cmd::drill::katex::KATEX_CSS_URL;
use crate::cmd::drill::katex::KATEX_JS_URL;
use crate::cmd::drill::katex::KATEX_MHCHEM_JS_URL;
use crate::cmd::drill::katex::katex_css_handler;
use crate::cmd::drill::katex::katex_font_handler;
use crate::cmd::drill::katex::katex_js_handler;
use crate::cmd::drill::katex::katex_mhchem_js_handler;
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

#[derive(ValueEnum, Clone, Copy, PartialEq)]
pub enum AnswerControls {
    /// Show all four rating buttons (Forgot/Hard/Good/Easy).
    Full,
    /// Show only two rating buttons (Forgot/Good).
    Binary,
}

impl Display for AnswerControls {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AnswerControls::Full => write!(f, "full"),
            AnswerControls::Binary => write!(f, "binary"),
        }
    }
}

pub struct ServerConfig {
    pub directory: Option<String>,
    pub host: String,
    pub resource_hostname: String,
    pub port: u16,
    pub session_started_at: Timestamp,
    pub card_limit: Option<usize>,
    pub new_card_limit: Option<usize>,
    pub deck_filter: Option<String>,
    pub shuffle: bool,
    pub answer_controls: AnswerControls,
    pub bury_siblings: bool,
}

pub async fn start_server(config: ServerConfig) -> Fallible<()> {
    let Collection {
        directory,
        db,
        cards,
        macros,
    } = Collection::new(config.directory.clone())?;

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
    let due_today: HashSet<CardHash> = db.due_today(today)?;
    let due_today: Vec<Card> = cards
        .into_iter()
        .filter(|card| due_today.contains(&card.hash()))
        .collect::<Vec<_>>();

    let due_today: Vec<Card> = filter_deck(&db, due_today, &config)?;

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
        resource_hostname: config.resource_hostname,
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
        answer_controls: config.answer_controls,
    };
    let app = Router::new();

    let app = app.route("/", get(get_handler));
    let app = app.route("/", post(post_handler));
    let app = app.route("/favicon.ico", get(favicon_handler));
    let app = app.route("/file/{*path}", get(file_handler));
    let app = app.route("/katex/fonts/{*path}", get(katex_font_handler));
    let app = app.route("/script.js", get(script_handler));
    let app = app.route("/common.css", get(common_css_handler));
    let app = app.route("/drill.css", get(drill_css_handler));
    let app = app.route("/finished.css", get(finished_css_handler));
    let app = app.route(HIGHLIGHT_CSS_URL, get(highlight_css_handler));
    let app = app.route(HIGHLIGHT_JS_URL, get(highlight_js_handler));
    let app = app.route(KATEX_CSS_URL, get(katex_css_handler));
    let app = app.route(KATEX_JS_URL, get(katex_js_handler));
    let app = app.route(KATEX_MHCHEM_JS_URL, get(katex_mhchem_js_handler));

    let app = app.fallback(not_found_handler);
    let app = app.with_state(state.clone());
    let bind = format!("{}:{}", config.host, config.port);

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
        let name = escape_js_string_literal(name);
        let definition = escape_js_string_literal(definition);
        content.push_str(&format!("MACROS['{name}'] = '{definition}';\n"));
    }
    content.push_str("MACROS[','] = '{\\\\char`,}';\n");
    content.push('\n');
    content.push_str(include_str!("script.js"));
    (StatusCode::OK, [(CONTENT_TYPE, "text/javascript")], content)
}

fn escape_js_string_literal(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace('$', "\\$")
}

fn css_response(
    bytes: &'static [u8],
) -> (StatusCode, [(HeaderName, &'static str); 2], &'static [u8]) {
    (
        StatusCode::OK,
        [
            (CONTENT_TYPE, "text/css"),
            (CACHE_CONTROL, CACHE_CONTROL_IMMUTABLE),
        ],
        bytes,
    )
}

async fn common_css_handler() -> (StatusCode, [(HeaderName, &'static str); 2], &'static [u8]) {
    css_response(include_bytes!("common.css"))
}

async fn drill_css_handler() -> (StatusCode, [(HeaderName, &'static str); 2], &'static [u8]) {
    css_response(include_bytes!("drill.css"))
}

async fn finished_css_handler() -> (StatusCode, [(HeaderName, &'static str); 2], &'static [u8]) {
    css_response(include_bytes!("finished.css"))
}

async fn favicon_handler() -> (StatusCode, [(HeaderName, &'static str); 2], &'static [u8]) {
    let bytes = include_bytes!("favicon.png");
    (
        StatusCode::OK,
        [
            (CONTENT_TYPE, "image/png"),
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

fn filter_deck(db: &Database, deck: Vec<Card>, config: &ServerConfig) -> Fallible<Vec<Card>> {
    // Apply the deck filter.
    let deck = match &config.deck_filter {
        Some(filter) => deck
            .into_iter()
            .filter(|card| card.deck_name() == filter)
            .collect(),
        None => deck,
    };

    // Apply the new card limit.
    let deck: Vec<Card> = match config.new_card_limit {
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

    let deck = if config.bury_siblings {
        bury_siblings(deck)
    } else {
        deck
    };

    // Apply the card limit.
    let deck = match config.card_limit {
        Some(limit) => deck.into_iter().take(limit).collect(),
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use crate::cmd::drill::server::AnswerControls;
    use crate::cmd::drill::server::ServerConfig;
    use crate::cmd::drill::server::filter_deck;
    use crate::db::Database;
    use crate::error::Fallible;
    use crate::types::card::Card;
    use crate::types::card::CardContent;
    use crate::types::card_hash::CardHash;
    use crate::types::date::Date;
    use crate::types::performance::Performance;
    use crate::types::performance::ReviewedPerformance;
    use crate::types::timestamp::Timestamp;

    const ANSWER: &str = "answer";

    fn insert_card(db: &Database, card: &Card, is_new: bool) -> Fallible<()> {
        let now = Timestamp::now();
        db.insert_card(card.hash(), now)?;
        db.update_card_performance(
            card.hash(),
            if is_new {
                Performance::New
            } else {
                Performance::Reviewed(ReviewedPerformance {
                    last_reviewed_at: now,
                    stability: 1.0,
                    difficulty: 1.0,
                    interval_raw: 1.0,
                    interval_days: 1,
                    due_date: Date::today(),
                    review_count: 1,
                })
            },
        )?;
        Ok(())
    }

    fn base_config() -> ServerConfig {
        ServerConfig {
            directory: None,
            host: "127.0.0.1".to_string(),
            resource_hostname: "localhost".to_string(),
            port: 0,
            session_started_at: Timestamp::now(),
            card_limit: None,
            new_card_limit: None,
            deck_filter: None,
            shuffle: false,
            answer_controls: AnswerControls::Full,
            bury_siblings: false,
        }
    }

    /// check `filter_deck` behaviour across a range of decks.
    #[test]
    fn test_filter_deck() -> Fallible<()> {
        struct CaseCard {
            label: String,
            card: Card,
            is_new: bool,
        }

        fn card(label: &'static str, deck: &'static str, is_new: bool) -> CaseCard {
            CaseCard {
                label: label.to_string(),
                card: Card::new(
                    deck.to_string(),
                    PathBuf::from("test.md"),
                    (0, 0),
                    CardContent::new_basic(label, ANSWER),
                ),
                is_new,
            }
        }

        // we can't reuse label for card content for cloze cards, because we need to find right
        // card by label in HashMap later
        fn cloze_card(
            label: &str,
            prompt: &str,
            start: usize,
            deck: &str,
            is_new: bool,
        ) -> CaseCard {
            CaseCard {
                label: label.to_string(),
                card: Card::new(
                    deck.to_string(),
                    PathBuf::from("test.md"),
                    (0, 0),
                    CardContent::new_cloze(prompt, start, start + 1),
                ),
                is_new,
            }
        }

        struct Case {
            name: &'static str,
            cards: Vec<CaseCard>,
            card_limit: Option<usize>,
            new_card_limit: Option<usize>,
            deck_filter: Option<String>,
            bury_siblings: bool,
            /// Labels of the cards expected to survive filtering.
            expected: &'static [&'static str],
        }

        let cases = [
            Case {
                name: "pull all cards",
                cards: vec![
                    card("first", "first", false),
                    card("second", "second", false),
                    card("new", "first", true),
                    cloze_card("cloze", "cloze", 0, "first", false),
                ],
                card_limit: None,
                new_card_limit: None,
                deck_filter: None,
                bury_siblings: false,
                expected: &["first", "second", "new", "cloze"],
            },
            Case {
                name: "pull all cards from specific deck",
                cards: vec![
                    card("first", "first", false),
                    card("second", "second", false),
                    card("new", "first", true),
                ],
                card_limit: None,
                new_card_limit: None,
                deck_filter: Some("first".to_string()),
                bury_siblings: false,
                expected: &["first", "new"],
            },
            Case {
                name: "pull N cards",
                cards: vec![
                    card("first", "first", false),
                    card("second", "second", false),
                    card("third", "third", false),
                ],
                card_limit: Some(2),
                new_card_limit: None,
                deck_filter: None,
                bury_siblings: false,
                expected: &["first", "second"],
            },
            Case {
                name: "pull N new cards",
                cards: vec![
                    card("first", "first", false),
                    card("second", "second", false),
                    card("new", "first", true),
                    card("new2", "first", true),
                    card("new3", "first", true),
                ],
                card_limit: None,
                new_card_limit: Some(2),
                deck_filter: None,
                bury_siblings: false,
                expected: &["first", "second", "new", "new2"],
            },
            Case {
                name: "bury all siblings",
                cards: vec![
                    cloze_card("cloze0", "cloze", 0, "first", false),
                    cloze_card("cloze1", "cloze", 1, "first", false),
                    cloze_card("cloze2", "cloze", 2, "first", false),
                    cloze_card("cloze3", "cloze", 3, "first", false),
                ],
                card_limit: None,
                new_card_limit: None,
                deck_filter: None,
                bury_siblings: true,
                expected: &["cloze0"],
            },
            Case {
                name: "don't bury siblings",
                cards: vec![
                    cloze_card("cloze0", "cloze", 0, "first", false),
                    cloze_card("cloze1", "cloze", 1, "first", false),
                    cloze_card("cloze2", "cloze", 2, "first", false),
                    cloze_card("cloze3", "cloze", 3, "first", false),
                ],
                card_limit: None,
                new_card_limit: None,
                deck_filter: None,
                bury_siblings: false,
                expected: &["cloze0", "cloze1", "cloze2", "cloze3"],
            },
            Case {
                name: "pull N new cards with card limit",
                cards: vec![
                    card("first", "first", false),
                    card("second", "second", false),
                    card("new", "first", true),
                    card("new2", "first", true),
                    card("new3", "first", true),
                    card("third", "third", false),
                    card("fourth", "fourth", false),
                ],
                card_limit: Some(5),
                new_card_limit: Some(2),
                deck_filter: None,
                bury_siblings: false,
                expected: &["first", "second", "new", "new2", "third"],
            },
            Case {
                name: "pull N cards with bury siblings",
                cards: vec![
                    card("first", "first", false),
                    card("second", "second", false),
                    cloze_card("cloze0", "cloze", 0, "first", false),
                    cloze_card("cloze1", "cloze", 1, "first", false),
                    cloze_card("cloze2", "cloze", 2, "first", false),
                    card("third", "third", false),
                    card("new", "first", true),
                    card("new2", "first", true),
                ],
                card_limit: Some(5),
                new_card_limit: None,
                deck_filter: None,
                bury_siblings: true,
                expected: &["first", "second", "cloze0", "third", "new"],
            },
        ];

        for case in cases {
            let db = Database::new(":memory:")?;

            let deck: Vec<Card> = case
                .cards
                .iter()
                .map(|case_card| {
                    insert_card(&db, &case_card.card, case_card.is_new)?;
                    Ok(case_card.card.clone())
                })
                .collect::<Fallible<_>>()?;

            let config = ServerConfig {
                card_limit: case.card_limit,
                deck_filter: case.deck_filter,
                new_card_limit: case.new_card_limit,
                bury_siblings: case.bury_siblings,
                ..base_config()
            };
            let result = filter_deck(&db, deck, &config)?;

            let got: Vec<CardHash> = result.iter().map(|card| card.hash()).collect();

            let by_label: HashMap<String, CardHash> = case
                .cards
                .iter()
                .map(|cc| (cc.label.clone(), cc.card.hash()))
                .collect();

            let want: Vec<CardHash> = case
                .expected
                .iter()
                .map(|l| by_label[&l.to_string()])
                .collect();

            assert_eq!(got, want, "case: {}", case.name);
        }
        Ok(())
    }
}
