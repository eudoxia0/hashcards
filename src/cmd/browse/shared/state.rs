// Copyright 2025–2026 Fernando Borretti
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

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::db::ReviewRecord;
use crate::types::card::Card;
use crate::types::card_hash::CardHash;
use crate::types::date::Date;
use crate::types::performance::Performance;

/// Server state.
#[derive(Clone)]
pub struct BrowseState {
    /// Server port.
    pub port: u16,
    /// Hostname to serve resources on.
    pub resource_hostname: String,
    /// The collection directory.
    pub directory: PathBuf,
    /// TeX macros.
    pub macros: Vec<(String, String)>,
    /// All the cards in the collection.
    pub cards: Arc<Vec<Card>>,
    /// Cloze cards grouped by their family hash, sorted by deletion position.
    pub families: Arc<HashMap<CardHash, Vec<Card>>>,
    /// Each card's performance data, as of server startup.
    pub performance: Arc<HashMap<CardHash, Performance>>,
    /// Each card's review history, in chronological order, as of server
    /// startup.
    pub reviews: Arc<HashMap<CardHash, Vec<ReviewRecord>>>,
    /// The date the server was started.
    pub today: Date,
}

impl BrowseState {
    /// The performance of the card with the given hash. Cards absent from the
    /// database return [`Performance::New`].
    pub fn performance_of(&self, hash: CardHash) -> Performance {
        self.performance
            .get(&hash)
            .copied()
            .unwrap_or(Performance::New)
    }

    /// Is the card due on or before today?
    pub fn is_due(&self, hash: CardHash) -> bool {
        match self.performance_of(hash) {
            Performance::New => true,
            Performance::Reviewed(rp) => rp.due_date <= self.today,
        }
    }

    /// The review history of the card with the given hash, in chronological
    /// order.
    pub fn reviews_of(&self, hash: CardHash) -> &[ReviewRecord] {
        self.reviews
            .get(&hash)
            .map(|reviews| reviews.as_slice())
            .unwrap_or(&[])
    }
}
