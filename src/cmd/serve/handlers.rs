use std::collections::HashSet;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use axum::Form;
use axum::extract::Path;
use axum::extract::State;
use serde::Deserialize;
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
use crate::cmd::serve::config::ResolvedCollection;
use crate::cmd::serve::git::clone_or_pull;
use crate::cmd::serve::git::refresh_collection_info;
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
    let html = match collection_get_inner(&state, &slug) {
        Ok(html) => html,
        Err(e) => page_template(html! {
            div.error {
                h1 { "Error" }
                p { (e) }
                a href="/" { "Back to collections" }
            }
        })
        .into_string(),
    };
    (StatusCode::OK, Html(html))
}

fn collection_get_inner(state: &AppState, slug: &str) -> Fallible<String> {
    let sessions = state.sessions.lock().unwrap();

    // If no active session, show the deck browser.
    if !sessions.contains_key(slug) {
        drop(sessions);
        let rc = find_collection(state, slug)
            .ok_or_else(|| crate::error::ErrorReport::new(format!("Unknown collection: {slug}")))?;
        let tree = build_deck_tree(&rc.coll_dir, &rc.db_path)?;
        let html = render_browse_page(&rc.name, slug, &tree);
        return Ok(html.into_string());
    }

    let session = sessions.get(slug).unwrap();
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
    Ok(html.into_string())
}

fn find_collection<'a>(state: &'a AppState, slug: &str) -> Option<&'a ResolvedCollection> {
    state.config.collections.iter().find(|c| c.slug == slug)
}

/// Form data for the start-drill endpoint.
#[derive(Deserialize)]
pub struct StartDrillForm {
    #[serde(default, deserialize_with = "deserialize_string_or_seq")]
    pub decks: Vec<String>,
}

/// Deserialize a field that may be a single string or a sequence of strings.
///
/// HTML forms submit a single `key=value` when one checkbox is checked, and
/// repeated `key=value&key=value` when multiple are checked. `serde_urlencoded`
/// sees the single-value case as a bare string, not a sequence, so we handle
/// both shapes here.
fn deserialize_string_or_seq<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{SeqAccess, Visitor};

    struct StringOrSeq;

    impl<'de> Visitor<'de> for StringOrSeq {
        type Value = Vec<String>;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("string or sequence of strings")
        }

        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
            Ok(vec![v.to_string()])
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut result = Vec::new();
            while let Some(value) = seq.next_element()? {
                result.push(value);
            }
            Ok(result)
        }
    }

    deserializer.deserialize_any(StringOrSeq)
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
        ActionResult::Home => Ok(Redirect::to("/")),
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
            let infos = refresh_collection_info(&state.config.collections);
            *state.collections.write().await = infos;
            *state.last_synced.lock().unwrap() = Some(Timestamp::now());
            log::debug!("Manual sync completed successfully");
        }
        Err(e) => {
            log::error!("Manual sync failed: {e}");
        }
    }
    Redirect::to("/")
}
