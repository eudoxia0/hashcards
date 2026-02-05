//! Minimal walkthrough server for generating screenshots.
//!
//! This standalone binary replicates the hashcards drill interface using
//! hashcards-core for card parsing and rendering, served via axum.
//! It does not persist any data to a database — it's purely for
//! generating walkthrough screenshots.

use std::collections::HashSet;
use std::env;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::{HeaderName, StatusCode};
use axum::http::header::CONTENT_TYPE;
use axum::response::Html;
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
    deck_count: usize,
}

struct DrillState {
    cards: Vec<Card>,
    reveal: bool,
    reviews: Vec<String>,
    finished: bool,
    started: bool,
}

// ── Handlers ───────────────────────────────────────────────────

async fn get_handler(State(state): State<AppState>) -> (StatusCode, Html<String>) {
    let m = state.mutable.lock().unwrap();
    let body = if !m.started {
        render_start(&state)
    } else if m.finished {
        render_completion(&state, &m)
    } else {
        render_session(&state, &m)
    };
    (StatusCode::OK, Html(page_template(body).into_string()))
}

async fn post_handler(
    State(state): State<AppState>,
    axum::extract::Form(form): axum::extract::Form<ActionForm>,
) -> (StatusCode, Html<String>) {
    {
        let mut m = state.mutable.lock().unwrap();
        match form.action.as_str() {
            "Start" => {
                m.started = true;
            }
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
                if let Some(grade) = m.reviews.pop() {
                    // For the walkthrough we can't truly undo, but we can
                    // pretend by re-adding a dummy card.
                    // Actually, let's keep it simple: just reset reveal state.
                    // The real implementation would restore the card; since we
                    // removed it we can't. But for screenshot purposes the page
                    // will re-render correctly with whatever card is current.
                    m.reveal = false;
                    let _ = grade;
                }
            }
            "End" => {
                m.finished = true;
            }
            "Finish" => {
                std::process::exit(0);
            }
            _ => {}
        }
    }
    get_handler(State(state)).await
}

#[derive(Deserialize)]
struct ActionForm {
    action: String,
}

// ── Rendering ──────────────────────────────────────────────────

fn render_start(state: &AppState) -> Markup {
    html! {
        div.start-screen {
            h1 { "hashcards" }
            p.subtitle {
                (state.total_cards) " cards in " (state.deck_count)
                @if state.deck_count == 1 { " deck" } @else { " decks" }
            }
            form action="/" method="post" {
                input #start .start-button type="submit" name="action" value="Start";
            }
        }
    }
}

fn render_session(state: &AppState, m: &DrillState) -> Markup {
    let has_undo = !m.reviews.is_empty();
    let cards_done = state.total_cards - m.cards.len();
    let percent = if state.total_cards == 0 {
        100
    } else {
        (cards_done * 100) / state.total_cards
    };
    let progress_style = format!("width: {}%;", percent);
    let card = &m.cards[0];
    let card_content = render_card(card, m.reveal);
    let controls = if m.reveal {
        html! {
            form action="/" method="post" {
                div.grades {
                    input id="forgot" type="submit" name="action" value="Forgot" title="Mark card as forgotten. Shortcut: 1.";
                    input id="hard" type="submit" name="action" value="Hard" title="Mark card as difficult. Shortcut: 2.";
                    input id="good" type="submit" name="action" value="Good" title="Mark card as remembered well. Shortcut: 3.";
                    input id="easy" type="submit" name="action" value="Easy" title="Mark card as very easy. Shortcut: 4.";
                }
            }
        }
    } else {
        html! {
            form action="/" method="post" {
                input id="reveal" type="submit" name="action" value="Reveal" title="Show the answer. Shortcut: space.";
            }
        }
    };
    html! {
        div.root {
            div.header {
                @if has_undo {
                    form.header-action action="/" method="post" {
                        input id="undo" type="submit" name="action" value="Undo" title="Undo last action. Shortcut: u.";
                    }
                } @else {
                    div.header-placeholder {}
                }
                div.progress-bar {
                    div.progress-fill style=(progress_style) {}
                }
                form.header-action action="/" method="post" {
                    input id="end" type="submit" name="action" value="End" title="End the session";
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
                (controls)
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
    let reviewed = state.total_cards - m.cards.len();
    html! {
        div.finished {
            h1 { "Session Completed 🎉" }
            div.summary {
                "Reviewed " (reviewed) " cards."
            }
            h2 { "Session Stats" }
            div.stats {
                table {
                    tbody {
                        tr {
                            td.key { "Total Cards" }
                            td.val { (state.total_cards) }
                        }
                        tr {
                            td.key { "Cards Reviewed" }
                            td.val { (reviewed) }
                        }
                        tr {
                            td.key { "Pace (s/card)" }
                            td.val { "2.50" }
                        }
                    }
                }
            }
            div.finish-container {
                form action="/" method="post" {
                    input #finish .finish-button type="submit" name="action" value="Finish" title="Close the session";
                }
            }
        }
    }
}

// ── Template ───────────────────────────────────────────────────

const HIGHLIGHT_JS: &str = "https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/highlight.min.js";
const HIGHLIGHT_CSS: &str = "https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/styles/github.min.css";
const GARAMOND_CSS: &str = "https://fonts.googleapis.com/css2?family=EB+Garamond:ital,wght@0,400;0,500;0,600;1,400;1,500;1,600&display=swap";

fn page_template(body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "hashcards" }
                meta name="color-scheme" content="light dark";
                link rel="preconnect" href="https://fonts.googleapis.com";
                link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous";
                link rel="stylesheet" href=(GARAMOND_CSS);
                link rel="stylesheet" href="/vendor/katex/katex.min.css";
                link rel="stylesheet" href=(HIGHLIGHT_CSS);
                script defer src="/vendor/katex/katex.min.js" {};
                script defer src=(HIGHLIGHT_JS) {};
                link rel="stylesheet" href="/style.css";
            }
            body {
                (body)
                script src="/script.js" {};
            }
        }
    }
}

// ── Static assets ──────────────────────────────────────────────

async fn style_handler() -> (StatusCode, [(HeaderName, &'static str); 1], &'static str) {
    (StatusCode::OK, [(CONTENT_TYPE, "text/css")], include_str!("style.css"))
}

async fn script_handler() -> (StatusCode, [(HeaderName, &'static str); 1], String) {
    let mut content = String::new();
    content.push_str("let MACROS = {};\n\n");
    content.push_str(include_str!("../../../src/cmd/drill/script.js"));
    (StatusCode::OK, [(CONTENT_TYPE, "text/javascript")], content)
}

async fn katex_css_handler() -> (StatusCode, [(HeaderName, &'static str); 1], &'static str) {
    (StatusCode::OK, [(CONTENT_TYPE, "text/css")], include_str!("vendor/katex/katex.min.css"))
}

async fn katex_js_handler() -> (StatusCode, [(HeaderName, &'static str); 1], &'static str) {
    (StatusCode::OK, [(CONTENT_TYPE, "text/javascript")], include_str!("vendor/katex/katex.min.js"))
}

async fn katex_font_handler(
    axum::extract::Path(filename): axum::extract::Path<String>,
) -> impl axum::response::IntoResponse {
    let vendor_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/katex/fonts");
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
        deck_count,
        mutable: Arc::new(Mutex::new(DrillState {
            cards: all_cards,
            reveal: false,
            reviews: Vec::new(),
            finished: false,
            started: false,
        })),
    };

    let app = Router::new()
        .route("/", get(get_handler))
        .route("/", post(post_handler))
        .route("/style.css", get(style_handler))
        .route("/script.js", get(script_handler))
        .route("/vendor/katex/katex.min.css", get(katex_css_handler))
        .route("/vendor/katex/katex.min.js", get(katex_js_handler))
        .route("/vendor/katex/fonts/{filename}", get(katex_font_handler))
        .with_state(state);

    let bind = format!("127.0.0.1:{port}");
    eprintln!("Listening on http://{bind}");
    let listener = TcpListener::bind(bind).await.expect("Failed to bind");
    axum::serve(listener, app).await.expect("Server error");
}
