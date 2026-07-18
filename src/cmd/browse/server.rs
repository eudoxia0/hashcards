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

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::extract::Path;
use axum::extract::State;
use axum::http::HeaderName;
use axum::http::StatusCode;
use axum::http::header::CACHE_CONTROL;
use axum::http::header::CONTENT_TYPE;
use axum::response::Html;
use axum::response::IntoResponse;
use axum::routing::get;
use tokio::net::TcpListener;
use tokio::signal;

use crate::cmd::browse::state::BrowseState;
use crate::cmd::browse::views::card_basic::basic_card_handler;
use crate::cmd::browse::views::card_cloze::cloze_family_handler;
use crate::cmd::browse::views::deck::deck_handler;
use crate::cmd::browse::views::index::index_handler;
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
use crate::collection::Collection;
use crate::error::Fallible;
use crate::media::load::MediaLoader;
use crate::types::card::Card;
use crate::types::card::CardContent;
use crate::types::card_hash::CardHash;
use crate::types::date::Date;
use crate::types::performance::Performance;
use crate::utils::CACHE_CONTROL_IMMUTABLE;

/// Server configuration.
pub struct BrowseServerConfig {
    /// The collection directory.
    pub directory: Option<String>,
    /// Interface to bind to.
    pub host: String,
    /// Hostname to serve resources on.
    pub resource_hostname: String,
    /// Server port.
    pub port: u16,
}

/// Start the browse server.
pub async fn start_browse_server(config: BrowseServerConfig) -> Fallible<()> {
    let Collection {
        directory,
        db,
        cards,
        macros,
    } = Collection::new(config.directory)?;

    // Take a snapshot of each card's performance data. Cards not yet in the
    // database are new. Browsing is read-only, so the database is not needed
    // after this.
    let mut performance: HashMap<CardHash, Performance> = HashMap::new();
    for card in cards.iter() {
        let perf = db
            .get_card_performance_opt(card.hash())?
            .unwrap_or(Performance::New);
        performance.insert(card.hash(), perf);
    }

    // Group cloze cards by their family hash, so that sibling cards can be
    // shown together. Within a family, sort the siblings by the position of
    // their deletion in the shared text.
    let mut families: HashMap<CardHash, Vec<Card>> = HashMap::new();
    for card in cards.iter() {
        if let Some(family) = card.family_hash() {
            families.entry(family).or_default().push(card.clone());
        }
    }
    for siblings in families.values_mut() {
        siblings.sort_by_key(deletion_start);
    }

    // Take a snapshot of each card's review history.
    let reviews = db.reviews_by_card()?;

    let state = BrowseState {
        port: config.port,
        resource_hostname: config.resource_hostname,
        directory,
        macros,
        cards: Arc::new(cards),
        families: Arc::new(families),
        performance: Arc::new(performance),
        reviews: Arc::new(reviews),
        today: Date::today(),
    };

    // Construct the app.
    let app = Router::new();
    let app = app.route("/", get(index_handler));
    let app = app.route("/deck/{name}", get(deck_handler));
    let app = app.route("/card/basic/{hash}", get(basic_card_handler));
    let app = app.route("/card/cloze/{hash}", get(cloze_family_handler));
    let app = app.route("/favicon.ico", get(favicon_handler));
    let app = app.route("/file/{*path}", get(file_handler));
    let app = app.route("/katex/fonts/{*path}", get(katex_font_handler));
    let app = app.route("/script.js", get(script_handler));
    let app = app.route("/browse.js", get(browse_js_handler));
    let app = app.route("/common.css", get(common_css_handler));
    let app = app.route("/browse.css", get(browse_css_handler));
    let app = app.route(HIGHLIGHT_CSS_URL, get(highlight_css_handler));
    let app = app.route(HIGHLIGHT_JS_URL, get(highlight_js_handler));
    let app = app.route(KATEX_CSS_URL, get(katex_css_handler));
    let app = app.route(KATEX_JS_URL, get(katex_js_handler));
    let app = app.route(KATEX_MHCHEM_JS_URL, get(katex_mhchem_js_handler));
    let app = app.fallback(not_found_handler);
    let app = app.with_state(state);

    // Start server.
    let bind = format!("{}:{}", config.host, config.port);
    log::debug!("Starting server on {bind}");
    let listener = TcpListener::bind(bind).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

/// The byte position of a cloze card's deletion in its text. Zero for basic
/// cards, which have no deletion.
fn deletion_start(card: &Card) -> usize {
    match card.content() {
        CardContent::Cloze { start, .. } => *start,
        CardContent::Basic { .. } => 0,
    }
}

async fn shutdown_signal() {
    signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
    log::debug!("Received Ctrl+C, shutting down gracefully.");
}

async fn script_handler(
    State(state): State<BrowseState>,
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
    content.push_str(include_str!("../drill/script.js"));
    (StatusCode::OK, [(CONTENT_TYPE, "text/javascript")], content)
}

async fn browse_js_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        [
            (CONTENT_TYPE, "text/javascript"),
            (CACHE_CONTROL, CACHE_CONTROL_IMMUTABLE),
        ],
        include_bytes!("resources/browse.js"),
    )
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
    css_response(include_bytes!("../drill/common.css"))
}

async fn browse_css_handler() -> (StatusCode, [(HeaderName, &'static str); 2], &'static [u8]) {
    css_response(include_bytes!("resources/browse.css"))
}

async fn favicon_handler() -> (StatusCode, [(HeaderName, &'static str); 2], &'static [u8]) {
    let bytes = include_bytes!("../drill/favicon.png");
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
    State(state): State<BrowseState>,
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
