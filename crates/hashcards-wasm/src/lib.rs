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

//! WASM bindings for hashcards - runs the spaced repetition system in the browser.

use std::collections::HashMap;

use wasm_bindgen::prelude::*;
use web_sys::console;

use hashcards_core::fsrs::Grade;
use hashcards_core::parser::parse_decks;
use hashcards_core::rng::{TinyRng, shuffle};
use hashcards_core::types::card::Card;
use hashcards_core::types::date::Date;
use hashcards_core::types::performance::{Performance, update_performance};
use hashcards_core::types::timestamp::Timestamp;

mod storage;

use storage::Storage;

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    console::log_1(&"hashcards WASM initialized".into());
}

/// The main application state managed from JavaScript.
#[wasm_bindgen]
pub struct HashcardsApp {
    /// All parsed cards
    cards: Vec<Card>,
    /// Cards remaining in the current session
    session_cards: Vec<Card>,
    /// Performance data for each card (by hash)
    performance: HashMap<String, Performance>,
    /// Storage backend
    storage: Storage,
    /// Whether the current card is revealed
    revealed: bool,
    /// LaTeX macros
    macros: Vec<(String, String)>,
    /// Map from original media path to blob URL
    media_urls: HashMap<String, String>,
    /// Total cards in session (for progress)
    total_session_cards: usize,
    /// Reviews performed in this session
    reviews_this_session: usize,
}

#[wasm_bindgen]
impl HashcardsApp {
    /// Create a new HashcardsApp instance.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            cards: Vec::new(),
            session_cards: Vec::new(),
            performance: HashMap::new(),
            storage: Storage::new(),
            revealed: false,
            macros: Vec::new(),
            media_urls: HashMap::new(),
            total_session_cards: 0,
            reviews_this_session: 0,
        }
    }

    /// Load cards from markdown content.
    /// Takes an array of [filename, content] pairs.
    #[wasm_bindgen]
    pub fn load_cards(&mut self, files_json: &str) -> Result<usize, JsValue> {
        let files: Vec<(String, String)> = serde_json::from_str(files_json)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse files JSON: {}", e)))?;

        let files_iter = files
            .iter()
            .map(|(name, content)| (name.as_str(), content.as_str()));

        self.cards = parse_decks(files_iter)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse cards: {}", e)))?;

        // Load performance data from storage
        self.load_performance()?;

        Ok(self.cards.len())
    }

    /// Register a media file URL mapping.
    #[wasm_bindgen]
    pub fn register_media(&mut self, original_path: &str, blob_url: &str) {
        self.media_urls
            .insert(original_path.to_string(), blob_url.to_string());
    }

    /// Set LaTeX macros from a JSON object.
    #[wasm_bindgen]
    pub fn set_macros(&mut self, macros_json: &str) -> Result<(), JsValue> {
        let macros: HashMap<String, String> = serde_json::from_str(macros_json)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse macros: {}", e)))?;
        self.macros = macros.into_iter().collect();
        Ok(())
    }

    /// Get macros as a JavaScript object string.
    #[wasm_bindgen]
    pub fn get_macros_js(&self) -> String {
        let mut js = String::from("{\n");
        for (name, def) in &self.macros {
            let name_escaped = name.replace('\\', "\\\\").replace('`', "\\`");
            let def_escaped = def.replace('\\', "\\\\").replace('`', "\\`");
            js.push_str(&format!("  \"{}\": \"{}\",\n", name_escaped, def_escaped));
        }
        js.push('}');
        js
    }

    /// Start a new drilling session.
    /// Returns the number of cards due today.
    #[wasm_bindgen]
    pub fn start_session(
        &mut self,
        today_str: &str,
        do_shuffle: bool,
        card_limit: Option<usize>,
        new_card_limit: Option<usize>,
    ) -> Result<usize, JsValue> {
        let today = Date::try_from(today_str.to_string())
            .map_err(|e| JsValue::from_str(&format!("Invalid date: {}", e)))?;

        // Find cards due today
        let mut due_cards: Vec<Card> = self
            .cards
            .iter()
            .filter(|card| {
                let hash = card.hash().to_hex();
                match self.performance.get(&hash) {
                    None | Some(Performance::New) => true, // New cards are always due
                    Some(Performance::Reviewed(rp)) => rp.due_date <= today,
                }
            })
            .cloned()
            .collect();

        // Apply new card limit
        if let Some(limit) = new_card_limit {
            let mut new_count = 0;
            due_cards.retain(|card| {
                let hash = card.hash().to_hex();
                let is_new = matches!(self.performance.get(&hash), None | Some(Performance::New));
                if is_new {
                    if new_count < limit {
                        new_count += 1;
                        true
                    } else {
                        false
                    }
                } else {
                    true
                }
            });
        }

        // Apply card limit
        if let Some(limit) = card_limit {
            due_cards.truncate(limit);
        }

        // Shuffle if requested
        if do_shuffle && !due_cards.is_empty() {
            let seed = js_sys::Date::now() as u64;
            let mut rng = TinyRng::from_seed(seed);
            due_cards = shuffle(due_cards, &mut rng);
        }

        self.total_session_cards = due_cards.len();
        self.session_cards = due_cards;
        self.revealed = false;
        self.reviews_this_session = 0;

        Ok(self.session_cards.len())
    }

    /// Check if there are more cards in the session.
    #[wasm_bindgen]
    pub fn has_cards(&self) -> bool {
        !self.session_cards.is_empty()
    }

    /// Get the number of remaining cards.
    #[wasm_bindgen]
    pub fn remaining_cards(&self) -> usize {
        self.session_cards.len()
    }

    /// Get total cards in the session.
    #[wasm_bindgen]
    pub fn total_cards(&self) -> usize {
        self.total_session_cards
    }

    /// Get progress (cards reviewed / total).
    #[wasm_bindgen]
    pub fn progress(&self) -> f64 {
        if self.total_session_cards == 0 {
            1.0
        } else {
            (self.total_session_cards - self.session_cards.len()) as f64
                / self.total_session_cards as f64
        }
    }

    /// Check if the current card is revealed.
    #[wasm_bindgen]
    pub fn is_revealed(&self) -> bool {
        self.revealed
    }

    /// Reveal the current card's answer.
    #[wasm_bindgen]
    pub fn reveal(&mut self) {
        self.revealed = true;
    }

    /// Get the current card's front HTML.
    #[wasm_bindgen]
    pub fn current_front_html(&self) -> Result<String, JsValue> {
        let card = self
            .session_cards
            .last()
            .ok_or_else(|| JsValue::from_str("No cards in session"))?;

        let url_rewriter = |url: &str| -> String {
            self.media_urls
                .get(url)
                .cloned()
                .unwrap_or_else(|| url.to_string())
        };

        card.html_front(Some(&url_rewriter))
            .map_err(|e| JsValue::from_str(&format!("Failed to render card: {}", e)))
    }

    /// Get the current card's back HTML.
    #[wasm_bindgen]
    pub fn current_back_html(&self) -> Result<String, JsValue> {
        let card = self
            .session_cards
            .last()
            .ok_or_else(|| JsValue::from_str("No cards in session"))?;

        let url_rewriter = |url: &str| -> String {
            self.media_urls
                .get(url)
                .cloned()
                .unwrap_or_else(|| url.to_string())
        };

        card.html_back(Some(&url_rewriter))
            .map_err(|e| JsValue::from_str(&format!("Failed to render card: {}", e)))
    }

    /// Get the current card's deck name.
    #[wasm_bindgen]
    pub fn current_deck_name(&self) -> Option<String> {
        self.session_cards.last().map(|c| c.deck_name().clone())
    }

    /// Grade the current card.
    /// grade: "forgot", "hard", "good", or "easy"
    #[wasm_bindgen]
    pub fn grade_card(&mut self, grade_str: &str, now_str: &str) -> Result<(), JsValue> {
        let grade = match grade_str {
            "forgot" => Grade::Forgot,
            "hard" => Grade::Hard,
            "good" => Grade::Good,
            "easy" => Grade::Easy,
            _ => return Err(JsValue::from_str(&format!("Invalid grade: {}", grade_str))),
        };

        let now = Timestamp::try_from(now_str.to_string())
            .map_err(|e| JsValue::from_str(&format!("Invalid timestamp: {}", e)))?;

        let card = self
            .session_cards
            .pop()
            .ok_or_else(|| JsValue::from_str("No cards in session"))?;

        let hash = card.hash().to_hex();
        let current_perf = self
            .performance
            .get(&hash)
            .copied()
            .unwrap_or(Performance::New);
        let new_perf = update_performance(current_perf, grade, now);

        self.performance
            .insert(hash.clone(), Performance::Reviewed(new_perf));
        self.reviews_this_session += 1;

        // Re-add card to session if forgot or hard
        if matches!(grade, Grade::Forgot | Grade::Hard) {
            self.session_cards.insert(0, card);
        }

        // Save performance to storage
        self.save_performance(&hash, &Performance::Reviewed(new_perf))?;

        self.revealed = false;

        Ok(())
    }

    /// Get total number of cards in the collection.
    #[wasm_bindgen]
    pub fn collection_size(&self) -> usize {
        self.cards.len()
    }

    /// Get the number of new cards.
    #[wasm_bindgen]
    pub fn new_cards_count(&self) -> usize {
        self.cards
            .iter()
            .filter(|card| {
                let hash = card.hash().to_hex();
                matches!(self.performance.get(&hash), None | Some(Performance::New))
            })
            .count()
    }

    /// Get list of deck names.
    #[wasm_bindgen]
    pub fn deck_names(&self) -> String {
        let mut names: Vec<&str> = self.cards.iter().map(|c| c.deck_name().as_str()).collect();
        names.sort();
        names.dedup();
        serde_json::to_string(&names).unwrap_or_else(|_| "[]".to_string())
    }

    /// Export all performance data as JSON.
    #[wasm_bindgen]
    pub fn export_performance(&self) -> String {
        serde_json::to_string(&self.performance).unwrap_or_else(|_| "{}".to_string())
    }

    /// Import performance data from JSON.
    #[wasm_bindgen]
    pub fn import_performance(&mut self, json: &str) -> Result<(), JsValue> {
        let data: HashMap<String, Performance> = serde_json::from_str(json)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse performance data: {}", e)))?;
        self.performance = data;
        // Save all to storage
        for (hash, perf) in &self.performance {
            self.save_performance(hash, perf)?;
        }
        Ok(())
    }

    // Private helper methods

    fn load_performance(&mut self) -> Result<(), JsValue> {
        if let Some(data) = self.storage.get("hashcards_performance")? {
            self.performance = serde_json::from_str(&data)
                .map_err(|e| JsValue::from_str(&format!("Failed to load performance: {}", e)))?;
        }
        Ok(())
    }

    fn save_performance(&self, _hash: &str, _perf: &Performance) -> Result<(), JsValue> {
        let data = serde_json::to_string(&self.performance)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize performance: {}", e)))?;
        self.storage.set("hashcards_performance", &data)
    }
}

impl Default for HashcardsApp {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the current timestamp as an ISO string.
#[wasm_bindgen]
pub fn now_timestamp() -> String {
    let date = js_sys::Date::new_0();
    let year = date.get_full_year();
    let month = date.get_month() + 1;
    let day = date.get_date();
    let hours = date.get_hours();
    let minutes = date.get_minutes();
    let seconds = date.get_seconds();
    let millis = date.get_milliseconds();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}",
        year, month, day, hours, minutes, seconds, millis
    )
}

/// Get today's date as a string (YYYY-MM-DD).
#[wasm_bindgen]
pub fn today_date() -> String {
    let date = js_sys::Date::new_0();
    let year = date.get_full_year();
    let month = date.get_month() + 1;
    let day = date.get_date();
    format!("{:04}-{:02}-{:02}", year, month, day)
}
