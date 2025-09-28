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

use crate::hash::Hash;
use crate::hash::Hasher;

#[derive(Clone)]
pub struct Card {
    /// The name of the deck this card belongs to.
    deck_name: String,
    /// The absolute path to the file this card was parsed from.
    #[allow(dead_code)]
    file_path: PathBuf,
    /// The card's content.
    content: CardContent,
    /// The cached hash of the card's content.
    hash: Hash,
}

#[derive(Clone)]
pub enum CardContent {
    Basic {
        question: String,
        answer: String,
    },
    Cloze {
        /// The text of the card without brackets.
        text: String,
        /// The position of the first character of the deletion.
        start: usize,
        /// The position of the last character of the deletion.
        end: usize,
    },
}

impl Card {
    pub fn new(deck_name: String, file_path: PathBuf, content: CardContent) -> Self {
        let hash = content.hash();
        Self {
            deck_name,
            file_path,
            content,
            hash,
        }
    }

    pub fn deck_name(&self) -> &str {
        &self.deck_name
    }

    pub fn content(&self) -> &CardContent {
        &self.content
    }

    pub fn hash(&self) -> Hash {
        self.hash
    }
}

impl CardContent {
    pub fn hash(&self) -> Hash {
        let mut hasher = Hasher::new();
        match &self {
            CardContent::Basic { question, answer } => {
                hasher.update(b"Basic");
                hasher.update(question.as_bytes());
                hasher.update(answer.as_bytes());
            }
            CardContent::Cloze { text, start, end } => {
                hasher.update(b"Cloze");
                hasher.update(text.as_bytes());
                hasher.update(&start.to_le_bytes());
                hasher.update(&end.to_le_bytes());
            }
        }
        hasher.finalize()
    }
}
