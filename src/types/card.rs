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

use maud::Markup;
use maud::PreEscaped;
use maud::html;

use crate::markdown::markdown_to_html;
use crate::markdown::markdown_to_html_inline;
use crate::types::card_type::CardType;
use crate::types::hash::Hash;
use crate::types::hash::Hasher;

const CLOZE_TAG: &str = "CLOZE_DELETION";

#[derive(Clone)]
pub struct Card {
    /// The name of the deck this card belongs to.
    deck_name: String,
    /// The absolute path to the file this card was parsed from.
    #[allow(dead_code)]
    file_path: PathBuf,
    /// The line number range that contains the card.
    #[allow(dead_code)]
    range: (usize, usize),
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
    pub fn new(
        deck_name: String,
        file_path: PathBuf,
        range: (usize, usize),
        content: CardContent,
    ) -> Self {
        let hash = content.hash();
        Self {
            deck_name,
            file_path,
            content,
            range,
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

    pub fn card_type(&self) -> CardType {
        match &self.content {
            CardContent::Basic { .. } => CardType::Basic,
            CardContent::Cloze { .. } => CardType::Cloze,
        }
    }

    pub fn html_front(&self) -> Markup {
        self.content.html_front()
    }

    pub fn html_back(&self) -> Markup {
        self.content.html_back()
    }
}

impl CardContent {
    pub fn new_basic(question: impl Into<String>, answer: impl Into<String>) -> Self {
        Self::Basic {
            question: question.into().trim().to_string(),
            answer: answer.into().trim().to_string(),
        }
    }

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

    pub fn html_front(&self) -> Markup {
        match self {
            CardContent::Basic { question, .. } => {
                html! {
                    (PreEscaped(markdown_to_html(question)))
                }
            }
            CardContent::Cloze { text, start, end } => {
                let mut prompt = text.clone();
                prompt.replace_range(*start..*end + 1, CLOZE_TAG);
                let prompt = markdown_to_html(&prompt);
                let prompt = prompt.replace(CLOZE_TAG, "<span class='cloze'>.............</span>");
                html! {
                    (PreEscaped(prompt))
                }
            }
        }
    }

    pub fn html_back(&self) -> Markup {
        match self {
            CardContent::Basic { answer, .. } => {
                html! {
                    (PreEscaped(markdown_to_html(answer)))
                }
            }
            CardContent::Cloze { text, start, end } => {
                let cloze_text = &text[*start..*end + 1];
                let mut answer = text.clone();
                answer.replace_range(*start..*end + 1, CLOZE_TAG);
                let answer = markdown_to_html(&answer);
                let cloze_text = markdown_to_html_inline(cloze_text);
                let answer = answer.replace(
                    CLOZE_TAG,
                    &format!("<span class='cloze-reveal'>{}</span>", cloze_text),
                );
                html! {
                    (PreEscaped(answer))
                }
            }
        }
    }
}
