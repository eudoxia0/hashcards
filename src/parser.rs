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

use blake3::Hash;
use blake3::Hasher;

#[derive(Clone)]
pub struct Card {
    pub content: CardContent,
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
    pub fn hash(&self) -> Hash {
        self.content.hash()
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

pub fn parse_cards(content: &str) -> Vec<Card> {
    let mut flashcards = Vec::new();

    let cards: Vec<&str> = content
        .split("\n\n")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    for card_text in cards {
        if let Some(separator_pos) = card_text.find(" / ") {
            let question = card_text[..separator_pos].trim().to_string();
            let answer = card_text[separator_pos + 3..].trim().to_string();
            if !question.is_empty() && !answer.is_empty() {
                let card = Card {
                    content: CardContent::Basic { question, answer },
                };
                flashcards.push(card);
            }
        } else if card_text.contains('[') && card_text.contains(']') {
            let clozes = parse_cloze_card(card_text);
            flashcards.extend(clozes);
        }
    }

    flashcards
}

// Parses a cloze deletion card and returns a vector of cards, one for each deletion.
fn parse_cloze_card(text: &str) -> Vec<Card> {
    let mut cards = Vec::new();

    // The full text of the card, without square brackets.
    let clean_text = text.replace(['[', ']'], "");

    let mut start = None;
    let mut index = 0;
    for c in text.chars() {
        if c == '[' {
            start = Some(index);
        } else if c == ']' {
            if let Some(s) = start {
                let end = index;
                let card = Card {
                    content: CardContent::Cloze {
                        text: clean_text.clone(),
                        start: s,
                        end: end - 1,
                    },
                };
                cards.push(card);
                start = None;
            }
        } else {
            index += 1;
        }
    }

    cards
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic() {
        let content = "What is the capital of France? / Paris";
        let cards = parse_cards(content);

        assert_eq!(cards.len(), 1);
        match &cards[0].content {
            CardContent::Basic { question, answer } => {
                assert_eq!(question, "What is the capital of France?");
                assert_eq!(answer, "Paris");
            }
            _ => panic!("Expected Basic card"),
        }
    }

    #[test]
    fn test_parse_cloze() {
        let content = "[Berlin] is the capital of [Germany].";
        let cards = parse_cards(content);
        assert_eq!(cards.len(), 2);
        match &cards[0].content {
            CardContent::Cloze { text, start, end } => {
                assert_eq!(text, "Berlin is the capital of Germany.");
                assert_eq!(*start, 0);
                assert_eq!(*end, 5);
            }
            _ => panic!("Expected Cloze card"),
        }
        match &cards[1].content {
            CardContent::Cloze { text, start, end } => {
                assert_eq!(text, "Berlin is the capital of Germany.");
                assert_eq!(*start, 25);
                assert_eq!(*end, 31);
            }
            _ => panic!("Expected Cloze card"),
        }
    }

    #[test]
    fn test_parse_multiple_cards() {
        let content =
            "What is the capital of France? / Paris\n\n[Berlin] is the capital of [Germany].";
        let cards = parse_cards(content);

        assert_eq!(cards.len(), 3);
        assert!(matches!(cards[0].content, CardContent::Basic { .. }));
        assert!(matches!(cards[1].content, CardContent::Cloze { .. }));
        assert!(matches!(cards[1].content, CardContent::Cloze { .. }));
    }

    #[test]
    fn test_parse_with_extra_whitespace() {
        let content = "  What is 2+2? / 4  \n\n\n[Python] is a programming language.  ";
        let cards = parse_cards(content);

        assert_eq!(cards.len(), 2);
        match &cards[0].content {
            CardContent::Basic { question, answer } => {
                assert_eq!(question, "What is 2+2?");
                assert_eq!(answer, "4");
            }
            _ => panic!("Expected Basic card"),
        }
    }

    #[test]
    fn test_empty_input() {
        let content = "";
        let cards = parse_cards(content);
        assert_eq!(cards.len(), 0);
    }

    #[test]
    fn test_empty_whitespace_input() {
        let content = "\n   \n  \n";
        let cards = parse_cards(content);
        assert_eq!(cards.len(), 0);
    }

    #[test]
    fn test_empty_basic() {
        let content = " / ";
        let cards = parse_cards(content);
        assert_eq!(cards.len(), 0);
    }

    #[test]
    fn test_invalid_cards_ignored() {
        let content = "This is not a valid card\n\nWhat is valid? / Yes\n\nAlso not valid";
        let cards = parse_cards(content);
        assert_eq!(cards.len(), 1);
        match &cards[0].content {
            CardContent::Basic { question, answer } => {
                assert_eq!(question, "What is valid?");
                assert_eq!(answer, "Yes");
            }
            _ => panic!("Expected Basic card"),
        }
    }

    #[test]
    fn test_multiline_question_answer() {
        let content = "What is\nthe capital of Russia? / Moscow";
        let cards = parse_cards(content);

        assert_eq!(cards.len(), 1);
        match &cards[0].content {
            CardContent::Basic { question, answer } => {
                assert_eq!(question, "What is\nthe capital of Russia?");
                assert_eq!(answer, "Moscow");
            }
            _ => panic!("Expected Basic card"),
        }
    }

    #[test]
    fn test_basic_card_hash() {
        let card1 = CardContent::Basic {
            question: "What is the capital of France?".to_string(),
            answer: "Paris".to_string(),
        };
        let card2 = CardContent::Basic {
            question: "What is the capital of France?".to_string(),
            answer: "Pariz".to_string(),
        };
        assert_ne!(card1.hash(), card2.hash());
    }

    #[test]
    fn test_cloze_card_hash() {
        let card1 = CardContent::Cloze {
            text: "Berlin is the capital of Germany.".to_string(),
            start: 0,
            end: 6,
        };
        let card2 = CardContent::Cloze {
            text: "Berlin is the capital of Germany.".to_string(),
            start: 0,
            end: 7,
        };
        assert_ne!(card1.hash(), card2.hash());
    }
}
