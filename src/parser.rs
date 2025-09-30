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

use std::collections::HashSet;
use std::path::PathBuf;

use walkdir::WalkDir;

use crate::error::Fallible;
use crate::error::fail;
use crate::types::card::Card;
use crate::types::card::CardContent;

/// Parses all Markdown files in the given directory.
pub fn parse_deck(directory: &PathBuf) -> Fallible<Vec<Card>> {
    let mut all_cards = Vec::new();
    for entry in WalkDir::new(directory) {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "md") {
            let text = std::fs::read_to_string(path)?;
            let deck_name: String = path
                .file_stem()
                .and_then(|os_str| os_str.to_str())
                .unwrap_or("None")
                .to_string();
            let parser = Parser::new(deck_name, path.to_path_buf());
            let cards = parser.parse(&text)?;
            all_cards.extend(cards);
        }
    }

    // Cards are sorted by their hash. This means cards are shown in a
    // deterministic sequence, but it appears random to the user. This gives us
    // both the debugging benefits of determinism, and the learning benefits of
    // randomization (mixing cards from different decks) without needing an
    // RNG.
    all_cards.sort_by_key(|c| c.hash());

    // Remove duplicates.
    all_cards.dedup_by_key(|c| c.hash());

    Ok(all_cards)
}

pub struct Parser {
    deck_name: String,
    file_path: PathBuf,
}

enum State {
    /// Initial state.
    Initial,
    /// Reading a question (Q:)
    ReadingQuestion { question: String, start_line: usize },
    /// Reading an answer (A:)
    ReadingAnswer {
        question: String,
        answer: String,
        start_line: usize,
    },
    /// Reading a cloze card (C:)
    ReadingCloze { text: String, start_line: usize },
}

enum Line {
    /// A line like `Q: <text>`.
    StartQuestion(String),
    /// A line like `A: <text>`.
    StartAnswer(String),
    /// A line like `C: <text>`.
    StartCloze(String),
    /// Any other line.
    Text(String),
}

impl Line {
    fn read(line: &str) -> Self {
        if is_question(line) {
            Line::StartQuestion(trim(line))
        } else if is_answer(line) {
            Line::StartAnswer(trim(line))
        } else if is_cloze(line) {
            Line::StartCloze(trim(line))
        } else {
            Line::Text(line.to_string())
        }
    }
}

fn is_question(line: &str) -> bool {
    line.starts_with("Q:")
}

fn is_answer(line: &str) -> bool {
    line.starts_with("A:")
}

fn is_cloze(line: &str) -> bool {
    line.starts_with("C:")
}

fn trim(line: &str) -> String {
    line[2..].trim().to_string()
}

impl Parser {
    pub fn new(deck_name: String, file_path: PathBuf) -> Self {
        Parser {
            deck_name,
            file_path,
        }
    }

    /// Parse all the cards in the given text.
    pub fn parse(&self, text: &str) -> Fallible<Vec<Card>> {
        let mut cards = Vec::new();
        let mut state = State::Initial;
        let lines: Vec<&str> = text.lines().collect();
        let last_line = if lines.is_empty() { 0 } else { lines.len() - 1 };
        for (line_num, line) in lines.iter().enumerate() {
            let line = Line::read(line);
            state = self.parse_line(state, line, line_num, &mut cards)?;
        }
        self.finalize(state, last_line, &mut cards)?;

        let mut seen = HashSet::new();
        let mut unique_cards = Vec::new();
        for card in cards {
            if seen.insert(card.hash()) {
                unique_cards.push(card);
            }
        }
        Ok(unique_cards)
    }

    fn parse_line(
        &self,
        state: State,
        line: Line,
        line_num: usize,
        cards: &mut Vec<Card>,
    ) -> Fallible<State> {
        match state {
            State::Initial => match line {
                Line::StartQuestion(text) => Ok(State::ReadingQuestion {
                    question: text,
                    start_line: line_num,
                }),
                Line::StartAnswer(_) => fail("Answer without question."),
                Line::StartCloze(text) => Ok(State::ReadingCloze {
                    text,
                    start_line: line_num,
                }),
                Line::Text(_) => Ok(State::Initial),
            },
            State::ReadingQuestion {
                question,
                start_line,
            } => match line {
                Line::StartQuestion(_) => fail("New question without answer."),
                Line::StartAnswer(text) => Ok(State::ReadingAnswer {
                    question,
                    answer: text,
                    start_line,
                }),
                Line::StartCloze(_) => {
                    fail("Started a cloze card inside a question card question.")
                }
                Line::Text(text) => Ok(State::ReadingQuestion {
                    question: format!("{question}\n{text}"),
                    start_line,
                }),
            },
            State::ReadingAnswer {
                question,
                answer,
                start_line,
            } => {
                match line {
                    Line::StartQuestion(text) => {
                        // Finalize the previous card.
                        let card = Card::new(
                            self.deck_name.clone(),
                            self.file_path.clone(),
                            (start_line, line_num),
                            CardContent::new_basic(question, answer),
                        );
                        cards.push(card);
                        // Start a new question.
                        Ok(State::ReadingQuestion {
                            question: text,
                            start_line: line_num,
                        })
                    }
                    Line::StartAnswer(_) => fail("New answer without question."),
                    Line::StartCloze(text) => {
                        // Finalize the previous card.
                        let card = Card::new(
                            self.deck_name.clone(),
                            self.file_path.clone(),
                            (start_line, line_num),
                            CardContent::new_basic(question, answer),
                        );
                        cards.push(card);
                        // Start reading a new cloze card.
                        Ok(State::ReadingCloze {
                            text,
                            start_line: line_num,
                        })
                    }
                    Line::Text(text) => Ok(State::ReadingAnswer {
                        question,
                        answer: format!("{answer}\n{text}"),
                        start_line,
                    }),
                }
            }
            State::ReadingCloze { text, start_line } => {
                match line {
                    Line::StartQuestion(new_text) => {
                        // Finalize the previous cloze card.
                        cards.extend(self.parse_cloze_cards(text, start_line, line_num)?);
                        // Start a new question card
                        Ok(State::ReadingQuestion {
                            question: new_text,
                            start_line: line_num,
                        })
                    }
                    Line::StartAnswer(_) => fail("Found answer tag while reading a cloze card."),
                    Line::StartCloze(new_text) => {
                        // Finalize the previous card.
                        cards.extend(self.parse_cloze_cards(text, start_line, line_num)?);
                        // Start reading a new cloze card.
                        Ok(State::ReadingCloze {
                            text: new_text,
                            start_line: line_num,
                        })
                    }
                    Line::Text(new_text) => Ok(State::ReadingCloze {
                        text: format!("{text}\n{new_text}"),
                        start_line,
                    }),
                }
            }
        }
    }

    fn finalize(&self, state: State, last_line: usize, cards: &mut Vec<Card>) -> Fallible<()> {
        match state {
            State::Initial => Ok(()),
            State::ReadingQuestion { .. } => fail("Unfinished question without answer at EOF."),
            State::ReadingAnswer {
                question,
                answer,
                start_line,
            } => {
                // Finalize the last card.
                let card = Card::new(
                    self.deck_name.clone(),
                    self.file_path.clone(),
                    (start_line, last_line),
                    CardContent::new_basic(question, answer),
                );
                cards.push(card);
                Ok(())
            }
            State::ReadingCloze { text, start_line } => {
                // Finalize the last cloze card.
                cards.extend(self.parse_cloze_cards(text, start_line, last_line)?);
                Ok(())
            }
        }
    }

    fn parse_cloze_cards(
        &self,
        text: String,
        start_line: usize,
        end_line: usize,
    ) -> Fallible<Vec<Card>> {
        let text = text.trim();
        let mut cards = Vec::new();

        // The full text of the card, without cloze deletion brackets.
        let clean_text: String = {
            let mut clean_text: Vec<u8> = Vec::new();
            let mut image_mode = false;
            // We use `bytes` rather than `chars` because the cloze start/end
            // positions are byte positions, not character positions. This
            // keeps things tractable: bytes are well-understood, "characters"
            // are a vague abstract concept.
            for c in text.bytes() {
                if c == b'[' {
                    if image_mode {
                        clean_text.push(c);
                    }
                } else if c == b']' {
                    if image_mode {
                        // We are in image mode, so this closing bracket is
                        // part of a Markdown image.
                        image_mode = false;
                        clean_text.push(c);
                    }
                } else if c == b'!' {
                    image_mode = true;
                    clean_text.push(c);
                } else {
                    clean_text.push(c);
                }
            }
            String::from_utf8(clean_text)?
        };

        let mut start = None;
        let mut index = 0;
        let mut image_mode = false;
        for c in text.bytes() {
            if c == b'[' {
                if image_mode {
                    index += 1;
                } else {
                    start = Some(index);
                }
            } else if c == b']' {
                if image_mode {
                    // We are in image mode, so this closing bracket is part of a markdown image.
                    image_mode = false;
                    index += 1;
                } else if let Some(s) = start {
                    let end = index;
                    let content = CardContent::Cloze {
                        text: clean_text.clone(),
                        start: s,
                        end: end - 1,
                    };
                    let card = Card::new(
                        self.deck_name.clone(),
                        self.file_path.clone(),
                        (start_line, end_line),
                        content,
                    );
                    cards.push(card);
                    start = None;
                }
            } else if c == b'!' {
                image_mode = true;
                index += 1;
            } else {
                index += 1;
            }
        }

        if cards.is_empty() {
            fail("Cloze card must have at least one deletion.")
        } else {
            Ok(cards)
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::env::temp_dir;
    use std::fs::create_dir_all;

    use super::*;

    #[test]
    fn test_empty_string() -> Fallible<()> {
        let input = "";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;
        assert_eq!(cards.len(), 0);
        Ok(())
    }

    #[test]
    fn test_whitespace_string() -> Fallible<()> {
        let input = "\n\n\n";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;
        assert_eq!(cards.len(), 0);
        Ok(())
    }

    #[test]
    fn test_basic_card() -> Fallible<()> {
        let input = "Q: What is Rust?\nA: A systems programming language.";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;
        assert_eq!(cards.len(), 1);
        match &cards[0].content() {
            CardContent::Basic { question, answer } => {
                assert_eq!(question, "What is Rust?");
                assert_eq!(answer, "A systems programming language.");
            }
            _ => panic!("Expected basic card"),
        }
        Ok(())
    }

    #[test]
    fn test_multiline_qa() -> Fallible<()> {
        let input = "Q: foo\nbaz\nbaz\nA: FOO\nBAR\nBAZ";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;

        assert_eq!(cards.len(), 1);
        match &cards[0].content() {
            CardContent::Basic { question, answer } => {
                assert_eq!(question, "foo\nbaz\nbaz");
                assert_eq!(answer, "FOO\nBAR\nBAZ");
            }
            _ => panic!("Expected basic card"),
        }
        Ok(())
    }

    #[test]
    fn test_two_questions() -> Fallible<()> {
        let input = "Q: foo\nA: bar\n\nQ: baz\nA: quux\n\n";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;
        assert_eq!(cards.len(), 2);
        match &cards[0].content() {
            CardContent::Basic { question, answer } => {
                assert_eq!(question, "foo");
                assert_eq!(answer, "bar");
            }
            _ => panic!("Expected basic card"),
        }
        match &cards[1].content() {
            CardContent::Basic { question, answer } => {
                assert_eq!(question, "baz");
                assert_eq!(answer, "quux");
            }
            _ => panic!("Expected basic card"),
        }
        Ok(())
    }

    #[test]
    fn test_cloze_followed_by_question() -> Fallible<()> {
        let input = "C: [foo]\nQ: Question\nA: Answer";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;
        assert_eq!(cards.len(), 2);
        assert_cloze(&cards[0..1], "foo", &[(0, 2)]);
        match &cards[1].content() {
            CardContent::Basic { question, answer } => {
                assert_eq!(question, "Question");
                assert_eq!(answer, "Answer");
            }
            _ => panic!("Expected basic card"),
        }
        Ok(())
    }

    #[test]
    fn test_cloze_single() -> Fallible<()> {
        let input = "C: Foo [bar] baz.";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;

        assert_cloze(&cards, "Foo bar baz.", &[(4, 6)]);
        Ok(())
    }

    #[test]
    fn test_cloze_multiple() -> Fallible<()> {
        let input = "C: Foo [bar] baz [quux].";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;

        assert_cloze(&cards, "Foo bar baz quux.", &[(4, 6), (12, 15)]);
        Ok(())
    }

    #[test]
    fn test_cloze_with_image() -> Fallible<()> {
        let input = "C: Foo [bar] ![](image.jpg) [quux].";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;

        assert_cloze(&cards, "Foo bar ![](image.jpg) quux.", &[(4, 6), (23, 26)]);
        Ok(())
    }

    #[test]
    fn test_multi_line_cloze() -> Fallible<()> {
        let input = "C: [foo]\n[bar]\nbaz.";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;

        assert_cloze(&cards, "foo\nbar\nbaz.", &[(0, 2), (4, 6)]);
        Ok(())
    }

    #[test]
    fn test_two_clozes() -> Fallible<()> {
        let input = "C: [foo]\nC: [bar]";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;
        assert_eq!(cards.len(), 2);
        assert_cloze(&cards[0..1], "foo", &[(0, 2)]);
        assert_cloze(&cards[1..2], "bar", &[(0, 2)]);
        Ok(())
    }

    #[test]
    fn test_question_without_answer() -> Fallible<()> {
        let input = "Q: Question without answer";
        let parser = make_test_parser();
        let result = parser.parse(input);
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_answer_without_question() -> Fallible<()> {
        let input = "A: Answer without question";
        let parser = make_test_parser();
        let result = parser.parse(input);
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_question_followed_by_cloze() -> Fallible<()> {
        let input = "Q: Question\nC: Cloze";
        let parser = make_test_parser();
        let result = parser.parse(input);
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_question_followed_by_question() -> Fallible<()> {
        let input = "Q: Question\nQ: Another";
        let parser = make_test_parser();
        let result = parser.parse(input);
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_multiple_answers() -> Fallible<()> {
        let input = "Q: Question\nA: Answer\nA: Another answer";
        let parser = make_test_parser();
        let result = parser.parse(input);
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_cloze_followed_by_answer() -> Fallible<()> {
        let input = "C: Cloze\nA: Answer";
        let parser = make_test_parser();
        let result = parser.parse(input);
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_cloze_without_deletions() -> Fallible<()> {
        let input = "C: Cloze";
        let parser = make_test_parser();
        let result = parser.parse(input);
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_cloze_with_initial_blank_line() -> Fallible<()> {
        let input = "C:\nBuild something people want in Lisp.\n\n— [Paul Graham], [_Hackers and Painters_]\n\n";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;

        assert_cloze(
            &cards,
            "Build something people want in Lisp.\n\n— Paul Graham, _Hackers and Painters_",
            &[(42, 52), (55, 76)],
        );
        Ok(())
    }

    #[test]
    fn test_parse_deck() -> Fallible<()> {
        let directory = PathBuf::from("./example");
        let deck = parse_deck(&directory);
        assert!(deck.is_ok());
        let cards = deck?;
        assert_eq!(cards.len(), 7);
        Ok(())
    }

    #[test]
    fn test_identical_basic_cards() -> Fallible<()> {
        let input = "Q: foo\nA: bar\n\nQ: foo\nA: bar\n\n";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;
        assert_eq!(cards.len(), 1);
        Ok(())
    }

    #[test]
    fn test_identical_cloze_cards() -> Fallible<()> {
        let input = "C: foo [bar]\n\nC: foo [bar]";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;
        assert_eq!(cards.len(), 1);
        Ok(())
    }

    #[test]
    fn test_identical_cards_across_files() -> Fallible<()> {
        let directory = temp_dir();
        let directory = directory.join("identical_cards_test");
        create_dir_all(&directory)?;
        let file1 = directory.join("file1.md");
        let file2 = directory.join("file2.md");
        std::fs::write(&file1, "Q: foo\nA: bar")?;
        std::fs::write(&file2, "Q: foo\nA: bar")?;
        let deck = parse_deck(&directory)?;
        assert_eq!(deck.len(), 1);
        Ok(())
    }

    fn make_test_parser() -> Parser {
        Parser::new("test_deck".to_string(), PathBuf::from("test.md"))
    }

    fn assert_cloze(cards: &[Card], clean_text: &str, deletions: &[(usize, usize)]) {
        assert_eq!(cards.len(), deletions.len());
        for (i, (start, end)) in deletions.iter().enumerate() {
            match &cards[i].content() {
                CardContent::Cloze {
                    text,
                    start: s,
                    end: e,
                } => {
                    assert_eq!(text, clean_text);
                    assert_eq!(*s, *start);
                    assert_eq!(*e, *end);
                }
                _ => panic!("Expected cloze card"),
            }
        }
    }
}
