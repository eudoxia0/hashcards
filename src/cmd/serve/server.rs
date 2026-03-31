use std::collections::HashMap;
use std::path::PathBuf;
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
use crate::cmd::serve::git::spawn_sync_task;
use crate::cmd::serve::handlers::collection_file_handler;
use crate::cmd::serve::handlers::collection_get_handler;
use crate::cmd::serve::handlers::collection_post_handler;
use crate::cmd::serve::handlers::collection_script_handler;
use crate::cmd::serve::handlers::collection_start_handler;
use crate::cmd::serve::handlers::hedgedoc_add_handler;
use crate::cmd::serve::handlers::hedgedoc_delete_handler;
use crate::cmd::serve::handlers::hedgedoc_manage_handler;
use crate::cmd::serve::handlers::hedgedoc_sync_now_handler;
use crate::cmd::serve::handlers::sync_handler;
use crate::cmd::serve::hedgedoc::build_combined_infos;
use crate::cmd::serve::hedgedoc::build_source;
use crate::cmd::serve::hedgedoc::spawn_hedgedoc_sync_task;
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

    // Ensure DB parent directories exist for all collections (in git mode these
    // are already created above; in non-git TOML mode they may not exist yet).
    for rc in &config.collections {
        if let Some(parent) = rc.db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
    }

    // Build initial HedgeDoc sources (fetch markdown, write to disk).
    let data_dir: Option<PathBuf> = config.data_dir.clone();

    let hedgedoc_sources_init = if let Some(ref dd) = data_dir {
        let mut sources = Vec::new();
        for entry in &config.hedgedoc_entries {
            match build_source(&entry.url, dd).await {
                Ok(s) => sources.push(s),
                Err(e) => log::error!("Failed to initialize HedgeDoc source {}: {e}", entry.url),
            }
        }
        sources
    } else {
        Vec::new()
    };

    // Build combined collection info (static + hedgedoc)
    let collection_infos = build_combined_infos(&config.collections, &hedgedoc_sources_init);
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

    // Determine poll interval for HedgeDoc (inherit from git, or default 30 min).
    let hedgedoc_poll_minutes = config
        .git
        .as_ref()
        .map(|g| g.poll_interval_minutes)
        .unwrap_or(30);

    let config_path: Option<Arc<PathBuf>> = config.config_path.clone().map(Arc::new);
    let bind = format!("{}:{}", config.host, config.port);

    let config = Arc::new(config);
    let hedgedoc_sources = Arc::new(Mutex::new(hedgedoc_sources_init));
    let hedgedoc_last_synced = Arc::new(Mutex::new(None::<Timestamp>));

    let state = AppState {
        config: config.clone(),
        collections: Arc::new(RwLock::new(collection_infos)),
        sessions: Arc::new(Mutex::new(HashMap::new())),
        last_synced: Arc::new(Mutex::new(last_synced)),
        hedgedoc_sources: hedgedoc_sources.clone(),
        hedgedoc_last_synced: hedgedoc_last_synced.clone(),
        config_path,
    };

    // Spawn background git sync task (only in git mode)
    if let Some(git) = sync_git {
        spawn_sync_task(
            git,
            sync_collections.clone(),
            state.collections.clone(),
            state.last_synced.clone(),
            state.hedgedoc_sources.clone(),
        );
    }

    // Spawn background HedgeDoc sync task (only when data_dir is available)
    if let Some(dd) = data_dir {
        spawn_hedgedoc_sync_task(
            hedgedoc_sources,
            state.collections.clone(),
            hedgedoc_last_synced,
            sync_collections,
            dd,
            hedgedoc_poll_minutes,
        );
    }

    let app = Router::new()
        .route("/", get(landing_handler))
        .route("/sync", post(sync_handler))
        .route("/hedgedoc", get(hedgedoc_manage_handler))
        .route("/hedgedoc/add", post(hedgedoc_add_handler))
        .route("/hedgedoc/delete", post(hedgedoc_delete_handler))
        .route("/hedgedoc/sync", post(hedgedoc_sync_now_handler))
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
        // Landing/browse pages use this route and expect MACROS to be defined.
        concat!("let MACROS = {};\n\n", include_str!("../drill/script.js")),
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
