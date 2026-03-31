use std::env::current_dir;
use std::fs::read_to_string;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

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
    #[serde(rename = "collection", default)]
    pub collections: Vec<CollectionEntry>,
    #[serde(rename = "hedgedoc", default)]
    pub hedgedoc: Vec<HedgedocEntry>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct HedgedocEntry {
    pub url: String,
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
    pub repo_url: Option<String>,
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

#[derive(Deserialize, Serialize, Clone)]
pub struct CollectionEntry {
    pub name: String,
    pub path: String,
}

impl CollectionEntry {
    pub fn slug(&self) -> String {
        slugify(&self.path)
    }
}

pub fn slugify(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect()
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

#[derive(Clone)]
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
    /// Set when loaded from a TOML file; None when using directory arguments.
    pub data_dir: Option<PathBuf>,
    /// Config file path; needed to persist UI changes back to disk.
    pub config_path: Option<PathBuf>,
    /// HedgeDoc source URLs loaded from the config file.
    pub hedgedoc_entries: Vec<HedgedocEntry>,
}

impl ResolvedServeConfig {
    pub fn from_toml(config: ServeConfig) -> Fallible<Self> {
        let data_dir = {
            let p = PathBuf::from(&config.server.data_dir);
            if p.is_absolute() {
                p
            } else {
                current_dir()?.join(p)
            }
        };
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

        let git = match config.git {
            None => None,
            Some(g) => match g.repo_url {
                Some(repo_url) => Some(ResolvedGit {
                    repo_url,
                    branch: g.branch,
                    poll_interval_minutes: g.poll_interval_minutes,
                    repo_dir: repo_dir.clone(),
                    db_dir: db_dir.clone(),
                }),
                None => return fail("configuration error: [git] section is present but `repo_url` is missing"),
            },
        };

        Ok(Self {
            host: config.server.host,
            port: config.server.port,
            git,
            defaults: config.defaults,
            collections,
            data_dir: Some(data_dir),
            config_path: None,
            hedgedoc_entries: config.hedgedoc,
        })
    }

    pub fn with_config_path(mut self, path: PathBuf) -> Self {
        self.config_path = Some(path);
        self
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

            let slug = slugify(&name);
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
            data_dir: None,
            config_path: None,
            hedgedoc_entries: Vec::new(),
        })
    }
}
