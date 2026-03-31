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

/// Build the collection slug for a HedgeDoc source.
pub fn slug_for_note_id(note_id: &str) -> String {
    format!("hedgedoc-{}", slugify(note_id))
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
pub fn resolved_collection(note_id: &str, name: &str, data_dir: &Path) -> ResolvedCollection {
    let slug = slug_for_note_id(note_id);
    let coll_dir = data_dir.join("hedgedoc").join(note_id);
    let db_path = data_dir.join("db").join(format!("{slug}.db"));
    ResolvedCollection {
        name: name.to_string(),
        slug,
        coll_dir,
        db_path,
    }
}

/// Fetch a HedgeDoc document, write it to `{rc.coll_dir}/content.md`, and
/// return the extracted title.
pub async fn sync_source(url: &str, rc: &ResolvedCollection) -> Fallible<String> {
    let markdown = fetch_markdown(url).await?;
    let note_id = note_id_from_url(url).unwrap_or_default();
    let title = extract_title(&markdown, &note_id);
    std::fs::create_dir_all(&rc.coll_dir)?;
    std::fs::write(rc.coll_dir.join("content.md"), &markdown)?;
    Ok(title)
}

/// Compute `CollectionInfo` for a single HedgeDoc source.
pub fn collection_info_for_source(source: &HedgedocSource) -> CollectionInfo {
    let (total_cards, due_today) =
        match compute_collection_counts(&source.collection.coll_dir, &source.collection.db_path) {
            Ok(counts) => counts,
            Err(e) => {
                log::warn!(
                    "Failed to count cards for HedgeDoc source '{}': {e}",
                    source.url
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

/// Re-derive a `HedgedocSource` from a URL, performing an initial sync.
/// Returns the source with a fresh name derived from the document title.
pub async fn build_source(url: &str, data_dir: &Path) -> Fallible<HedgedocSource> {
    let note_id = note_id_from_url(url)
        .ok_or_else(|| crate::error::ErrorReport::new(format!("Cannot derive note ID from URL: {url}")))?;

    // Temporary placeholder name for directory setup
    let rc_temp = resolved_collection(&note_id, &note_id, data_dir);
    std::fs::create_dir_all(&rc_temp.coll_dir)?;
    if let Some(parent) = rc_temp.db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let title = match sync_source(url, &rc_temp).await {
        Ok(t) => t,
        Err(e) => {
            let msg = e.to_string();
            log::error!("Initial HedgeDoc sync failed for {url}: {msg}");
            // Return a source with the error recorded; cards will be 0.
            return Ok(HedgedocSource {
                url: url.to_string(),
                collection: rc_temp,
                last_error: Some(msg),
            });
        }
    };

    let rc = resolved_collection(&note_id, &title, data_dir);
    Ok(HedgedocSource {
        url: url.to_string(),
        collection: rc,
        last_error: None,
    })
}

/// Spawn a background task that periodically re-fetches all HedgeDoc sources.
pub fn spawn_hedgedoc_sync_task(
    hedgedoc_sources: Arc<Mutex<Vec<HedgedocSource>>>,
    collection_infos: Arc<RwLock<Vec<CollectionInfo>>>,
    hedgedoc_last_synced: Arc<Mutex<Option<Timestamp>>>,
    static_collections: Vec<ResolvedCollection>,
    data_dir: PathBuf,
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
            let entries: Vec<(String, String)> = {
                let sources = hedgedoc_sources.lock().unwrap();
                sources
                    .iter()
                    .map(|s| (s.url.clone(), s.collection.slug.clone()))
                    .collect()
            };

            for (url, _slug) in &entries {
                let note_id = match note_id_from_url(url) {
                    Some(id) => id,
                    None => continue,
                };
                let rc_tmp = resolved_collection(&note_id, "", &data_dir);
                match sync_source(url, &rc_tmp).await {
                    Ok(title) => {
                        let mut sources = hedgedoc_sources.lock().unwrap();
                        if let Some(src) = sources.iter_mut().find(|s| &s.url == url) {
                            src.collection.name = title;
                            src.last_error = None;
                        }
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        log::error!("Periodic HedgeDoc sync failed for {url}: {msg}");
                        let mut sources = hedgedoc_sources.lock().unwrap();
                        if let Some(src) = sources.iter_mut().find(|s| &s.url == url) {
                            src.last_error = Some(msg);
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
