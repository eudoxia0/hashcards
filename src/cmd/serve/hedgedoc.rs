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
/// Query parameters and fragments are ignored.
pub fn note_id_from_url(url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url).ok()?;
    parsed
        .path_segments()?
        .filter(|s| !s.is_empty())
        .last()
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

/// Validate that a HedgeDoc URL is safe to fetch (HTTPS only).
fn validate_hedgedoc_url(url: &str) -> Fallible<()> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|e| crate::error::ErrorReport::new(format!("Invalid HedgeDoc URL `{}`: {}", url, e)))?;
    if parsed.scheme() != "https" {
        return fail(format!("HedgeDoc URLs must use HTTPS (got: {})", url));
    }
    Ok(())
}

/// Build the `/download` URL for a HedgeDoc note, safely appending the path
/// segment without interfering with any query string or fragment.
fn build_download_url(url: &str) -> Fallible<reqwest::Url> {
    let mut parsed = reqwest::Url::parse(url)
        .map_err(|e| crate::error::ErrorReport::new(format!("Invalid HedgeDoc URL `{}`: {}", url, e)))?;
    parsed.set_query(None);
    parsed.set_fragment(None);
    {
        let mut segments = parsed
            .path_segments_mut()
            .map_err(|_| crate::error::ErrorReport::new(format!("Cannot modify path for HedgeDoc URL `{}`", url)))?;
        segments.pop_if_empty();
        segments.push("download");
    }
    Ok(parsed)
}

/// Return the shared HTTP client, initialising it on first use.
/// The client is configured with a 30-second timeout.
fn http_client() -> Fallible<&'static reqwest::Client> {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    // Fast path: already initialised.
    if let Some(c) = CLIENT.get() {
        return Ok(c);
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(crate::error::ErrorReport::from)?;
    // get_or_init is stable; if another thread raced us, the already-stored
    // client is returned and our freshly built one is dropped (harmless).
    Ok(CLIENT.get_or_init(|| client))
}

/// Fetch raw markdown from a HedgeDoc note URL.
/// Appends `/download` to the note URL path to get the raw markdown.
/// Only HTTPS URLs are accepted. Requests time out after 30 seconds.
pub async fn fetch_markdown(url: &str) -> Fallible<String> {
    validate_hedgedoc_url(url)?;
    let download_url = build_download_url(url)?;
    let client = http_client()?;
    let response = client.get(download_url.clone()).send().await?;
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

fn split_yaml_frontmatter(markdown: &str) -> Option<(&str, &str)> {
    let content = markdown.trim_start();
    if !content.starts_with("---\n") && !content.starts_with("---\r\n") {
        return None;
    }

    let after_start = if content.starts_with("---\r\n") { 5 } else { 4 };
    let after = &content[after_start..];
    
    let mut current = 0;
    while let Some(idx) = after[current..].find("\n---") {
        let abs_idx = current + idx;
        let next_char_idx = abs_idx + 4;
        let is_end = next_char_idx == after.len() 
            || after[next_char_idx..].starts_with('\n') 
            || after[next_char_idx..].starts_with("\r\n");
            
        if is_end {
            let mut fm_end = abs_idx;
            if fm_end > 0 && after.as_bytes()[fm_end - 1] == b'\r' {
                fm_end -= 1;
            }
            let frontmatter = &after[..fm_end];
            let skip = if after[next_char_idx..].starts_with("\r\n") {
                2
            } else if after[next_char_idx..].starts_with('\n') {
                1
            } else {
                0
            };
            return Some((frontmatter, &after[next_char_idx + skip..]));
        }
        current = abs_idx + 4;
    }
    None
}

fn strip_leading_yaml_frontmatter(markdown: &str) -> &str {
    match split_yaml_frontmatter(markdown) {
        Some((_, body)) => body.trim_start_matches(|c| c == '\n' || c == '\r'),
        None => markdown,
    }
}

fn wrap_with_deck_frontmatter(markdown: &str, deck_name: &str) -> Fallible<String> {
    let mut table = toml::map::Map::new();
    table.insert("name".to_string(), toml::Value::String(deck_name.to_string()));
    let mut frontmatter_toml = toml::to_string(&toml::Value::Table(table))?;
    // Ensure the TOML block ends with a newline so the closing `---` stays on
    // its own line and the frontmatter parser can find it.
    if !frontmatter_toml.ends_with('\n') {
        frontmatter_toml.push('\n');
    }
    let body = strip_leading_yaml_frontmatter(markdown);
    Ok(format!("---\n{frontmatter_toml}---\n\n{body}"))
}

/// Parse the `title:` field from YAML (`---`) frontmatter.
fn frontmatter_title(markdown: &str) -> Option<String> {
    let (frontmatter, _) = split_yaml_frontmatter(markdown)?;
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
    tokio::fs::create_dir_all(&rc.coll_dir).await?;
    // Atomic write: write to a temp file then rename so concurrent readers
    // never see a partially-written note file.
    let final_path = rc.coll_dir.join(&file_name);
    let tmp_path = rc.coll_dir.join(format!(".{}.tmp", file_name));
    tokio::fs::write(&tmp_path, sync_markdown).await?;
    // On Unix, rename over an existing file is atomic.
    // On Windows, rename fails if the destination exists, so remove it first
    // (non-atomic, but best-effort on that platform).
    #[cfg(windows)]
    if tokio::fs::metadata(&final_path).await.is_ok() {
        if let Err(e) = tokio::fs::remove_file(&final_path).await {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return Err(e.into());
        }
    }
    if let Err(e) = tokio::fs::rename(&tmp_path, &final_path).await {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        return Err(e.into());
    }
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
    tokio::fs::create_dir_all(&rc.coll_dir).await?;
    if let Some(parent) = rc.db_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
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

            let mut any_success = false;

            for (url, rc) in &entries {
                match sync_source(url, rc).await {
                    Ok((deck_name, file_name)) => {
                        any_success = true;
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

            // Snapshot sources before releasing the lock, then do FS/DB work outside it.
            let hedgedoc_snapshot = hedgedoc_sources.lock().unwrap().clone();
            let all_infos = build_combined_infos(&static_collections, &hedgedoc_snapshot);
            *collection_infos.write().await = all_infos;
            if any_success {
                *hedgedoc_last_synced.lock().unwrap() = Some(Timestamp::now());
            }
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
/// if it doesn't already exist. `data_dir` should be the actual data directory
/// already in use by the running server so the generated config matches it.
pub fn create_minimal_config(data_dir: &Path) -> Fallible<PathBuf> {
    let config_path = std::env::current_dir()?.join("hashcards.toml");

    if config_path.exists() {
        return Ok(config_path);
    }

    let minimal_config = format!(
        "# hashcards server configuration\n# Auto-generated on first HedgeDoc source add\n\n[server]\ndata_dir = {:?}\n",
        data_dir.to_string_lossy()
    );

    std::fs::write(&config_path, minimal_config)?;
    Ok(config_path)
}

/// Write the current set of HedgeDoc URLs back to the TOML config file.
/// Other config keys are preserved by value, but comments and key ordering
/// in the file are not guaranteed to survive the round-trip.
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
    // Atomic write: write to a temp file in the same directory then rename,
    // so a crash mid-write cannot corrupt the config.
    static WRITE_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = WRITE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = config_path.parent().unwrap_or(Path::new("."));
    let tmp_path = dir.join(format!(".hashcards-config-{}-{}.tmp", std::process::id(), n));
    std::fs::write(&tmp_path, serialized)?;
    // On Unix, rename over an existing file is atomic.
    // On Windows, rename fails if the destination exists, so remove it first
    // (non-atomic, but best-effort on that platform).
    #[cfg(windows)]
    if config_path.exists() {
        if let Err(e) = std::fs::remove_file(config_path) {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(e.into());
        }
    }
    if let Err(e) = std::fs::rename(&tmp_path, config_path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn note_id_strips_query_and_fragment() {
        assert_eq!(
            note_id_from_url("https://notes.example.com/abc123?foo=1"),
            Some("abc123".to_string())
        );
        assert_eq!(
            note_id_from_url("https://notes.example.com/abc123#section"),
            Some("abc123".to_string())
        );
        assert_eq!(
            note_id_from_url("https://notes.example.com/abc123"),
            Some("abc123".to_string())
        );
        assert_eq!(
            note_id_from_url("https://notes.example.com/abc123/"),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn build_download_url_basic() {
        let url = build_download_url("https://notes.example.com/abc123").unwrap();
        assert_eq!(url.as_str(), "https://notes.example.com/abc123/download");
    }

    #[test]
    fn build_download_url_strips_query_and_fragment() {
        let url = build_download_url("https://notes.example.com/abc123?foo=1#bar").unwrap();
        assert_eq!(url.as_str(), "https://notes.example.com/abc123/download");
    }

    #[test]
    fn build_download_url_trailing_slash() {
        let url = build_download_url("https://notes.example.com/abc123/").unwrap();
        assert_eq!(url.as_str(), "https://notes.example.com/abc123/download");
    }

    #[test]
    fn validate_hedgedoc_url_rejects_http() {
        assert!(validate_hedgedoc_url("http://notes.example.com/abc123").is_err());
    }

    #[test]
    fn validate_hedgedoc_url_accepts_https() {
        assert!(validate_hedgedoc_url("https://notes.example.com/abc123").is_ok());
    }

    fn write_toml(path: &Path, content: &str) {
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn persist_adds_hedgedoc_array() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("hashcards.toml");
        write_toml(&config_path, "[server]\ndata_dir = \"/tmp\"\n");

        let entries = vec![
            HedgedocEntry { url: "https://notes.example.com/doc1".to_string() },
            HedgedocEntry { url: "https://notes.example.com/doc2".to_string() },
        ];
        persist_hedgedoc_entries(&config_path, &entries).unwrap();

        let content = std::fs::read_to_string(&config_path).unwrap();
        let value: toml::Value = toml::from_str(&content).unwrap();
        let table = value.as_table().unwrap();
        assert!(table.contains_key("server"));
        let arr = table["hedgedoc"].as_array().unwrap();
        let urls: Vec<&str> = arr
            .iter()
            .map(|v| v.as_table().unwrap()["url"].as_str().unwrap())
            .collect();
        assert_eq!(urls, vec!["https://notes.example.com/doc1", "https://notes.example.com/doc2"]);
    }

    #[test]
    fn persist_replaces_existing_array() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("hashcards.toml");
        write_toml(&config_path, "[[hedgedoc]]\nurl = \"https://old.example.com/old\"\n[server]\ndata_dir = \"/tmp\"\n");

        let entries = vec![HedgedocEntry { url: "https://new.example.com/new".to_string() }];
        persist_hedgedoc_entries(&config_path, &entries).unwrap();

        let content = std::fs::read_to_string(&config_path).unwrap();
        let value: toml::Value = toml::from_str(&content).unwrap();
        let arr = value.as_table().unwrap()["hedgedoc"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0].as_table().unwrap()["url"].as_str().unwrap(), "https://new.example.com/new");
    }

    #[test]
    fn persist_removes_array_when_empty() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("hashcards.toml");
        write_toml(&config_path, "[[hedgedoc]]\nurl = \"https://example.com/doc\"\n[server]\ndata_dir = \"/tmp\"\n");

        persist_hedgedoc_entries(&config_path, &[]).unwrap();

        let content = std::fs::read_to_string(&config_path).unwrap();
        let value: toml::Value = toml::from_str(&content).unwrap();
        let table = value.as_table().unwrap();
        assert!(!table.contains_key("hedgedoc"));
        assert!(table.contains_key("server"));
    }
}
