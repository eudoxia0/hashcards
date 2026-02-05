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

//! hashcards-core: Core library for hashcards spaced repetition system.
//!
//! This library provides WASM-compatible types and algorithms for:
//! - Parsing Markdown flashcard files
//! - FSRS (Free Spaced Repetition Scheduler) algorithm
//! - Card types and performance tracking
//! - Markdown to HTML rendering

pub mod error;
pub mod fsrs;
pub mod markdown;
pub mod parser;
pub mod rng;
pub mod types;

// Re-exports for convenience
pub use error::{ErrorReport, Fallible, fail};
pub use fsrs::Grade;
pub use parser::{parse_deck_content, parse_decks};
pub use types::card::{Card, CardContent, CardType};
pub use types::card_hash::CardHash;
pub use types::date::Date;
pub use types::performance::{Performance, ReviewedPerformance, update_performance};
pub use types::timestamp::Timestamp;
