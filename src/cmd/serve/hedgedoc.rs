use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use tokio::sync::RwLock;
use tokio::time::interval;

use crate::cmd::serve::config::HedgedocEntry;
use crate::cmd::serve::config::ResolvedCollection;
use crate::cmd::serve::config::slugify;
use crate::cmd::serve::git::compute_collection_counts;
use crate::cmd::serve::state::CollectionInfo;
use crate::cmd::serve::state::HedgedocNote;
use crate::cmd::serve::state::HedgedocSource;
use crate::error::Fallible;
use crate::error::fail;
use crate::types::timestamp::Timestamp;

/// Extract the note ID (last non-empty path segment) from a HedgeDoc URL.
pub fn note_id_from_url(url: &str) -> Option<String> {
    url.trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

pub fn source_uri_from_url(url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    let port = parsed.port();
    Some(match port {
        Some(p) => format!("{}://{}:{}", parsed.scheme(), host, p),
        None => format!("{}://{}", parsed.scheme(), host),
    })
}

pub fn source_display_name(source_uri: &str) -> String {
    source_uri.to_string()
}

/// Build the collection slug for a HedgeDoc source URI.
pub fn slug_for_source_uri(source_uri: &str) -> String {
    format!("hedgedoc-{}", slugify(source_uri))
}

/// Fetch raw markdown from a HedgeDoc note URL.
/// Appends `/download` to the note URL to get the raw markdown.
pub async fn fetch_markdown(url: &str) -> Fallible<String> {
    let download_url = format!("{}/download", url.trim_end_matches('/'));
    let response = reqwest::get(&download_url).await?;
    if !response.status().is_success() {
        return fail(format!(
            "HedgeDoc fetch returned HTTP {} for {}",
            response.status(),
            download_url
        ));
    }
    Ok(response.text().await?)
}

/// Extract a human-readable title from markdown content.
/// Checks YAML frontmatter `title:` first, then the first `# heading`.
/// Falls back to `fallback` (typically the note ID).
pub fn extract_title(markdown: &str, fallback: &str) -> String {
    if let Some(title) = frontmatter_title(markdown) {
        return title;
    }
    for line in markdown.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("# ") {
            let title = rest.trim();
            if !title.is_empty() {
                return title.to_string();
            }
        }
    }
    fallback.to_string()
}

fn sanitize_deck_name(name: &str, fallback: &str) -> String {
    let candidate = if name.trim().is_empty() {
        fallback.trim()
    } else {
        name.trim()
    };
    let normalized = candidate.replace('/', " - ");
    if normalized.trim().is_empty() {
        "Deck".to_string()
    } else {
        normalized.trim().to_string()
    }
}

fn strip_leading_yaml_frontmatter(markdown: &str) -> &str {
    let content = markdown.trim_start();
    if !content.starts_with("---") {
        return markdown;
    }
    let after = match content.get(3..) {
        Some(s) => s,
        None => return markdown,
    };
    let end = match after.find("\n---") {
        Some(idx) => idx,
        None => return markdown,
    };

    let body_start_in_trimmed = 3 + end + 4;
    let body = match content.get(body_start_in_trimmed..) {
        Some(s) => s,
        None => return markdown,
    };
    body.trim_start_matches('\n')
}

fn wrap_with_deck_frontmatter(markdown: &str, deck_name: &str) -> Fallible<String> {
    let mut table = toml::map::Map::new();
    table.insert("name".to_string(), toml::Value::String(deck_name.to_string()));
    let frontmatter_toml = toml::to_string(&toml::Value::Table(table))?;
    let body = strip_leading_yaml_frontmatter(markdown);
    Ok(format!("---\n{frontmatter_toml}---\n\n{body}"))
}

/// Parse the `title:` field from YAML (`---`) frontmatter.
fn frontmatter_title(markdown: &str) -> Option<String> {
    let content = markdown.trim_start();
    if !content.starts_with("---") {
        return None;
    }
    let after = content.get(3..)?;
    let end = after.find("\n---")?;
    let frontmatter = &after[..end];
    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("title:") {
            let title = rest.trim().trim_matches('"').trim_matches('\'');
            if !title.is_empty() {
                return Some(title.to_string());
            }
        }
    }
    None
}

/// Build a `ResolvedCollection` for a HedgeDoc source given its note ID and
/// the resolved data directory.
pub fn resolved_collection(source_uri: &str, data_dir: &Path) -> ResolvedCollection {
    let slug = slug_for_source_uri(source_uri);
    let source_key = slugify(source_uri);
    let coll_dir = data_dir.join("hedgedoc").join(source_key);
    let db_path = data_dir.join("db").join(format!("{slug}.db"));
    ResolvedCollection {
        name: source_display_name(source_uri),
        slug,
        coll_dir,
        db_path,
    }
}

fn note_file_name(url: &str) -> String {
    let note_id = note_id_from_url(url).unwrap_or_else(|| "note".to_string());
    let stem = slugify(&note_id);
    format!("{}.md", if stem.is_empty() { "note" } else { &stem })
}

/// Fetch a HedgeDoc document, write it to `{rc.coll_dir}/{note}.md`, and
/// return the extracted deck title with file metadata.
pub async fn sync_source(url: &str, rc: &ResolvedCollection) -> Fallible<(String, String)> {
    let markdown = fetch_markdown(url).await?;
    let note_id = note_id_from_url(url).unwrap_or_default();
    let title = extract_title(&markdown, &note_id);
    let deck_name = sanitize_deck_name(&title, &note_id);
    let sync_markdown = wrap_with_deck_frontmatter(&markdown, &deck_name)?;
    let file_name = note_file_name(url);
    std::fs::create_dir_all(&rc.coll_dir)?;
    std::fs::write(rc.coll_dir.join(&file_name), sync_markdown)?;
    Ok((deck_name, file_name))
}

/// Compute `CollectionInfo` for a single HedgeDoc source.
pub fn collection_info_for_source(source: &HedgedocSource) -> CollectionInfo {
    let (total_cards, due_today) =
        match compute_collection_counts(&source.collection.coll_dir, &source.collection.db_path) {
            Ok(counts) => counts,
            Err(e) => {
                log::warn!(
                    "Failed to count cards for HedgeDoc source '{}': {e}",
                    source.source_uri
                );
                (0, 0)
            }
        };
    CollectionInfo {
        name: source.collection.name.clone(),
        slug: source.collection.slug.clone(),
        total_cards,
        due_today,
    }
}

pub async fn build_note(url: &str, collection: &ResolvedCollection) -> Fallible<HedgedocNote> {
    let (deck_name, file_name) = match sync_source(url, collection).await {
        Ok(info) => info,
        Err(e) => {
            let msg = e.to_string();
            log::error!("Initial HedgeDoc sync failed for {url}: {msg}");
            return Ok(HedgedocNote {
                url: url.to_string(),
                deck_name: note_id_from_url(url).unwrap_or_else(|| "note".to_string()),
                file_name: note_file_name(url),
                last_error: Some(msg),
            });
        }
    };

    Ok(HedgedocNote {
        url: url.to_string(),
        deck_name,
        file_name,
        last_error: None,
    })
}

/// Re-derive a `HedgedocSource` from a URL, performing an initial sync for the note.
pub async fn build_source(url: &str, data_dir: &Path) -> Fallible<HedgedocSource> {
    let source_uri = source_uri_from_url(url)
        .ok_or_else(|| crate::error::ErrorReport::new(format!("Cannot derive source URI from URL: {url}")))?;

    let rc = resolved_collection(&source_uri, data_dir);
    std::fs::create_dir_all(&rc.coll_dir)?;
    if let Some(parent) = rc.db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let note = build_note(url, &rc).await?;

    Ok(HedgedocSource {
        source_uri,
        collection: rc,
        notes: vec![note],
    })
}

/// Spawn a background task that periodically re-fetches all HedgeDoc sources.
pub fn spawn_hedgedoc_sync_task(
    hedgedoc_sources: Arc<Mutex<Vec<HedgedocSource>>>,
    collection_infos: Arc<RwLock<Vec<CollectionInfo>>>,
    hedgedoc_last_synced: Arc<Mutex<Option<Timestamp>>>,
    static_collections: Vec<ResolvedCollection>,
    _data_dir: PathBuf,
    poll_interval_minutes: u64,
) {
    if poll_interval_minutes == 0 {
        log::debug!("Periodic HedgeDoc sync disabled (poll_interval_minutes = 0)");
        return;
    }

    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(poll_interval_minutes * 60));
        // Skip the first immediate tick — we already synced on startup.
        ticker.tick().await;

        loop {
            ticker.tick().await;
            log::debug!("Periodic HedgeDoc sync triggered");

            // Collect the URLs we need to sync (without holding the lock during await).
            let entries: Vec<(String, ResolvedCollection)> = {
                let sources = hedgedoc_sources.lock().unwrap();
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
                        let mut sources = hedgedoc_sources.lock().unwrap();
                        if let Some(src) = sources
                            .iter_mut()
                            .find(|s| s.collection.slug == rc.slug)
                        {
                            if let Some(note) = src.notes.iter_mut().find(|n| &n.url == url) {
                                note.deck_name = deck_name;
                                note.file_name = file_name;
                                note.last_error = None;
                            }
                        }
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        log::error!("Periodic HedgeDoc sync failed for {url}: {msg}");
                        let mut sources = hedgedoc_sources.lock().unwrap();
                        for src in sources.iter_mut() {
                            if let Some(note) = src.notes.iter_mut().find(|n| &n.url == url) {
                                note.last_error = Some(msg.clone());
                                break;
                            }
                        }
                    }
                }
            }

            // Refresh the unified collection info list.
            let all_infos = build_combined_infos(
                &static_collections,
                &hedgedoc_sources.lock().unwrap(),
            );
            *collection_infos.write().await = all_infos;
            *hedgedoc_last_synced.lock().unwrap() = Some(Timestamp::now());
            log::debug!("Periodic HedgeDoc sync complete");
        }
    });
}

/// Combine `CollectionInfo` for static (git/directory) and HedgeDoc sources.
pub fn build_combined_infos(
    static_collections: &[ResolvedCollection],
    hedgedoc_sources: &[HedgedocSource],
) -> Vec<CollectionInfo> {
    use crate::cmd::serve::git::refresh_collection_info;

    let mut infos = refresh_collection_info(static_collections);
    for src in hedgedoc_sources {
        infos.push(collection_info_for_source(src));
    }
    infos
}

pub fn all_hedgedoc_entries(hedgedoc_sources: &[HedgedocSource]) -> Vec<HedgedocEntry> {
    hedgedoc_sources
        .iter()
        .flat_map(|s| s.notes.iter().map(|n| HedgedocEntry { url: n.url.clone() }))
        .collect()
}

/// Create a minimal hashcards.toml config file in the current working directory
/// if it doesn't already exist. This is used when adding the first HedgeDoc source
/// in no-config mode.
pub fn create_minimal_config() -> Fallible<PathBuf> {
    let config_path = std::env::current_dir()?.join("hashcards.toml");
    
    if config_path.exists() {
        return Ok(config_path);
    }

    let data_dir = std::env::current_dir()?.join(".hashcards");
    let minimal_config = format!(
        "# hashcards server configuration\n# Auto-generated on first HedgeDoc source add\n\n[server]\ndata_dir = {:?}\n",
        data_dir.to_string_lossy()
    );
    
    std::fs::write(&config_path, minimal_config)?;
    Ok(config_path)
}

/// Write the current set of HedgeDoc URLs back to the TOML config file.
/// All existing config is preserved; only the `[[hedgedoc]]` array is replaced.
pub fn persist_hedgedoc_entries(
    config_path: &Path,
    entries: &[HedgedocEntry],
) -> Fallible<()> {
    let content = std::fs::read_to_string(config_path)?;
    let mut doc: toml::Value = toml::from_str(&content)?;

    let table = doc
        .as_table_mut()
        .ok_or_else(|| crate::error::ErrorReport::new("Config is not a TOML table"))?;

    if entries.is_empty() {
        table.remove("hedgedoc");
    } else {
        let array: Vec<toml::Value> = entries
            .iter()
            .map(|e| {
                let mut t = toml::map::Map::new();
                t.insert("url".to_string(), toml::Value::String(e.url.clone()));
                toml::Value::Table(t)
            })
            .collect();
        table.insert("hedgedoc".to_string(), toml::Value::Array(array));
    }

    let serialized = toml::to_string_pretty(&doc)?;
    std::fs::write(config_path, serialized)?;
    Ok(())
}
