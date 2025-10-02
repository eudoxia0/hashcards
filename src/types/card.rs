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

use crate::error::Fallible;
use crate::markdown::markdown_to_html;
use crate::markdown::markdown_to_html_inline;
use crate::types::card_hash::CardHash;
use crate::types::card_hash::Hasher;
use crate::types::card_type::CardType;

const CLOZE_TAG_BYTES: &[u8] = b"CLOZE_DELETION";
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
    hash: CardHash,
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

    pub fn hash(&self) -> CardHash {
        self.hash
    }

    pub fn family_hash(&self) -> Option<CardHash> {
        self.content.family_hash()
    }

    pub fn card_type(&self) -> CardType {
        match &self.content {
            CardContent::Basic { .. } => CardType::Basic,
            CardContent::Cloze { .. } => CardType::Cloze,
        }
    }

    pub fn html_front(&self, port: u16) -> Fallible<Markup> {
        self.content.html_front(port)
    }

    pub fn html_back(&self, port: u16) -> Fallible<Markup> {
        self.content.html_back(port)
    }
}

impl CardContent {
    pub fn new_basic(question: impl Into<String>, answer: impl Into<String>) -> Self {
        Self::Basic {
            question: question.into().trim().to_string(),
            answer: answer.into().trim().to_string(),
        }
    }

    pub fn new_cloze(prompt: impl Into<String>, start: usize, end: usize) -> Self {
        Self::Cloze {
            text: prompt.into(),
            start,
            end,
        }
    }

    pub fn hash(&self) -> CardHash {
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

    /// All cloze cards derived from the same text have the same family hash.
    ///
    /// For basic cards, this is `None`.
    pub fn family_hash(&self) -> Option<CardHash> {
        match &self {
            CardContent::Basic { .. } => None,
            CardContent::Cloze { text, .. } => {
                let mut hasher = Hasher::new();
                hasher.update(b"Cloze");
                hasher.update(text.as_bytes());
                Some(hasher.finalize())
            }
        }
    }

    pub fn html_front(&self, port: u16) -> Fallible<Markup> {
        let html = match self {
            CardContent::Basic { question, .. } => {
                html! {
                    (PreEscaped(markdown_to_html(question, port)))
                }
            }
            CardContent::Cloze { text, start, end } => {
                let mut text_bytes: Vec<u8> = text.as_bytes().to_owned();
                text_bytes.splice(*start..*end + 1, CLOZE_TAG_BYTES.iter().copied());
                let text: String = String::from_utf8(text_bytes)?;
                let text: String = markdown_to_html(&text, port);
                let text: String =
                    text.replace(CLOZE_TAG, "<span class='cloze'>.............</span>");
                html! {
                    (PreEscaped(text))
                }
            }
        };
        Ok(html)
    }

    pub fn html_back(&self, port: u16) -> Fallible<Markup> {
        let html = match self {
            CardContent::Basic { answer, .. } => {
                html! {
                    (PreEscaped(markdown_to_html(answer, port)))
                }
            }
            CardContent::Cloze { text, start, end } => {
                let mut text_bytes: Vec<u8> = text.as_bytes().to_owned();
                let deleted_text: Vec<u8> = text_bytes[*start..*end + 1].to_owned();
                let deleted_text: String = String::from_utf8(deleted_text)?;
                let deleted_text: String = markdown_to_html_inline(&deleted_text, port);
                text_bytes.splice(*start..*end + 1, CLOZE_TAG_BYTES.iter().copied());
                let text: String = String::from_utf8(text_bytes)?;
                let text = markdown_to_html(&text, port);
                let text = text.replace(
                    CLOZE_TAG,
                    &format!("<span class='cloze-reveal'>{}</span>", deleted_text),
                );
                html! {
                    (PreEscaped(text))
                }
            }
        };
        Ok(html)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_card_hash() {
        let card1 = CardContent::new_basic("What is 2+2?", "4");
        let card2 = CardContent::new_basic("What is 2+2?", "4");
        let card3 = CardContent::new_basic("What is 3+3?", "6");
        assert_eq!(card1.hash(), card2.hash());
        assert_ne!(card1.hash(), card3.hash());
    }

    #[test]
    fn test_cloze_card_hash() {
        let a = CardContent::new_cloze("The capital of France is Paris", 0, 1);
        let b = CardContent::new_cloze("The capital of France is Paris", 0, 2);
        assert_eq!(a.family_hash(), b.family_hash());
    }

    #[test]
    fn test_family_hash() {
        let a = CardContent::new_cloze("The capital of France is Paris", 0, 1);
        let b = CardContent::new_cloze("The capital of France is Paris", 0, 2);
        assert_eq!(a.family_hash(), b.family_hash());
    }
}
