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

use axum::Router;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use tokio::net::TcpListener;
use tokio::select;
use tokio::signal;
use tokio::sync::oneshot::Receiver;
use tokio::sync::oneshot::channel;

use crate::collection::Collection;
use crate::error::Fallible;

/// Server configuration.
pub struct ServerConfig {
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
pub async fn start_server(config: ServerConfig) -> Fallible<()> {
    let Collection {
        directory,
        db,
        cards,
        macros,
    } = Collection::new(config.directory)?;

    // Create shutdown channel.
    let (shutdown_tx, shutdown_rx) = channel();

    // Construct the app.
    let app = Router::new();
    let app = app.route("/", get(index_handler));

    // Start server.
    let bind = format!("{}:{}", config.host, config.port);
    log::debug!("Starting server on {bind}");
    let listener = TcpListener::bind(bind).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(shutdown_rx))
        .await?;
    Ok(())
}

async fn index_handler() -> impl IntoResponse {
    (StatusCode::OK, "Hello, world!")
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
            log::debug!("Received Ctrl+C, shutting down gracefully.");
        },
        _ = shutdown => {
            log::debug!("Received shutdown signal, shutting down gracefully.");
        },
    }
}
