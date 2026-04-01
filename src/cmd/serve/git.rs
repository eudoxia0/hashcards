use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use tokio::process::Command;
use tokio::sync::RwLock;
use tokio::time::interval;

use crate::cmd::serve::config::ResolvedCollection;
use crate::cmd::serve::config::ResolvedGit;
use crate::cmd::serve::state::CollectionInfo;
use crate::cmd::serve::state::HedgedocSource;
use crate::error::Fallible;
use crate::error::fail;
use crate::types::timestamp::Timestamp;

pub async fn clone_or_pull(repo_url: &str, branch: &str, target_dir: &Path) -> Fallible<()> {
    if target_dir.join(".git").exists() {
        log::debug!("Checking out branch {} in {}", branch, target_dir.display());
        let checkout = Command::new("git")
            .env("GIT_TERMINAL_PROMPT", "0")
            .args(["checkout", branch])
            .current_dir(target_dir)
            .output()
            .await?;
        if !checkout.status.success() {
            let stderr = String::from_utf8_lossy(&checkout.stderr);
            return fail(format!("git checkout {branch} failed: {stderr}"));
        }
        log::debug!("Pulling latest changes in {}", target_dir.display());
        let pull = Command::new("git")
            .env("GIT_TERMINAL_PROMPT", "0")
            .args(["pull", "--ff-only", "origin", branch])
            .current_dir(target_dir)
            .output()
            .await?;
        if !pull.status.success() {
            let stderr = String::from_utf8_lossy(&pull.stderr);
            return fail(format!("git pull origin {branch} failed: {stderr}"));
        }
    } else {
        log::debug!("Cloning {} into {}", repo_url, target_dir.display());
        let output = Command::new("git")
            .env("GIT_TERMINAL_PROMPT", "0")
            .args(["clone", "--branch", branch, "--single-branch", repo_url])
            .arg(target_dir)
            .output()
            .await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return fail(format!("git clone failed: {stderr}"));
        }
    }
    Ok(())
}

pub fn refresh_collection_info(collections: &[ResolvedCollection]) -> Vec<CollectionInfo> {
    let mut infos = Vec::new();
    for rc in collections {
        let (total_cards, due_today) = match compute_collection_counts(&rc.coll_dir, &rc.db_path) {
            Ok(counts) => counts,
            Err(e) => {
                log::warn!("Failed to load collection '{}': {e}", rc.name);
                (0, 0)
            }
        };

        infos.push(CollectionInfo {
            name: rc.name.clone(),
            slug: rc.slug.clone(),
            total_cards,
            due_today,
        });
    }
    infos
}

pub fn compute_collection_counts(coll_dir: &Path, db_path: &Path) -> Fallible<(usize, usize)> {
    use crate::collection::Collection;
    use crate::types::date::Date;

    if !coll_dir.exists() {
        return Ok((0, 0));
    }

    let collection = Collection::with_db_path(coll_dir.to_path_buf(), db_path.to_path_buf())?;
    let total_cards = collection.cards.len();

    let today: Date = Timestamp::now().date();

    // Sync new cards to DB
    let db_hashes = collection.db.card_hashes()?;
    let now = Timestamp::now();
    for card in collection.cards.iter() {
        if !db_hashes.contains(&card.hash()) {
            collection.db.insert_card(card.hash(), now)?;
        }
    }

    let due_hashes = collection.db.due_today(today)?;
    let due_today = collection
        .cards
        .iter()
        .filter(|c| due_hashes.contains(&c.hash()))
        .count();

    Ok((total_cards, due_today))
}

pub fn spawn_sync_task(
    git: ResolvedGit,
    collections: Vec<ResolvedCollection>,
    collection_infos: Arc<RwLock<Vec<CollectionInfo>>>,
    last_synced: Arc<Mutex<Option<Timestamp>>>,
    hedgedoc_sources: Arc<Mutex<Vec<HedgedocSource>>>,
) {
    if git.poll_interval_minutes == 0 {
        log::debug!("Periodic git sync disabled (poll_interval_minutes = 0)");
        return;
    }

    let poll_minutes = git.poll_interval_minutes;
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(poll_minutes * 60));
        // Skip the first immediate tick (we already synced on startup)
        ticker.tick().await;

        loop {
            ticker.tick().await;
            log::debug!("Periodic git sync triggered");
            if let Err(e) =
                clone_or_pull(&git.repo_url, &git.branch, &git.repo_dir).await
            {
                log::error!("Periodic git sync failed: {e}");
                continue;
            }
            let static_infos = refresh_collection_info(&collections);
            // Snapshot only the paths needed, then release the lock before
            // doing filesystem/DB work to avoid blocking other handlers.
            let source_paths: Vec<(String, String, std::path::PathBuf, std::path::PathBuf)> = {
                let sources = hedgedoc_sources.lock().unwrap();
                sources
                    .iter()
                    .map(|s| (
                        s.collection.name.clone(),
                        s.collection.slug.clone(),
                        s.collection.coll_dir.clone(),
                        s.collection.db_path.clone(),
                    ))
                    .collect()
            };
            let hedgedoc_infos: Vec<CollectionInfo> = match tokio::task::spawn_blocking(move || {
                source_paths
                    .into_iter()
                    .map(|(name, slug, coll_dir, db_path)| {
                        let (total_cards, due_today) = match compute_collection_counts(&coll_dir, &db_path) {
                            Ok(counts) => counts,
                            Err(e) => {
                                log::warn!(
                                    "Failed to compute HedgeDoc collection counts for '{}' (slug: '{}', dir: '{}', db: '{}'): {e}",
                                    name,
                                    slug,
                                    coll_dir.display(),
                                    db_path.display(),
                                );
                                (0, 0)
                            }
                        };
                        CollectionInfo { name, slug, total_cards, due_today }
                    })
                    .collect::<Vec<CollectionInfo>>()
            })
            .await
            {
                Ok(infos) => infos,
                Err(e) => {
                    log::error!("Failed to join HedgeDoc collection counts task: {e}");
                    Vec::new()
                }
            };
            let mut combined = static_infos;
            combined.extend(hedgedoc_infos);
            *collection_infos.write().await = combined;
            *last_synced.lock().unwrap() = Some(Timestamp::now());
            log::debug!("Periodic git sync complete");
        }
    });
}
