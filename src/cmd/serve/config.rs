use std::env::current_dir;
use std::fs::read_to_string;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;

use crate::cmd::drill::server::AnswerControls;
use crate::error::Fallible;
use crate::error::fail;

// --- TOML deserialization structs ---

#[derive(Deserialize)]
pub struct ServeConfig {
    pub server: ServerSection,
    #[serde(default)]
    pub git: Option<GitSection>,
    #[serde(default)]
    pub defaults: DefaultsSection,
    #[serde(rename = "collection")]
    pub collections: Vec<CollectionEntry>,
}

#[derive(Deserialize)]
pub struct ServerSection {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub data_dir: String,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8000
}

#[derive(Deserialize)]
pub struct GitSection {
    pub repo_url: String,
    #[serde(default = "default_branch")]
    pub branch: String,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_minutes: u64,
}

fn default_branch() -> String {
    "main".to_string()
}

fn default_poll_interval() -> u64 {
    30
}

#[derive(Deserialize)]
pub struct DefaultsSection {
    #[serde(default = "default_answer_controls")]
    pub answer_controls: AnswerControlsConfig,
    #[serde(default = "default_true")]
    pub bury_siblings: bool,
}

impl Default for DefaultsSection {
    fn default() -> Self {
        Self {
            answer_controls: default_answer_controls(),
            bury_siblings: true,
        }
    }
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum AnswerControlsConfig {
    Full,
    Binary,
}

fn default_answer_controls() -> AnswerControlsConfig {
    AnswerControlsConfig::Full
}

fn default_true() -> bool {
    true
}

impl From<AnswerControlsConfig> for AnswerControls {
    fn from(config: AnswerControlsConfig) -> Self {
        match config {
            AnswerControlsConfig::Full => AnswerControls::Full,
            AnswerControlsConfig::Binary => AnswerControls::Binary,
        }
    }
}

#[derive(Deserialize, Clone)]
pub struct CollectionEntry {
    pub name: String,
    pub path: String,
}

impl CollectionEntry {
    pub fn slug(&self) -> String {
        self.path.replace('/', "-")
    }
}

pub fn load_config(path: &Path) -> Fallible<ServeConfig> {
    let content = read_to_string(path)?;
    let config: ServeConfig = toml::from_str(&content)?;
    Ok(config)
}

// --- Resolved runtime config ---

pub struct ResolvedGit {
    pub repo_url: String,
    pub branch: String,
    pub poll_interval_minutes: u64,
    pub repo_dir: PathBuf,
    pub db_dir: PathBuf,
}

pub struct ResolvedCollection {
    pub name: String,
    pub slug: String,
    pub coll_dir: PathBuf,
    pub db_path: PathBuf,
}

pub struct ResolvedServeConfig {
    pub host: String,
    pub port: u16,
    pub git: Option<ResolvedGit>,
    pub defaults: DefaultsSection,
    pub collections: Vec<ResolvedCollection>,
}

impl ResolvedServeConfig {
    pub fn from_toml(config: ServeConfig) -> Self {
        let data_dir = PathBuf::from(&config.server.data_dir);
        let repo_dir = data_dir.join("repo");
        let db_dir = data_dir.join("db");

        let collections = config
            .collections
            .iter()
            .map(|entry| {
                let slug = entry.slug();
                ResolvedCollection {
                    name: entry.name.clone(),
                    coll_dir: repo_dir.join(&entry.path),
                    db_path: db_dir.join(format!("{slug}.db")),
                    slug,
                }
            })
            .collect();

        let git = config.git.map(|g| ResolvedGit {
            repo_url: g.repo_url,
            branch: g.branch,
            poll_interval_minutes: g.poll_interval_minutes,
            repo_dir,
            db_dir,
        });

        Self {
            host: config.server.host,
            port: config.server.port,
            git,
            defaults: config.defaults,
            collections,
        }
    }

    pub fn from_directories(
        directories: Vec<String>,
        host: String,
        port: u16,
    ) -> Fallible<Self> {
        let base = current_dir()?;
        let mut collections = Vec::new();

        for dir_str in &directories {
            let dir = base.join(dir_str);
            if !dir.exists() {
                return fail(format!("directory does not exist: {dir_str}"));
            }
            let dir = dir.canonicalize()?;

            let name = dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| dir_str.clone());

            let slug = name.replace('/', "-");
            let db_path = dir.join("hashcards.db");

            collections.push(ResolvedCollection {
                name,
                slug,
                coll_dir: dir,
                db_path,
            });
        }

        Ok(Self {
            host,
            port,
            git: None,
            defaults: DefaultsSection::default(),
            collections,
        })
    }
}
