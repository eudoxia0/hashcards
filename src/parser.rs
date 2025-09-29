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
    ReadingQuestion { question: String },
    /// Reading an answer (A:)
    ReadingAnswer { question: String, answer: String },
    /// Reading a cloze card (C:)
    ReadingCloze { text: String },
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
        for line in lines.iter() {
            let line = Line::read(line);
            state = self.parse_line(state, line, &mut cards)?;
        }
        self.finalize(state, &mut cards)?;
        Ok(cards)
    }

    fn parse_line(&self, state: State, line: Line, cards: &mut Vec<Card>) -> Fallible<State> {
        match state {
            State::Initial => match line {
                Line::StartQuestion(text) => Ok(State::ReadingQuestion { question: text }),
                Line::StartAnswer(_) => fail("Answer without question."),
                Line::StartCloze(text) => Ok(State::ReadingCloze { text }),
                Line::Text(_) => Ok(State::Initial),
            },
            State::ReadingQuestion { question } => match line {
                Line::StartQuestion(_) => fail("New question without answer."),
                Line::StartAnswer(text) => Ok(State::ReadingAnswer {
                    question,
                    answer: text,
                }),
                Line::StartCloze(_) => {
                    fail("Started a cloze card inside a question card question.")
                }
                Line::Text(text) => Ok(State::ReadingQuestion {
                    question: format!("{question}\n{text}"),
                }),
            },
            State::ReadingAnswer { question, answer } => {
                match line {
                    Line::StartQuestion(text) => {
                        // Finalize the previous card.
                        let card = Card::new(
                            self.deck_name.clone(),
                            self.file_path.clone(),
                            CardContent::Basic { question, answer },
                        );
                        cards.push(card);
                        // Start a new question.
                        Ok(State::ReadingQuestion { question: text })
                    }
                    Line::StartAnswer(_) => fail("New answer without question."),
                    Line::StartCloze(text) => {
                        // Finalize the previous card.
                        let card = Card::new(
                            self.deck_name.clone(),
                            self.file_path.clone(),
                            CardContent::Basic { question, answer },
                        );
                        cards.push(card);
                        // Start reading a new cloze card.
                        Ok(State::ReadingCloze { text })
                    }
                    Line::Text(text) => Ok(State::ReadingAnswer {
                        question,
                        answer: format!("{answer}\n{text}"),
                    }),
                }
            }
            State::ReadingCloze { text } => {
                match line {
                    Line::StartQuestion(new_text) => {
                        // Finalize the previous cloze card.
                        cards.extend(self.parse_cloze_cards(text)?);
                        // Start a new question card
                        Ok(State::ReadingQuestion { question: new_text })
                    }
                    Line::StartAnswer(_) => fail("Found answer tag while reading a cloze card."),
                    Line::StartCloze(new_text) => {
                        // Finalize the previous card.
                        cards.extend(self.parse_cloze_cards(text)?);
                        // Start reading a new cloze card.
                        Ok(State::ReadingCloze { text: new_text })
                    }
                    Line::Text(new_text) => Ok(State::ReadingCloze {
                        text: format!("{text}\n{new_text}"),
                    }),
                }
            }
        }
    }

    fn finalize(&self, state: State, cards: &mut Vec<Card>) -> Fallible<()> {
        match state {
            State::Initial => Ok(()),
            State::ReadingQuestion { .. } => fail("Unfinished question without answer at EOF."),
            State::ReadingAnswer { question, answer } => {
                // Finalize the last card.
                let card = Card::new(
                    self.deck_name.clone(),
                    self.file_path.clone(),
                    CardContent::Basic { question, answer },
                );
                cards.push(card);
                Ok(())
            }
            State::ReadingCloze { text } => {
                // Finalize the last cloze card.
                cards.extend(self.parse_cloze_cards(text)?);
                Ok(())
            }
        }
    }

    fn parse_cloze_cards(&self, text: String) -> Fallible<Vec<Card>> {
        let mut cards = Vec::new();

        // The full text of the card, without square brackets.
        let clean_text = text.replace(['[', ']'], "");

        let mut start = None;
        let mut index = 0;
        let mut image_mode = false;
        for c in text.chars() {
            if c == '[' {
                if !image_mode {
                    start = Some(index);
                }
            } else if c == ']' {
                if image_mode {
                    // We are in image mode, so this closing bracket is part of a markdown image.
                    image_mode = false;
                } else if let Some(s) = start {
                    let end = index;
                    let content = CardContent::Cloze {
                        text: clean_text.clone(),
                        start: s,
                        end: end - 1,
                    };
                    let card = Card::new(self.deck_name.clone(), self.file_path.clone(), content);
                    cards.push(card);
                    start = None;
                }
            } else if c == '!' {
                image_mode = true;
            } else {
                index += 1;
            }
        }

        Ok(cards)
    }
}
