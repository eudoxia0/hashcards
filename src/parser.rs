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

pub enum Card {
    Basic {
        question: String,
        answer: String,
    },
    Cloze {
        text: String,
        deletions: Vec<ClozeRange>,
    },
}

pub struct ClozeRange {
    pub start: usize,
    pub end: usize,
}

impl Card {
    #[allow(dead_code)]
    pub fn hash(&self) -> Hash {
        let mut hasher = Hasher::new();
        match self {
            Card::Basic { question, answer } => {
                hasher.update(b"Basic");
                hasher.update(question.as_bytes());
                hasher.update(answer.as_bytes());
            }
            Card::Cloze { text, deletions } => {
                hasher.update(b"Cloze");
                hasher.update(text.as_bytes());
                for deletion in deletions {
                    hasher.update(deletion.start.to_le_bytes().as_ref());
                    hasher.update(deletion.end.to_le_bytes().as_ref());
                }
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
        if card_text.trim().is_empty() {
            continue;
        }

        if let Some(separator_pos) = card_text.find(" / ") {
            let question = card_text[..separator_pos].trim().to_string();
            let answer = card_text[separator_pos + 3..].trim().to_string();
            if !question.is_empty() && !answer.is_empty() {
                flashcards.push(Card::Basic { question, answer });
            }
        } else if card_text.contains('[')
            && card_text.contains(']')
            && let Some(cloze) = parse_cloze_card(card_text)
        {
            flashcards.push(cloze);
        }
    }

    flashcards
}

fn parse_cloze_card(text: &str) -> Option<Card> {
    let mut deletions = Vec::new();
    let mut clean_text = String::new();
    let mut chars = text.chars().peekable();
    let mut current_pos = 0;

    while let Some(ch) = chars.next() {
        if ch == '[' {
            // Start of a cloze deletion
            let start_pos = current_pos;
            let mut deletion_content = String::new();

            // Collect characters until we find the closing bracket
            let mut found_closing = false;
            for inner_ch in chars.by_ref() {
                if inner_ch == ']' {
                    found_closing = true;
                    break;
                }
                deletion_content.push(inner_ch);
            }

            if found_closing && !deletion_content.is_empty() {
                deletions.push(ClozeRange {
                    start: start_pos,
                    end: start_pos + deletion_content.len(),
                });
                clean_text.push_str(&deletion_content);
                current_pos += deletion_content.len();
            } else {
                clean_text.push(ch);
                current_pos += 1;
                clean_text.push_str(&deletion_content);
                current_pos += deletion_content.len();
            }
        } else {
            clean_text.push(ch);
            current_pos += 1;
        }
    }

    if !deletions.is_empty() {
        Some(Card::Cloze {
            text: clean_text,
            deletions,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic() {
        let content = "What is the capital of France? / Paris";
        let cards = parse_flashcards(content);

        assert_eq!(cards.len(), 1);
        match &cards[0] {
            Card::Basic { question, answer } => {
                assert_eq!(question, "What is the capital of France?");
                assert_eq!(answer, "Paris");
            }
            _ => panic!("Expected Basic card"),
        }
    }

    #[test]
    fn test_parse_cloze() {
        let content = "[Berlin] is the capital of [Germany].";
        let cards = parse_flashcards(content);

        assert_eq!(cards.len(), 1);
        match &cards[0] {
            Card::Cloze { text, deletions } => {
                assert_eq!(text, "Berlin is the capital of Germany.");
                assert_eq!(deletions.len(), 2);
                let first = &deletions[0];
                assert_eq!(first.start, 0);
                assert_eq!(first.end, 6);
                assert_eq!(&text[first.start..first.end], "Berlin");
                let second = &deletions[1];
                assert_eq!(second.start, 25);
                assert_eq!(second.end, 32);
                assert_eq!(&text[second.start..second.end], "Germany");
            }
            _ => panic!("Expected Cloze card"),
        }
    }

    #[test]
    fn test_parse_multiple_cards() {
        let content =
            "What is the capital of France? / Paris\n\n[Berlin] is the capital of [Germany].";
        let cards = parse_flashcards(content);

        assert_eq!(cards.len(), 2);
        assert!(matches!(cards[0], Card::Basic { .. }));
        assert!(matches!(cards[1], Card::Cloze { .. }));
    }

    #[test]
    fn test_parse_with_extra_whitespace() {
        let content = "  What is 2+2? / 4  \n\n\n[Python] is a programming language.  ";
        let cards = parse_flashcards(content);

        assert_eq!(cards.len(), 2);
        match &cards[0] {
            Card::Basic { question, answer } => {
                assert_eq!(question, "What is 2+2?");
                assert_eq!(answer, "4");
            }
            _ => panic!("Expected Basic card"),
        }
    }

    #[test]
    fn test_empty_input() {
        let content = "";
        let cards = parse_flashcards(content);
        assert_eq!(cards.len(), 0);
    }

    #[test]
    fn test_invalid_cards_ignored() {
        let content = "This is not a valid card\n\nWhat is valid? / Yes\n\nAlso not valid";
        let cards = parse_flashcards(content);
        assert_eq!(cards.len(), 1);
        match &cards[0] {
            Card::Basic { question, answer } => {
                assert_eq!(question, "What is valid?");
                assert_eq!(answer, "Yes");
            }
            _ => panic!("Expected Basic card"),
        }
    }

    #[test]
    fn test_cloze_with_multiple_words() {
        let content = "[The quick brown fox] jumps over [the lazy dog].";
        let cards = parse_flashcards(content);

        assert_eq!(cards.len(), 1);
        match &cards[0] {
            Card::Cloze { text, deletions } => {
                assert_eq!(text, "The quick brown fox jumps over the lazy dog.");
                assert_eq!(deletions.len(), 2);
                let first = &deletions[0];
                assert_eq!(&text[first.start..first.end], "The quick brown fox");
                let second = &deletions[1];
                assert_eq!(&text[second.start..second.end], "the lazy dog");
            }
            _ => panic!("Expected Cloze card"),
        }
    }

    #[test]
    fn test_multiline_question_answer() {
        let content = "What is\nthe capital of Russia? / Moscow";
        let cards = parse_flashcards(content);

        assert_eq!(cards.len(), 1);
        match &cards[0] {
            Card::Basic { question, answer } => {
                assert_eq!(question, "What is\nthe capital of Russia?");
                assert_eq!(answer, "Moscow");
            }
            _ => panic!("Expected Basic card"),
        }
    }
}
