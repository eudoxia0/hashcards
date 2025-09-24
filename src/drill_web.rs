use std::path::PathBuf;

use axum::Router;
use axum::http::StatusCode;
use axum::response::Html;
use axum::routing::get;
use chrono::NaiveDate;
use tokio::net::TcpListener;

use crate::error::Fallible;

#[derive(Clone)]
pub struct ServerState {}

pub async fn drill_web(directory: PathBuf, today: NaiveDate) -> Fallible<()> {
    let state = ServerState {};
    let app = Router::new();
    let app = app.route("/", get(root));
    let app = app.fallback(not_found_handler);
    let app = app.with_state(state);
    let listener = TcpListener::bind("0.0.0.0:8000").await?;
    axum::serve(listener, app).await?;
    todo!()
}

async fn root() -> &'static str {
    "Hello, World!"
}

async fn not_found_handler() -> (StatusCode, Html<String>) {
    let html = format!("Not Found");
    (StatusCode::OK, Html(html))
}
