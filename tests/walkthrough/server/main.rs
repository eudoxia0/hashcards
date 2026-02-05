//! Minimal walkthrough server for generating screenshots.
//!
//! This standalone binary replicates the hashcards drill interface using
//! hashcards-core for card parsing and rendering, served via axum.
//! It does not persist any data to a database — it's purely for
//! generating walkthrough screenshots.

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
) -> (StatusCode, Html<String>) {
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
            "Shutdown" => {
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
    let controls = if m.reveal {
        html! {
            form action="/" method="post" {
                @if undo_disabled {
                    input id="undo" type="submit" name="action" value="Undo" disabled;
                } @else {
                    input id="undo" type="submit" name="action" value="Undo" title="Undo last action. Shortcut: u.";
                }
                div.spacer {}
                div.grades {
                    input id="forgot" type="submit" name="action" value="Forgot" title="Mark card as forgotten. Shortcut: 1.";
                    input id="hard" type="submit" name="action" value="Hard" title="Mark card as difficult. Shortcut: 2.";
                    input id="good" type="submit" name="action" value="Good" title="Mark card as remembered well. Shortcut: 3.";
                    input id="easy" type="submit" name="action" value="Easy" title="Mark card as very easy. Shortcut: 4.";
                }
                div.spacer {}
                input id="end" type="submit" name="action" value="End" title="End the session (changes are saved)";
            }
        }
    } else {
        html! {
            form action="/" method="post" {
                @if undo_disabled {
                    input id="undo" type="submit" name="action" value="Undo" disabled;
                } @else {
                    input id="undo" type="submit" name="action" value="Undo" title="Undo last action. Shortcut: u.";
                }
                div.spacer {}
                input id="reveal" type="submit" name="action" value="Reveal" title="Show the answer. Shortcut: space.";
                div.spacer {}
                input id="end" type="submit" name="action" value="End" title="End the session (changes are saved)";
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
            div.shutdown-container {
                form action="/" method="post" {
                    input #shutdown .shutdown-button type="submit" name="action" value="Shutdown" title="Shut down the server";
                }
            }
        }
    }
}

// ── Template ───────────────────────────────────────────────────

const KATEX_CSS: &str = "https://cdn.jsdelivr.net/npm/katex@0.16.21/dist/katex.min.css";
const KATEX_JS: &str = "https://cdn.jsdelivr.net/npm/katex@0.16.21/dist/katex.min.js";
const HIGHLIGHT_JS: &str = "https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/highlight.min.js";
const HIGHLIGHT_CSS: &str = "https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/styles/github.min.css";

fn page_template(body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "hashcards" }
                meta name="color-scheme" content="light dark";
                link rel="stylesheet" href=(KATEX_CSS);
                link rel="stylesheet" href=(HIGHLIGHT_CSS);
                script defer src=(KATEX_JS) {};
                script defer src=(HIGHLIGHT_JS) {};
                link rel="stylesheet" href="/style.css";
                // Start visible — no JS opacity trick needed for screenshots
            }
            body {
                (body)
                script src="/script.js" {};
            }
        }
    }
}

// ── Static assets ──────────────────────────────────────────────

const DARK_MODE_CSS: &str = r#"
@media (prefers-color-scheme: dark) {
    body { background: #1a1a2e; color: #e0e0e0; }

    .root {
        .header, .controls { background: #16213e; }
        .header { border-bottom-color: #334; }
        .header .progress-bar { background: #222; border-color: #555; }
        .card-container { background: #1a1a2e; }
        .card-container .card {
            background: #16213e;
            border-color: #334;
            box-shadow: 0px 0px 48px 16px rgba(0,0,0,0.4);
        }
        .card-container .card .card-header { border-bottom-color: #334; }
        .card-container .card .card-content .question { border-bottom-color: #334; }
        .card-container .card .card-content .rich-text {
            color: #e0e0e0;
        }
        .card-container .card .card-content .rich-text .cloze { background: #555; }
        .card-container .card .card-content .rich-text .cloze-reveal { color: #6cb4ee; }
        .card-container .card .card-content .rich-text code { background: #2a2a4a; border-color: #444; }
        .card-container .card .card-content .rich-text pre { background: #2a2a4a; border-color: #444; }
        .card-container .card .card-content .rich-text blockquote { background: #222244; border-left-color: #555; }
        .card-container .card .card-content .rich-text table { border-color: #444; }
        .card-container .card .card-content .rich-text table th { background: #2a2a4a; border-color: #444; }
        .card-container .card .card-content .rich-text table td { border-color: #444; }
        .card-container .card .card-content .rich-text table tbody tr:nth-child(odd) { background: #1a1a2e; }
        .card-container .card .card-content .rich-text table tbody tr:nth-child(even) { background: #222244; }
        .controls { border-top-color: #334; }
        .controls form input {
            background: #2a2a4a;
            color: #e0e0e0;
            border-color: #555;
            box-shadow: rgba(0,0,0,0.3) 0px 1px 3px 0px;
        }
    }

    .finished {
        h2 { border-bottom-color: #444; }
        .stats table tr { border-bottom-color: #333; }
    }
}
"#;

async fn style_handler() -> (StatusCode, [(HeaderName, &'static str); 1], String) {
    let mut css = String::from_utf8_lossy(include_bytes!("../../../src/cmd/drill/style.css")).into_owned();
    css.push_str(DARK_MODE_CSS);
    (StatusCode::OK, [(CONTENT_TYPE, "text/css")], css)
}

async fn script_handler() -> (StatusCode, [(HeaderName, &'static str); 1], String) {
    let mut content = String::new();
    content.push_str("let MACROS = {};\n\n");
    content.push_str(include_str!("../../../src/cmd/drill/script.js"));
    (StatusCode::OK, [(CONTENT_TYPE, "text/javascript")], content)
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
    eprintln!("Loaded {} cards from {}", total, collection_dir.display());

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
        .with_state(state);

    let bind = format!("127.0.0.1:{port}");
    eprintln!("Listening on http://{bind}");
    let listener = TcpListener::bind(bind).await.expect("Failed to bind");
    axum::serve(listener, app).await.expect("Server error");
}
