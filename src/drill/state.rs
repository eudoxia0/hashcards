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

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use crate::new_db::Database;
use crate::new_db::Date;
use crate::new_db::Review;
use crate::new_db::Timestamp;
use crate::parser::Card;

#[derive(Clone)]
pub struct ServerState {
    pub today: Date,
    pub directory: PathBuf,
    pub macros: Vec<(String, String)>,
    pub total_cards: usize,
    pub session_started_at: Timestamp,
    pub mutable: Arc<Mutex<MutableState>>,
}

pub struct MutableState {
    pub reveal: bool,
    pub db: Database,
    pub cards: Vec<Card>,
    pub reviews: Vec<Review>,
}
