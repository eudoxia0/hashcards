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

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use axum::Router;
use axum::extract::Path;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use axum::response::IntoResponse;
use axum::routing::get;
use http::header::CACHE_CONTROL;
use http::header::CONTENT_TYPE;
use tokio::net::TcpListener;
use tokio::signal;

use crate::cmd::browse::views::card_basic::card_basic_handler;
use crate::cmd::browse::views::card_cloze::card_cloze_handler;
use crate::cmd::browse::views::deck::deck_handler;
use crate::cmd::browse::views::index::index_handler;
use crate::collection::Collection;
use crate::db::Database;
use crate::error::Fallible;
use crate::server::constants::CACHE_CONTROL_IMMUTABLE;
use crate::server::constants::CONTENT_TYPE_CSS;
use crate::server::constants::CONTENT_TYPE_JS;
use crate::server::file_handler::file_handler_logic;
use crate::server::highlight::HIGHLIGHT_CSS_URL;
use crate::server::highlight::HIGHLIGHT_JS_URL;
use crate::server::highlight::highlight_css_handler;
use crate::server::highlight::highlight_js_handler;
use crate::server::js::escape_js_string_literal;
use crate::server::katex::KATEX_CSS_URL;
use crate::server::katex::KATEX_JS_URL;
use crate::server::katex::KATEX_MHCHEM_JS_URL;
use crate::server::katex::katex_css_handler;
use crate::server::katex::katex_font_handler;
use crate::server::katex::katex_js_handler;
use crate::server::katex::katex_mhchem_js_handler;
use crate::server::resources::common_css_handler;
use crate::server::resources::favicon_handler;
use crate::types::card::Card;

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

/// Server state.
#[derive(Clone)]
pub struct BrowseState {
    /// Server port.
    pub port: u16,
    /// Hostname to serve resources on.
    pub resource_hostname: String,
    /// The collection directory.
    pub directory: PathBuf,
    /// TeX macros.
    pub macros: Vec<(String, String)>,
    /// All the cards in the collection, behind an [`Arc`] so we don't copy the
    /// entire collection on each request.
    pub cards: Arc<Vec<Card>>,
    /// The database.
    pub db: Arc<Mutex<Database>>,
}

/// Start the browse server.
pub async fn start_browse_server(config: BrowseServerConfig) -> Fallible<()> {
    // Load the collection.
    let Collection {
        directory,
        db,
        cards,
        macros,
    } = Collection::new(config.directory)?;
    // Construct app state.
    let cards: Arc<Vec<Card>> = Arc::new(cards);
    let db: Arc<Mutex<Database>> = Arc::new(Mutex::new(db));
    let state = BrowseState {
        port: config.port,
        resource_hostname: config.resource_hostname.clone(),
        directory: directory,
        macros,
        cards,
        db,
    };
    // Construct the app.
    let app = Router::new();
    let app = app.route("/", get(index_handler));
    let app = app.route("/browse.css", get(browse_css_handler));
    let app = app.route("/browse.js", get(browse_js_handler));
    let app = app.route("/card/basic/{card_hash}", get(card_basic_handler));
    let app = app.route("/card/cloze/{family_hash}", get(card_cloze_handler));
    let app = app.route("/common.css", get(common_css_handler));
    let app = app.route("/deck/{deck_name}", get(deck_handler));
    let app = app.route("/favicon.ico", get(favicon_handler));
    let app = app.route("/file/{*path}", get(file_handler));
    let app = app.route("/katex/fonts/{*path}", get(katex_font_handler));
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

async fn shutdown_signal() {
    signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
    log::debug!("Received Ctrl+C, shutting down gracefully.");
}

async fn not_found_handler() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, Html("Not Found"))
}

async fn browse_css_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        [
            (CONTENT_TYPE, CONTENT_TYPE_CSS),
            (CACHE_CONTROL, CACHE_CONTROL_IMMUTABLE),
        ],
        include_bytes!("resources/browse.css"),
    )
}

async fn browse_js_handler(State(state): State<BrowseState>) -> impl IntoResponse {
    let mut content = String::new();
    content.push_str("let MACROS = {};\n");
    for (name, definition) in &state.macros {
        let name = escape_js_string_literal(name);
        let definition = escape_js_string_literal(definition);
        content.push_str(&format!("MACROS['{name}'] = '{definition}';\n"));
    }
    content.push_str("MACROS[','] = '{\\\\char`,}';\n");
    content.push('\n');
    content.push_str(include_str!("resources/browse.js"));
    (StatusCode::OK, [(CONTENT_TYPE, "text/javascript")], content)
}

async fn file_handler(
    State(state): State<BrowseState>,
    Path(path): Path<String>,
) -> impl IntoResponse {
    file_handler_logic(state.directory.clone(), path).await
}
