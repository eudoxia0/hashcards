use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use tokio::sync::RwLock;

use crate::cmd::drill::cache::Cache;
use crate::cmd::drill::server::AnswerControls;
use crate::cmd::drill::state::MutableState;
use crate::cmd::serve::config::ResolvedCollection;
use crate::cmd::serve::config::ResolvedServeConfig;
use crate::types::timestamp::Timestamp;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<ResolvedServeConfig>,
    pub collections: Arc<RwLock<Vec<CollectionInfo>>>,
    pub sessions: Arc<Mutex<HashMap<String, DrillSession>>>,
    pub last_synced: Arc<Mutex<Option<Timestamp>>>,
    pub hedgedoc_sources: Arc<Mutex<Vec<HedgedocSource>>>,
    pub hedgedoc_last_synced: Arc<Mutex<Option<Timestamp>>>,
    pub config_path: Arc<Mutex<Option<PathBuf>>>,
}

pub struct CollectionInfo {
    pub name: String,
    pub slug: String,
    pub total_cards: usize,
    pub due_today: usize,
}

/// A HedgeDoc markdown endpoint used as a collection source.
pub struct HedgedocSource {
    pub source_uri: String,
    pub collection: ResolvedCollection,
    pub notes: Vec<HedgedocNote>,
}

pub struct HedgedocNote {
    pub url: String,
    pub deck_name: String,
    pub file_name: String,
    pub last_error: Option<String>,
}

pub struct DrillSession {
    pub directory: PathBuf,
    pub macros: Vec<(String, String)>,
    pub total_cards: usize,
    pub session_started_at: Timestamp,
    pub answer_controls: AnswerControls,
    pub mutable: MutableState,
}

impl DrillSession {
    pub fn new(
        directory: PathBuf,
        macros: Vec<(String, String)>,
        cards: Vec<crate::types::card::Card>,
        cache: Cache,
        session_started_at: Timestamp,
        answer_controls: AnswerControls,
        db: crate::db::Database,
    ) -> Self {
        let total_cards = cards.len();
        Self {
            directory,
            macros,
            total_cards,
            session_started_at,
            answer_controls,
            mutable: MutableState {
                reveal: false,
                db,
                cache,
                cards,
                reviews: Vec::new(),
                finished_at: None,
            },
        }
    }
}
