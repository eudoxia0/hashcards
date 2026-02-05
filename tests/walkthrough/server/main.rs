//! Minimal walkthrough server for generating screenshots.
//!
//! This standalone binary replicates the hashcards drill interface using
//! hashcards-core for card parsing and rendering, served via axum.
//! It does not persist any data to a database — it's purely for
//! generating walkthrough screenshots.
//!
//! IMPORTANT: The HTML templates and CSS in this file must match the
//! production server in `src/cmd/drill/`. The CSS is included directly
//! from the production file. If you change the production templates,
//! update the rendering functions here to match.

use std::collections::HashSet;
use std::env;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::{HeaderName, StatusCode};
use axum::http::header::CONTENT_TYPE;
use axum::response::{Html, Redirect};
use axum::routing::{get, post};
use axum::Router;
use hashcards_core::parser::parse_decks;
use hashcards_core::types::card::{Card, CardType};
use serde::Deserialize;
use maud::{DOCTYPE, Markup, html};
use tokio::net::TcpListener;

// ── State ──────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    mutable: Arc<Mutex<DrillState>>,
    total_cards: usize,
}

struct DrillState {
    cards: Vec<Card>,
    reveal: bool,
    reviews: Vec<String>,
    finished: bool,
}

// ── Handlers ───────────────────────────────────────────────────

async fn get_handler(State(state): State<AppState>) -> (StatusCode, Html<String>) {
    let m = state.mutable.lock().unwrap();
    let body = if m.finished {
        render_completion(&state, &m)
    } else {
        render_session(&state, &m)
    };
    (StatusCode::OK, Html(page_template(body).into_string()))
}

async fn post_handler(
    State(state): State<AppState>,
    axum::extract::Form(form): axum::extract::Form<ActionForm>,
) -> Redirect {
    {
        let mut m = state.mutable.lock().unwrap();
        match form.action.as_str() {
            "Reveal" => {
                m.reveal = true;
            }
            "Forgot" | "Hard" | "Good" | "Easy" => {
                m.reviews.push(form.action.clone());
                m.cards.remove(0);
                m.reveal = false;
                if m.cards.is_empty() {
                    m.finished = true;
                }
            }
            "Undo" => {
                if let Some(_grade) = m.reviews.pop() {
                    m.reveal = false;
                }
            }
            "End" => {
                m.finished = true;
            }
            "Shutdown" => {
                std::process::exit(0);
            }
            _ => {}
        }
    }
    Redirect::to("/")
}

#[derive(Deserialize)]
struct ActionForm {
    action: String,
}

// ── Rendering ──────────────────────────────────────────────────
//
// These functions replicate the production HTML structure from
// src/cmd/drill/get.rs. Keep them in sync.

fn render_session(state: &AppState, m: &DrillState) -> Markup {
    let undo_disabled = m.reviews.is_empty();
    let cards_done = state.total_cards - m.cards.len();
    let percent = if state.total_cards == 0 {
        100
    } else {
        (cards_done * 100) / state.total_cards
    };
    let progress_style = format!("width: {}%;", percent);
    let card = &m.cards[0];
    let card_content = render_card(card, m.reveal);
    let card_controls = if m.reveal {
        html! {
            form action="/" method="post" {
                (undo_button(undo_disabled))
                div.spacer {}
                div.grades {
                    input id="forgot" type="submit" name="action" value="Forgot" title="Mark card as forgotten. Shortcut: 1.";
                    input id="hard" type="submit" name="action" value="Hard" title="Mark card as difficult. Shortcut: 2.";
                    input id="good" type="submit" name="action" value="Good" title="Mark card as remembered well. Shortcut: 3.";
                    input id="easy" type="submit" name="action" value="Easy" title="Mark card as very easy. Shortcut: 4.";
                }
                div.spacer {}
                (end_button())
            }
        }
    } else {
        html! {
            form action="/" method="post" {
                (undo_button(undo_disabled))
                div.spacer {}
                input id="reveal" type="submit" name="action" value="Reveal" title="Show the answer. Shortcut: space.";
                div.spacer {}
                (end_button())
            }
        }
    };
    html! {
        div.root {
            div.header {
                div.progress-bar {
                    div.progress-fill style=(progress_style) {}
                }
            }
            div.card-container {
                div.card {
                    div.card-header {
                        h1 { (card.deck_name()) }
                    }
                    (card_content)
                }
            }
            div.controls {
                (card_controls)
            }
        }
    }
}

fn render_card(card: &Card, reveal: bool) -> Markup {
    let inner = match card.card_type() {
        CardType::Basic => {
            let front = card.html_front(None).unwrap_or_default();
            if reveal {
                let back = card.html_back(None).unwrap_or_default();
                html! {
                    div.question.rich-text { (maud::PreEscaped(front)) }
                    div.answer.rich-text { (maud::PreEscaped(back)) }
                }
            } else {
                html! {
                    div.question.rich-text { (maud::PreEscaped(front)) }
                    div.answer.rich-text {}
                }
            }
        }
        CardType::Cloze => {
            if reveal {
                let back = card.html_back(None).unwrap_or_default();
                html! {
                    div.prompt.rich-text { (maud::PreEscaped(back)) }
                }
            } else {
                let front = card.html_front(None).unwrap_or_default();
                html! {
                    div.prompt.rich-text { (maud::PreEscaped(front)) }
                }
            }
        }
    };
    html! {
        div.card-content {
            (inner)
        }
    }
}

fn render_completion(state: &AppState, m: &DrillState) -> Markup {
    let total_cards = state.total_cards;
    let cards_reviewed = state.total_cards - m.cards.len();
    let pace = if cards_reviewed == 0 {
        "0.00".to_string()
    } else {
        "2.50".to_string()
    };
    html! {
        div.finished {
            h1 { "Session Completed \u{1F389}" }
            div.summary {
                "Reviewed " (cards_reviewed) " cards in 25 seconds."
            }
            h2 { "Session Stats" }
            div.stats {
                table {
                    tbody {
                        tr {
                            td.key { "Total Cards" }
                            td.val { (total_cards) }
                        }
                        tr {
                            td.key { "Cards Reviewed" }
                            td.val { (cards_reviewed) }
                        }
                        tr {
                            td.key { "Started" }
                            td.val { "2025-01-01 12:00:00" }
                        }
                        tr {
                            td.key { "Finished" }
                            td.val { "2025-01-01 12:00:25" }
                        }
                        tr {
                            td.key { "Duration (seconds)" }
                            td.val { "25" }
                        }
                        tr {
                            td.key { "Pace (s/card)" }
                            td.val { (pace) }
                        }
                    }
                }
            }
            div.shutdown-container {
                form action="/" method="post" {
                    input #shutdown .shutdown-button type="submit" name="action" value="Shutdown" title="Shut down the server";
                }
            }
        }
    }
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

// ── Template ───────────────────────────────────────────────────
//
// Matches the production template in src/cmd/drill/template.rs.

const KATEX_CSS_URL: &str = "/katex/katex.css";
const KATEX_JS_URL: &str = "/katex/katex.js";
const KATEX_MHCHEM_JS_URL: &str = "/katex/mhchem.js";
const HIGHLIGHT_JS_URL: &str =
    "https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/highlight.min.js";
const HIGHLIGHT_CSS_URL: &str =
    "https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/styles/github.min.css";

fn page_template(body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "hashcards" }
                link rel="stylesheet" href=(KATEX_CSS_URL);
                link rel="stylesheet" href=(HIGHLIGHT_CSS_URL);
                script defer src=(KATEX_JS_URL) {};
                script defer src=(KATEX_MHCHEM_JS_URL) {};
                script defer src=(HIGHLIGHT_JS_URL) {};
                link rel="stylesheet" href="/style.css";
                style { ".card-content { opacity: 0; }" }
                noscript { style { ".card-content { opacity: 1; }" } }
            }
            body {
                (body)
                script src="/script.js" {};
            }
        }
    }
}

// ── Static assets ──────────────────────────────────────────────
//
// CSS is included directly from the production source file to ensure
// screenshots reflect the actual look and feel.

async fn style_handler() -> (StatusCode, [(HeaderName, &'static str); 1], &'static str) {
    (StatusCode::OK, [(CONTENT_TYPE, "text/css")], include_str!("../../../src/cmd/drill/style.css"))
}

async fn script_handler() -> (StatusCode, [(HeaderName, &'static str); 1], String) {
    let mut content = String::new();
    content.push_str("let MACROS = {};\n\n");
    content.push_str(include_str!("../../../src/cmd/drill/script.js"));
    (StatusCode::OK, [(CONTENT_TYPE, "text/javascript")], content)
}

async fn katex_css_handler() -> (StatusCode, [(HeaderName, &'static str); 1], &'static str) {
    (StatusCode::OK, [(CONTENT_TYPE, "text/css")], include_str!("../../../vendor/katex/katex.min.css"))
}

async fn katex_js_handler() -> (StatusCode, [(HeaderName, &'static str); 1], &'static str) {
    (StatusCode::OK, [(CONTENT_TYPE, "text/javascript")], include_str!("../../../vendor/katex/katex.min.js"))
}

async fn katex_mhchem_js_handler() -> (StatusCode, [(HeaderName, &'static str); 1], &'static str) {
    (StatusCode::OK, [(CONTENT_TYPE, "text/javascript")], include_str!("../../../vendor/katex/mhchem.min.js"))
}

async fn katex_font_handler(
    axum::extract::Path(filename): axum::extract::Path<String>,
) -> impl axum::response::IntoResponse {
    let vendor_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../vendor/katex/fonts");
    let path = vendor_dir.join(&filename);
    if !path.exists() || !filename.ends_with(".woff2") {
        return (StatusCode::NOT_FOUND, [("content-type", "text/plain")], Vec::new());
    }
    let bytes = std::fs::read(&path).unwrap_or_default();
    (StatusCode::OK, [("content-type", "font/woff2")], bytes)
}

// ── Main ───────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let collection_dir = args.get(1).map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../collection")
    });
    let port: u16 = args
        .get(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(8000);

    // Collect all markdown files
    let mut files: Vec<(String, String)> = Vec::new();
    for entry in walkdir::WalkDir::new(&collection_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "md") {
            let content = std::fs::read_to_string(path).expect("Failed to read deck file");
            let filename = path.file_name().unwrap().to_string_lossy().to_string();
            files.push((filename, content));
        }
    }

    // Parse all decks
    let all_cards: Vec<Card> = parse_decks(
        files.iter().map(|(name, content)| (name.as_str(), content.as_str()))
    ).expect("Failed to parse decks");

    if all_cards.is_empty() {
        eprintln!("No cards found in {}", collection_dir.display());
        std::process::exit(1);
    }

    let total = all_cards.len();
    let deck_count = {
        let mut names: HashSet<&str> = HashSet::new();
        for card in &all_cards {
            names.insert(card.deck_name());
        }
        names.len()
    };
    eprintln!("Loaded {} cards from {} decks in {}", total, deck_count, collection_dir.display());

    let state = AppState {
        total_cards: total,
        mutable: Arc::new(Mutex::new(DrillState {
            cards: all_cards,
            reveal: false,
            reviews: Vec::new(),
            finished: false,
        })),
    };

    let app = Router::new()
        .route("/", get(get_handler))
        .route("/", post(post_handler))
        .route("/style.css", get(style_handler))
        .route("/script.js", get(script_handler))
        .route(KATEX_CSS_URL, get(katex_css_handler))
        .route(KATEX_JS_URL, get(katex_js_handler))
        .route(KATEX_MHCHEM_JS_URL, get(katex_mhchem_js_handler))
        .route("/katex/fonts/{filename}", get(katex_font_handler))
        .with_state(state);

    let bind = format!("127.0.0.1:{port}");
    eprintln!("Listening on http://{bind}");
    let listener = TcpListener::bind(bind).await.expect("Failed to bind");
    axum::serve(listener, app).await.expect("Server error");
}
