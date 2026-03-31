use std::collections::HashSet;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use axum::Form;
use axum::extract::Path;
use axum::extract::State;
use axum::http::HeaderName;
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::Html;
use axum::response::Redirect;
use maud::html;

use crate::cmd::drill::cache::Cache;
use crate::cmd::drill::get::CompletionAction;
use crate::cmd::drill::get::RenderContext;
use crate::cmd::drill::get::render_completion_page;
use crate::cmd::drill::get::render_session_page;
use crate::cmd::drill::post::Action;
use crate::cmd::drill::post::ActionResult;
use crate::cmd::drill::post::FormData;
use crate::cmd::drill::post::handle_action;
use crate::cmd::drill::server::escape_js_string_literal;
use crate::cmd::drill::template::page_template;
use crate::cmd::drill::template::page_template_with_script;
use crate::cmd::serve::browse::build_deck_tree;
use crate::cmd::serve::browse::render_browse_page;
use crate::cmd::serve::config::HedgedocEntry;
use crate::cmd::serve::config::ResolvedCollection;
use crate::cmd::serve::git::clone_or_pull;
use crate::cmd::serve::hedgedoc::build_combined_infos;
use crate::cmd::serve::hedgedoc::build_note;
use crate::cmd::serve::hedgedoc::build_source;
use crate::cmd::serve::hedgedoc::create_minimal_config;
use crate::cmd::serve::hedgedoc::all_hedgedoc_entries;
use crate::cmd::serve::hedgedoc::persist_hedgedoc_entries;
use crate::cmd::serve::hedgedoc::source_uri_from_url;
use crate::cmd::serve::hedgedoc::sync_source;
use crate::cmd::serve::state::AppState;
use crate::cmd::serve::state::DrillSession;
use crate::collection::Collection;
use crate::error::Fallible;
use crate::media::load::MediaLoader;
use crate::rng::TinyRng;
use crate::rng::shuffle;
use crate::types::card::Card;
use crate::types::card_hash::CardHash;
use crate::types::date::Date;
use crate::types::timestamp::Timestamp;

pub async fn collection_get_handler(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> (StatusCode, Html<String>) {
    // Determine whether this slug is known before calling the inner function,
    // so we can return 404 for unknown collections vs. 500 for real errors.
    let known = find_collection(&state, &slug).is_some()
        || state.sessions.lock().unwrap().contains_key(&slug);
    match collection_get_inner(&state, &slug) {
        Ok(html) => (StatusCode::OK, Html(html)),
        Err(e) => {
            let status = if known {
                StatusCode::INTERNAL_SERVER_ERROR
            } else {
                StatusCode::NOT_FOUND
            };
            let html = page_template(html! {
                div.error {
                    h1 { "Error" }
                    p { (e) }
                    a href="/" { "Back to collections" }
                }
            })
            .into_string();
            (status, Html(html))
        }
    }
}

fn collection_get_inner(state: &AppState, slug: &str) -> Fallible<String> {
    // Take the session out of the map so the lock is not held during rendering.
    let session = state.sessions.lock().unwrap().remove(slug);

    let Some(session) = session else {
        // No active session: show the deck browser.
        let rc = find_collection(state, slug)
            .ok_or_else(|| crate::error::ErrorReport::new(format!("Unknown collection: {slug}")))?;
        let tree = build_deck_tree(&rc.coll_dir, &rc.db_path)?;
        let html = render_browse_page(&rc.name, slug, &tree);
        return Ok(html.into_string());
    };

    let form_action = format!("/collection/{slug}");
    let file_url_prefix = format!("/collection/{slug}/file");
    let ctx = RenderContext {
        directory: &session.directory,
        total_cards: session.total_cards,
        session_started_at: session.session_started_at,
        answer_controls: session.answer_controls,
        form_action: &form_action,
        file_url_prefix: &file_url_prefix,
        completion_action: CompletionAction::BackToCollections,
    };
    let body = if session.mutable.finished_at.is_some() {
        render_completion_page(&ctx, &session.mutable)?
    } else {
        render_session_page(&ctx, &session.mutable)?
    };
    let script_url = format!("/collection/{slug}/script.js");
    let html = page_template_with_script(&script_url, body);
    // Put the session back now that rendering is done.
    state.sessions.lock().unwrap().insert(slug.to_owned(), session);
    Ok(html.into_string())
}

fn find_collection(state: &AppState, slug: &str) -> Option<ResolvedCollection> {
    if let Some(rc) = state.config.collections.iter().find(|c| c.slug == slug) {
        return Some(rc.clone());
    }
    let sources = state.hedgedoc_sources.lock().unwrap();
    sources
        .iter()
        .find(|s| s.collection.slug == slug)
        .map(|s| s.collection.clone())
}

/// Form data for the start-drill endpoint.
pub struct StartDrillForm {
    pub decks: Vec<String>,
}

/// Custom `Deserialize` for `StartDrillForm`.
///
/// `serde_urlencoded` presents repeated keys (`decks=foo&decks=bar`) as
/// separate map entries rather than grouping them into a sequence first.
/// The derived `Deserialize` macro errors on "duplicate field" in that case,
/// so we implement the visitor manually to accumulate all `decks` values.
impl<'de> serde::Deserialize<'de> for StartDrillForm {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::{MapAccess, Visitor};

        struct FormVisitor;

        impl<'de> Visitor<'de> for FormVisitor {
            type Value = StartDrillForm;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a form with optional decks fields")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut decks = Vec::new();
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "decks" => decks.push(map.next_value::<String>()?),
                        _ => {
                            let _ = map.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(StartDrillForm { decks })
            }
        }

        deserializer.deserialize_map(FormVisitor)
    }
}

pub async fn collection_start_handler(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Form(form): Form<StartDrillForm>,
) -> Redirect {
    match collection_start_inner(&state, &slug, form.decks) {
        Ok(()) => Redirect::to(&format!("/collection/{slug}")),
        Err(e) => {
            log::error!("error starting drill for collection {slug}: {e}");
            Redirect::to(&format!("/collection/{slug}"))
        }
    }
}

fn collection_start_inner(
    state: &AppState,
    slug: &str,
    selected_decks: Vec<String>,
) -> Fallible<()> {
    // Remove any existing session before doing DB work.
    state.sessions.lock().unwrap().remove(slug);

    // Create the session outside the lock (may do filesystem/DB work).
    let session = create_session(state, slug, &selected_decks)?;
    if let Some(s) = session {
        state.sessions.lock().unwrap().insert(slug.to_string(), s);
    }
    Ok(())
}

fn create_session(
    state: &AppState,
    slug: &str,
    selected_decks: &[String],
) -> Fallible<Option<DrillSession>> {
    let rc = find_collection(state, slug)
        .ok_or_else(|| crate::error::ErrorReport::new(format!("Unknown collection: {slug}")))?;

    let collection = Collection::with_db_path(rc.coll_dir.clone(), rc.db_path.clone())?;

    let session_started_at = Timestamp::now();
    let today: Date = session_started_at.date();

    // Sync new cards to DB
    let db_hashes: HashSet<CardHash> = collection.db.card_hashes()?;
    for card in collection.cards.iter() {
        if !db_hashes.contains(&card.hash()) {
            collection.db.insert_card(card.hash(), session_started_at)?;
        }
    }

    // Filter by selected decks.
    let deck_filter: HashSet<&str> = selected_decks.iter().map(|s| s.as_str()).collect();
    let cards: Vec<Card> = if deck_filter.is_empty() {
        collection.cards
    } else {
        collection
            .cards
            .into_iter()
            .filter(|card| deck_filter.contains(card.deck_name().as_str()))
            .collect()
    };

    // Find cards due today
    let due_today: HashSet<CardHash> = collection.db.due_today(today)?;
    let mut due_cards: Vec<Card> = cards
        .into_iter()
        .filter(|card| due_today.contains(&card.hash()))
        .collect();

    if state.config.defaults.bury_siblings {
        due_cards = bury_siblings(due_cards);
    }

    if due_cards.is_empty() {
        return Ok(None);
    }

    // Shuffle
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    let mut rng = TinyRng::from_seed(seed);
    due_cards = shuffle(due_cards, &mut rng);

    // Build cache
    let mut cache = Cache::new();
    for card in due_cards.iter() {
        let performance = collection.db.get_card_performance(card.hash())?;
        cache.insert(card.hash(), performance)?;
    }

    let answer_controls = state.config.defaults.answer_controls.into();

    Ok(Some(DrillSession::new(
        collection.directory,
        collection.macros,
        due_cards,
        cache,
        session_started_at,
        answer_controls,
        collection.db,
    )))
}

fn bury_siblings(deck: Vec<Card>) -> Vec<Card> {
    let mut seen_families = HashSet::new();
    let mut result = Vec::new();
    for card in deck.into_iter() {
        if let Some(family) = card.family_hash() {
            if seen_families.contains(&family) {
                continue;
            }
            seen_families.insert(family);
        }
        result.push(card);
    }
    result
}

pub async fn collection_post_handler(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Form(form): Form<FormData>,
) -> Redirect {
    match collection_post_inner(&state, &slug, form.action) {
        Ok(redirect) => redirect,
        Err(e) => {
            log::error!("error handling action for collection {slug}: {e}");
            Redirect::to(&format!("/collection/{slug}"))
        }
    }
}

fn collection_post_inner(state: &AppState, slug: &str, action: Action) -> Fallible<Redirect> {
    // Home action: drop session without needing to hold lock during DB work
    if matches!(action, Action::Home) {
        state.sessions.lock().unwrap().remove(slug);
        
        // Refresh collection info to update due_today counts
        let combined = {
            let sources = state.hedgedoc_sources.lock().unwrap();
            build_combined_infos(&state.config.collections, &sources)
        };
        // Update in background (don't block the response)
        let collections_clone = state.collections.clone();
        tokio::spawn(async move {
            *collections_clone.write().await = combined;
        });
        
        return Ok(Redirect::to("/"));
    }

    // Take ownership of the session so we can release the global lock before
    // handle_action does any DB work.
    let mut session = match state.sessions.lock().unwrap().remove(slug) {
        Some(s) => s,
        None => return Ok(Redirect::to(&format!("/collection/{slug}"))),
    };

    let result = handle_action(
        &mut session.mutable,
        session.session_started_at,
        action,
    )?;

    match result {
        ActionResult::Home => {
            // Refresh collection info to update due_today counts
            let combined = {
                let sources = state.hedgedoc_sources.lock().unwrap();
                build_combined_infos(&state.config.collections, &sources)
            };
            // Update in background (don't block the response)
            let collections_clone = state.collections.clone();
            tokio::spawn(async move {
                *collections_clone.write().await = combined;
            });
            Ok(Redirect::to("/"))
        }
        _ => {
            state.sessions.lock().unwrap().insert(slug.to_owned(), session);
            Ok(Redirect::to(&format!("/collection/{slug}")))
        }
    }
}

pub async fn collection_file_handler(
    State(state): State<AppState>,
    Path((slug, path)): Path<(String, String)>,
) -> (StatusCode, [(HeaderName, &'static str); 1], Vec<u8>) {
    let coll_dir = match find_collection(&state, &slug) {
        Some(rc) => rc.coll_dir.clone(),
        None => {
            return (
                StatusCode::NOT_FOUND,
                [(CONTENT_TYPE, "text/plain")],
                b"Collection not found".to_vec(),
            );
        }
    };

    let loader = MediaLoader::new(coll_dir);
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

pub async fn collection_script_handler(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> (StatusCode, [(HeaderName, &'static str); 1], String) {
    let sessions = state.sessions.lock().unwrap();
    let macros = match sessions.get(&slug) {
        Some(session) => &session.macros,
        None => {
            // No active session; serve script without macros
            let content = format!("let MACROS = {{}};\n\n{}", include_str!("../drill/script.js"));
            return (StatusCode::OK, [(CONTENT_TYPE, "text/javascript")], content);
        }
    };
    let mut content = String::new();
    content.push_str("let MACROS = {};\n");
    for (name, definition) in macros {
        let name = escape_js_string_literal(name);
        let definition = escape_js_string_literal(definition);
        content.push_str(&format!("MACROS['{name}'] = '{definition}';\n"));
    }
    content.push('\n');
    content.push_str(include_str!("../drill/script.js"));
    (StatusCode::OK, [(CONTENT_TYPE, "text/javascript")], content)
}

pub async fn sync_handler(State(state): State<AppState>) -> Redirect {
    let git = match &state.config.git {
        Some(git) => git,
        None => return Redirect::to("/"),
    };

    match clone_or_pull(&git.repo_url, &git.branch, &git.repo_dir).await {
        Ok(()) => {
            let combined = {
                let sources = state.hedgedoc_sources.lock().unwrap();
                build_combined_infos(&state.config.collections, &sources)
            };
            *state.collections.write().await = combined;
            *state.last_synced.lock().unwrap() = Some(Timestamp::now());
            log::debug!("Manual sync completed successfully");
        }
        Err(e) => {
            log::error!("Manual sync failed: {e}");
        }
    }
    Redirect::to("/")
}

// ---- HedgeDoc management handlers ----

/// Render the HedgeDoc source management page.
pub async fn hedgedoc_manage_handler(
    State(state): State<AppState>,
) -> (StatusCode, Html<String>) {
    use crate::cmd::serve::hedgedoc_ui::render_manage_page;
    let sources = state.hedgedoc_sources.lock().unwrap();
    let last_synced = *state.hedgedoc_last_synced.lock().unwrap();
    let config_available = state.config.data_dir.is_some();
    let html = render_manage_page(&sources, last_synced, config_available);
    (StatusCode::OK, Html(html.into_string()))
}

#[derive(serde::Deserialize)]
pub struct AddHedgedocForm {
    pub url: String,
}

/// Add a new HedgeDoc source URL.
pub async fn hedgedoc_add_handler(
    State(state): State<AppState>,
    Form(form): Form<AddHedgedocForm>,
) -> Redirect {
    let url = form.url.trim().to_string();
    if url.is_empty() {
        return Redirect::to("/hedgedoc");
    }

    let data_dir = match &state.config.data_dir {
        Some(d) => d.clone(),
        None => {
            log::error!("Cannot add HedgeDoc source: no data_dir configured");
            return Redirect::to("/hedgedoc");
        }
    };

    // Check for duplicate URL.
    {
        let sources = state.hedgedoc_sources.lock().unwrap();
        if sources
            .iter()
            .flat_map(|s| s.notes.iter())
            .any(|n| n.url == url)
        {
            return Redirect::to("/hedgedoc");
        }
    }

    let source_uri = match source_uri_from_url(&url) {
        Some(uri) => uri,
        None => {
            log::error!("Failed to parse HedgeDoc source URI from {url}");
            return Redirect::to("/hedgedoc");
        }
    };

    let existing_collection = {
        let sources = state.hedgedoc_sources.lock().unwrap();
        sources
            .iter()
            .find(|s| s.source_uri == source_uri)
            .map(|s| s.collection.clone())
    };

    let mut new_source: Option<crate::cmd::serve::state::HedgedocSource> = None;
    let mut new_note: Option<crate::cmd::serve::state::HedgedocNote> = None;

    if let Some(collection) = existing_collection {
        match build_note(&url, &collection).await {
            Ok(note) => new_note = Some(note),
            Err(e) => {
                log::error!("Failed to add HedgeDoc note {url}: {e}");
                return Redirect::to("/hedgedoc");
            }
        }
    } else {
        match build_source(&url, &data_dir).await {
            Ok(source) => new_source = Some(source),
            Err(e) => {
                log::error!("Failed to add HedgeDoc source {url}: {e}");
                return Redirect::to("/hedgedoc");
            }
        }
    }

    // Add to in-memory state
    let new_entries: Vec<HedgedocEntry> = {
        let mut sources = state.hedgedoc_sources.lock().unwrap();
        if let Some(source) = new_source {
            if sources
                .iter()
                .flat_map(|s| s.notes.iter())
                .any(|n| n.url == url)
            {
                return Redirect::to("/hedgedoc");
            }
            sources.push(source);
        } else if let Some(note) = new_note {
            if let Some(src) = sources.iter_mut().find(|s| s.source_uri == source_uri) {
                if src.notes.iter().any(|n| n.url == url) {
                    return Redirect::to("/hedgedoc");
                }
                src.notes.push(note);
            }
        }
        all_hedgedoc_entries(&sources)
    };

    // Get or create config path (critical section before async operations)
    let config_path = {
        let mut config_path_guard = state.config_path.lock().unwrap();
        match config_path_guard.as_ref() {
            Some(p) => p.clone(),
            None => {
                // Create minimal config file on first add
                match create_minimal_config() {
                    Ok(p) => {
                        *config_path_guard = Some(p.clone());
                        p
                    }
                    Err(e) => {
                        log::error!("Failed to create minimal config file: {e}");
                        return Redirect::to("/hedgedoc");
                    }
                }
            }
        }
    };

    // Persist to TOML
    if let Err(e) = persist_hedgedoc_entries(&config_path, &new_entries) {
        log::error!("Failed to persist HedgeDoc entries to config: {e}");
        return Redirect::to("/hedgedoc");
    }

    // Refresh combined collection infos.
    let combined = {
        let sources = state.hedgedoc_sources.lock().unwrap();
        build_combined_infos(&state.config.collections, &sources)
    };
    *state.collections.write().await = combined;

    Redirect::to("/hedgedoc")
}

#[derive(serde::Deserialize)]
pub struct DeleteHedgedocForm {
    pub url: String,
}

/// Remove a HedgeDoc source by URL.
pub async fn hedgedoc_delete_handler(
    State(state): State<AppState>,
    Form(form): Form<DeleteHedgedocForm>,
) -> Redirect {
    let remaining: Vec<HedgedocEntry> = {
        let mut sources = state.hedgedoc_sources.lock().unwrap();
        for src in sources.iter_mut() {
            src.notes.retain(|n| n.url != form.url);
        }
        sources.retain(|s| !s.notes.is_empty());
        all_hedgedoc_entries(&sources)
    };

    // Persist to TOML if config file is available
    if let Some(config_path) = state.config_path.lock().unwrap().as_ref() {
        if let Err(e) = persist_hedgedoc_entries(config_path, &remaining) {
            log::error!("Failed to persist HedgeDoc entries after deletion: {e}");
        }
    }

    // Refresh combined collection infos.
    let combined = {
        let sources = state.hedgedoc_sources.lock().unwrap();
        build_combined_infos(&state.config.collections, &sources)
    };
    *state.collections.write().await = combined;

    Redirect::to("/hedgedoc")
}

/// Manually re-sync all HedgeDoc sources.
pub async fn hedgedoc_sync_now_handler(State(state): State<AppState>) -> Redirect {
    if state.config.data_dir.is_none() {
        return Redirect::to("/hedgedoc");
    }

    // Collect URLs to sync (release lock before awaiting).
    let entries: Vec<(String, ResolvedCollection)> = {
        let sources = state.hedgedoc_sources.lock().unwrap();
        sources
            .iter()
            .flat_map(|s| {
                s.notes
                    .iter()
                    .map(move |n| (n.url.clone(), s.collection.clone()))
            })
            .collect()
    };

    for (url, rc) in &entries {
        match sync_source(url, rc).await {
            Ok((deck_name, file_name)) => {
                let mut sources = state.hedgedoc_sources.lock().unwrap();
                for src in sources.iter_mut() {
                    if let Some(note) = src.notes.iter_mut().find(|n| &n.url == url) {
                        note.deck_name = deck_name.clone();
                        note.file_name = file_name.clone();
                        note.last_error = None;
                        break;
                    }
                }
            }
            Err(e) => {
                let msg = e.to_string();
                log::error!("Manual HedgeDoc sync failed for {url}: {msg}");
                let mut sources = state.hedgedoc_sources.lock().unwrap();
                for src in sources.iter_mut() {
                    if let Some(note) = src.notes.iter_mut().find(|n| &n.url == url) {
                        note.last_error = Some(msg.clone());
                        break;
                    }
                }
            }
        }
    }

    let combined = {
        let sources = state.hedgedoc_sources.lock().unwrap();
        build_combined_infos(&state.config.collections, &sources)
    };
    *state.collections.write().await = combined;
    *state.hedgedoc_last_synced.lock().unwrap() = Some(Timestamp::now());

    Redirect::to("/hedgedoc")
}
