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
use axum::http::StatusCode;
use axum::response::Html;
use axum::response::IntoResponse;
use axum::routing::get;
use tokio::net::TcpListener;
use tokio::signal;

use crate::cmd::browse::views::index::index_handler;
use crate::collection::Collection;
use crate::db::Database;
use crate::error::Fallible;
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
