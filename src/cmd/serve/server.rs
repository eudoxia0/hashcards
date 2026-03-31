use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use axum::Router;
use axum::routing::get;
use axum::routing::post;
use tokio::net::TcpListener;
use tokio::signal;
use tokio::sync::RwLock;

use crate::cmd::drill::hljs::HLJS_CSS_URL;
use crate::cmd::drill::hljs::HLJS_JS_URL;
use crate::cmd::drill::hljs::hljs_css_handler;
use crate::cmd::drill::hljs::hljs_js_handler;
use crate::cmd::drill::katex::KATEX_CSS_URL;
use crate::cmd::drill::katex::KATEX_JS_URL;
use crate::cmd::drill::katex::KATEX_MHCHEM_JS_URL;
use crate::cmd::drill::katex::katex_css_handler;
use crate::cmd::drill::katex::katex_font_handler;
use crate::cmd::drill::katex::katex_js_handler;
use crate::cmd::drill::katex::katex_mhchem_js_handler;
use crate::cmd::serve::config::ResolvedCollection;
use crate::cmd::serve::config::ResolvedGit;
use crate::cmd::serve::config::ResolvedServeConfig;
use crate::cmd::serve::git::clone_or_pull;
use crate::cmd::serve::git::refresh_collection_info;
use crate::cmd::serve::git::spawn_sync_task;
use crate::cmd::serve::handlers::collection_file_handler;
use crate::cmd::serve::handlers::collection_get_handler;
use crate::cmd::serve::handlers::collection_post_handler;
use crate::cmd::serve::handlers::collection_script_handler;
use crate::cmd::serve::handlers::collection_start_handler;
use crate::cmd::serve::handlers::sync_handler;
use crate::cmd::serve::landing::landing_handler;
use crate::cmd::serve::state::AppState;
use crate::error::Fallible;
use crate::types::timestamp::Timestamp;

pub async fn start_serve(config: ResolvedServeConfig) -> Fallible<()> {
    // Git mode: clone/pull repo and create data directories
    let sync_git = match &config.git {
        Some(git) => {
            std::fs::create_dir_all(&git.repo_dir)?;
            std::fs::create_dir_all(&git.db_dir)?;

            log::debug!("Initial git sync...");
            clone_or_pull(&git.repo_url, &git.branch, &git.repo_dir).await?;

            Some(ResolvedGit {
                repo_url: git.repo_url.clone(),
                branch: git.branch.clone(),
                poll_interval_minutes: git.poll_interval_minutes,
                repo_dir: git.repo_dir.clone(),
                db_dir: git.db_dir.clone(),
            })
        }
        None => None,
    };

    // Build collection info
    let collection_infos = refresh_collection_info(&config.collections);
    log::debug!("Loaded {} collections", collection_infos.len());

    let last_synced = if config.git.is_some() {
        Some(Timestamp::now())
    } else {
        None
    };

    let sync_collections: Vec<ResolvedCollection> = config
        .collections
        .iter()
        .map(|c| ResolvedCollection {
            name: c.name.clone(),
            slug: c.slug.clone(),
            coll_dir: c.coll_dir.clone(),
            db_path: c.db_path.clone(),
        })
        .collect();

    let bind = format!("{}:{}", config.host, config.port);

    let config = Arc::new(config);
    let state = AppState {
        config: config.clone(),
        collections: Arc::new(RwLock::new(collection_infos)),
        sessions: Arc::new(Mutex::new(HashMap::new())),
        last_synced: Arc::new(Mutex::new(last_synced)),
    };

    // Spawn background sync task (only in git mode)
    if let Some(git) = sync_git {
        spawn_sync_task(
            git,
            sync_collections,
            state.collections.clone(),
            state.last_synced.clone(),
        );
    }

    let app = Router::new()
        .route("/", get(landing_handler))
        .route("/sync", post(sync_handler))
        .route("/collection/{slug}", get(collection_get_handler))
        .route("/collection/{slug}", post(collection_post_handler))
        .route("/collection/{slug}/start", post(collection_start_handler))
        .route(
            "/collection/{slug}/file/{*path}",
            get(collection_file_handler),
        )
        .route(
            "/collection/{slug}/script.js",
            get(collection_script_handler),
        )
        .route("/script.js", get(script_handler))
        .route("/style.css", get(style_handler))
        .route(KATEX_CSS_URL, get(katex_css_handler))
        .route(KATEX_JS_URL, get(katex_js_handler))
        .route(KATEX_MHCHEM_JS_URL, get(katex_mhchem_js_handler))
        .route("/katex/fonts/{*path}", get(katex_font_handler))
        .route(HLJS_CSS_URL, get(hljs_css_handler))
        .route(HLJS_JS_URL, get(hljs_js_handler))
        .with_state(state);

    log::debug!("Starting server on {bind}");
    let listener = TcpListener::bind(&bind).await?;
    println!("hashcards server running on http://{bind}/");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    log::debug!("Server shut down");
    Ok(())
}

async fn script_handler() -> (
    axum::http::StatusCode,
    [(axum::http::HeaderName, &'static str); 1],
    &'static str,
) {
    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/javascript")],
        include_str!("../drill/script.js"),
    )
}

async fn style_handler() -> (
    axum::http::StatusCode,
    [(axum::http::HeaderName, &'static str); 2],
    &'static [u8],
) {
    (
        axum::http::StatusCode::OK,
        [
            (axum::http::header::CONTENT_TYPE, "text/css"),
            (
                axum::http::header::CACHE_CONTROL,
                crate::utils::CACHE_CONTROL_IMMUTABLE,
            ),
        ],
        include_bytes!("../drill/style.css"),
    )
}

async fn shutdown_signal() {
    signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
    log::debug!("Received shutdown signal");
}
