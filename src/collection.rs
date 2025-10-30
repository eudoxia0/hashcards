// Copyright 2025 Fernando Borretti
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::env::current_dir;
use std::fs::read_to_string;
use std::path::PathBuf;
use std::time::Instant;

use crate::db::Database;
use crate::error::ErrorReport;
use crate::error::Fallible;
use crate::error::fail;
use crate::media::validate::validate_media_files;
use crate::parser::parse_deck;
use crate::types::card::Card;

pub struct Collection {
    pub directory: PathBuf,
    pub db: Database,
    pub cards: Vec<Card>,
    pub macros: Vec<(String, String)>,
}

impl Collection {
    pub fn new(directory: Option<String>) -> Fallible<Self> {
        let directory: PathBuf = match directory {
            Some(dir) => PathBuf::from(dir),
            None => current_dir()?,
        };
        let directory = if directory.exists() {
            directory.canonicalize()?
        } else {
            return fail("directory does not exist.");
        };

        let db_path: PathBuf = directory.join("hashcards.db");
        let db_path: &str = db_path
            .to_str()
            .ok_or_else(|| ErrorReport::new("invalid path"))?;
        let db: Database = Database::new(db_path)?;

        let macros = {
            let mut macros = Vec::new();
            let macros_path = directory.join("macros.tex");
            if macros_path.exists() {
                let content = read_to_string(macros_path)?;
                for line in content.lines() {
                    // Skip lines starting with '%'.
                    if !line.trim_start().starts_with('%') {
                        if let Some((name, definition)) = line.split_once(' ') {
                            macros.push((name.to_string(), definition.to_string()));
                        }
                    }
                }
            }
            macros
        };

        let cards = {
            log::debug!("Loading deck...");
            let start = Instant::now();
            let cards = parse_deck(&directory)?;
            let end = Instant::now();
            let duration = end.duration_since(start).as_millis();
            log::debug!("Deck loaded in {duration}ms.");
            cards
        };

        // Validate media files
        validate_media_files(&cards, &directory)?;

        Ok(Self {
            directory,
            db,
            cards,
            macros,
        })
    }
}
