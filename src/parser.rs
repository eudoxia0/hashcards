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
use std::error::Error;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fs::read_to_string;
use std::path::PathBuf;

use serde::Deserialize;
use walkdir::WalkDir;

use crate::error::ErrorReport;
use crate::error::Fallible;
use crate::types::aliases::DeckName;
use crate::types::card::Card;
use crate::types::card::CardContent;

/// Metadata that can be specified at the top of a deck file.
#[derive(Debug, Deserialize)]
struct DeckMetadata {
    name: Option<String>,
}

/// Extract TOML frontmatter from markdown text.
/// Returns (frontmatter_metadata, content_without_frontmatter)
///
/// This function returns a slice of the original text to avoid
/// collecting lines, joining them, and then re-splitting in parse().
fn extract_frontmatter(text: &str) -> Fallible<(DeckMetadata, &str)> {
    let mut lines = text.lines().enumerate().peekable();

    // Check if the file starts with frontmatter delimiter
    match lines.peek() {
        Some((_, line)) if line.trim() == "---" => {}
        _ => return Ok((DeckMetadata { name: None }, text)),
    };
    lines.next(); // consume the opening delimiter

    // Collect frontmatter lines and find closing delimiter
    let mut frontmatter_lines = Vec::new();
    let mut closing_line_idx = None;

    for (idx, line) in lines {
        if line.trim() == "---" {
            closing_line_idx = Some(idx);
            break;
        }
        frontmatter_lines.push(line);
    }

    let closing_line_idx = closing_line_idx
        .ok_or_else(|| ErrorReport::new("Frontmatter opening '---' found but no closing '---'"))?;

    // Parse TOML from frontmatter
    let frontmatter_str = frontmatter_lines.join("\n");
    let metadata: DeckMetadata = toml::from_str(&frontmatter_str)
        .map_err(|e| ErrorReport::new(format!("Failed to parse TOML frontmatter: {}", e)))?;

    // Find byte offset where content starts (line after closing delimiter)
    // We do this by finding the position of the closing delimiter line in the original text
    let content_start_line = closing_line_idx + 1;
    let mut current_line = 0;
    let mut byte_pos = None;

    for (pos, ch) in text.char_indices() {
        if ch == '\n' {
            current_line += 1;
            if current_line == content_start_line {
                byte_pos = Some(pos + 1); // Start after the newline
                break;
            }
        }
    }

    // If byte_pos was never set, content starts at end of text (empty content)
    let content = match byte_pos {
        Some(pos) if pos < text.len() => &text[pos..],
        _ => "",
    };

    Ok((metadata, content))
}

/// Parses all Markdown files in the given directory.
pub fn parse_deck(directory: &PathBuf) -> Fallible<Vec<Card>> {
    let mut all_cards = Vec::new();
    for entry in WalkDir::new(directory) {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "md") {
            let text = read_to_string(path)?;

            // Extract frontmatter and get custom deck name if specified
            let (metadata, content) = extract_frontmatter(&text)?;

            let deck_name: DeckName = metadata.name.unwrap_or_else(|| {
                path.file_stem()
                    .and_then(|os_str| os_str.to_str())
                    .unwrap_or("None")
                    .to_string()
            });

            let parser = Parser::new(deck_name, path.to_path_buf());
            let cards = parser.parse(content)?;
            all_cards.extend(cards);
        }
    }

    // Cards are sorted by their hash to make subsequent code more
    // deterministic.
    all_cards.sort_by_key(|c| c.hash());

    // Remove duplicates.
    all_cards.dedup_by_key(|c| c.hash());

    Ok(all_cards)
}

pub struct Parser {
    deck_name: DeckName,
    file_path: PathBuf,
}

#[derive(Debug)]
pub struct ParserError {
    pub message: String,
    pub file_path: PathBuf,
    pub line_num: usize,
}

impl ParserError {
    fn new(message: impl Into<String>, file_path: PathBuf, line_num: usize) -> Self {
        ParserError {
            message: message.into(),
            file_path,
            line_num,
        }
    }
}

impl Display for ParserError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} Location: {}:{}",
            self.message,
            self.file_path.display(),
            self.line_num + 1
        )
    }
}

impl Error for ParserError {}

enum State {
    /// Start state.
    Start,
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
    /// End state.
    End,
}

enum Line {
    /// A line like `Q: <text>`.
    StartQuestion(String),
    /// A line like `A: <text>`.
    StartAnswer(String),
    /// A line like `C: <text>`.
    StartCloze(String),
    /// A line that's just `---` (flashcard separator).
    Separator,
    /// A line starting with `#`.
    Comment,
    /// Any other line.
    Text(String),
    /// End of file
    Eof,
}

impl Line {
    fn read(line: &str) -> Self {
        if is_question(line) {
            Line::StartQuestion(trim(line))
        } else if is_answer(line) {
            Line::StartAnswer(trim(line))
        } else if is_cloze(line) {
            Line::StartCloze(trim(line))
        } else if is_separator(line) {
            Line::Separator
        } else if is_comment(line) {
            Line::Comment
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

fn is_separator(line: &str) -> bool {
    line.trim() == "---"
}

fn is_comment(line: &str) -> bool {
    line.trim_start().starts_with('#')
}

fn trim(line: &str) -> String {
    line[2..].trim().to_string()
}

impl Parser {
    pub fn new(deck_name: DeckName, file_path: PathBuf) -> Self {
        Parser {
            deck_name,
            file_path,
        }
    }

    /// Parse all the cards in the given text.
    pub fn parse(&self, text: &str) -> Result<Vec<Card>, ParserError> {
        let mut cards = Vec::new();
        let mut state = State::Start;
        let lines: Vec<&str> = text.lines().collect();
        let last_line = if lines.is_empty() { 0 } else { lines.len() - 1 };
        for (line_num, line) in lines.iter().enumerate() {
            let line = Line::read(line);
            state = self.parse_line(state, line, line_num, &mut cards)?;
        }
        self.parse_line(state, Line::Eof, last_line, &mut cards)?;

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
    ) -> Result<State, ParserError> {
        match state {
            State::Start => match line {
                Line::StartQuestion(text) => Ok(State::ReadingQuestion {
                    question: text,
                    start_line: line_num,
                }),
                Line::StartAnswer(_) => Err(ParserError::new(
                    "Found answer tag without a question.",
                    self.file_path.clone(),
                    line_num,
                )),
                Line::StartCloze(text) => Ok(State::ReadingCloze {
                    text,
                    start_line: line_num,
                }),
                Line::Comment => Ok(State::Start),
                Line::Separator => Ok(State::Start),
                Line::Text(_) => Ok(State::Start),
                Line::Eof => Ok(State::End),
            },
            State::ReadingQuestion {
                question,
                start_line,
            } => match line {
                Line::Comment => Ok(State::ReadingQuestion {
                    question,
                    start_line,
                }),
                Line::StartQuestion(_) => Err(ParserError::new(
                    "New question without answer.",
                    self.file_path.clone(),
                    line_num,
                )),
                Line::StartAnswer(text) => Ok(State::ReadingAnswer {
                    question,
                    answer: text,
                    start_line,
                }),
                Line::StartCloze(_) => Err(ParserError::new(
                    "Found cloze tag while reading a question.",
                    self.file_path.clone(),
                    line_num,
                )),
                Line::Separator => Err(ParserError::new(
                    "Found flashcard separator while reading a question.",
                    self.file_path.clone(),
                    line_num,
                )),
                Line::Text(text) => Ok(State::ReadingQuestion {
                    question: format!("{question}\n{text}"),
                    start_line,
                }),
                Line::Eof => Err(ParserError::new(
                    "File ended while reading a question without an answer.",
                    self.file_path.clone(),
                    line_num,
                )),
            },
            State::ReadingAnswer {
                question,
                answer,
                start_line,
            } => {
                match line {
                    Line::Comment => Ok(State::ReadingAnswer {
                        question,
                        answer,
                        start_line,
                    }),
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
                    Line::StartAnswer(_) => Err(ParserError::new(
                        "Found answer tag while reading an answer.",
                        self.file_path.clone(),
                        line_num,
                    )),
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
                    Line::Separator => {
                        // Finalize the current card.
                        let card = Card::new(
                            self.deck_name.clone(),
                            self.file_path.clone(),
                            (start_line, line_num),
                            CardContent::new_basic(question, answer),
                        );
                        cards.push(card);
                        // Return to start state.
                        Ok(State::Start)
                    }
                    Line::Text(text) => Ok(State::ReadingAnswer {
                        question,
                        answer: format!("{answer}\n{text}"),
                        start_line,
                    }),
                    Line::Eof => {
                        // Finalize the current card.
                        let card = Card::new(
                            self.deck_name.clone(),
                            self.file_path.clone(),
                            (start_line, line_num),
                            CardContent::new_basic(question, answer),
                        );
                        cards.push(card);
                        Ok(State::End)
                    }
                }
            }
            State::ReadingCloze { text, start_line } => {
                match line {
                    Line::Comment => Ok(State::ReadingCloze { text, start_line }),
                    Line::StartQuestion(new_text) => {
                        // Finalize the previous cloze card.
                        cards.extend(self.parse_cloze_cards(text, start_line, line_num)?);
                        // Start a new question card
                        Ok(State::ReadingQuestion {
                            question: new_text,
                            start_line: line_num,
                        })
                    }
                    Line::StartAnswer(_) => Err(ParserError::new(
                        "Found answer tag while reading a cloze card.",
                        self.file_path.clone(),
                        line_num,
                    )),
                    Line::StartCloze(new_text) => {
                        // Finalize the previous card.
                        cards.extend(self.parse_cloze_cards(text, start_line, line_num)?);
                        // Start reading a new cloze card.
                        Ok(State::ReadingCloze {
                            text: new_text,
                            start_line: line_num,
                        })
                    }
                    Line::Separator => {
                        // Finalize the current cloze card.
                        cards.extend(self.parse_cloze_cards(text, start_line, line_num)?);
                        // Return to start state.
                        Ok(State::Start)
                    }
                    Line::Text(new_text) => Ok(State::ReadingCloze {
                        text: format!("{text}\n{new_text}"),
                        start_line,
                    }),
                    Line::Eof => {
                        // Finalize the current cloze card.
                        cards.extend(self.parse_cloze_cards(text, start_line, line_num)?);
                        Ok(State::End)
                    }
                }
            }
            State::End => unreachable!("Parsed a line after the end of the file."),
        }
    }

    fn parse_cloze_cards(
        &self,
        text: String,
        start_line: usize,
        end_line: usize,
    ) -> Result<Vec<Card>, ParserError> {
        let text = text.trim();
        let mut cards = Vec::new();

        // The full text of the card, without cloze deletion brackets.
        let clean_text: String = {
            let mut clean_text: Vec<u8> = Vec::new();
            // Flags to indicate should treat the next `[` or `]` differently.
            // Set when the preceeding byte indicates it should be evaluated as
            // markdown and not part of the cloze and therefore added to clean_text.
            let mut image_mode = false; // ![
            let mut escape_mode = false; // \[ and \]
            // We use `bytes` rather than `chars` because the cloze start/end
            // positions are byte positions, not character positions. This
            // keeps things tractable: bytes are well-understood, "characters"
            // are a vague abstract concept.
            for (bytepos, c) in text.bytes().enumerate() {
                if c == b'[' {
                    if image_mode {
                        clean_text.push(c);
                    }
                    if escape_mode {
                        escape_mode = false;
                        clean_text.push(c);
                    }
                } else if c == b']' {
                    if image_mode {
                        // We are in image mode, so this closing bracket is
                        // part of a Markdown image.
                        image_mode = false;
                        clean_text.push(c);
                    } else if escape_mode {
                        // We are in escape mode, so this closing bracket is
                        // part of the markdown text.
                        escape_mode = false;
                        clean_text.push(c);
                    }
                } else if c == b'!' {
                    if !image_mode {
                        // image_mode must be turned on *only* if the '!' is
                        // immediately before a `[`. Otherwise, exclamation
                        // marks in other positions would trigger it.
                        let nextopt = text.as_bytes().get(bytepos + 1).copied();
                        match nextopt {
                            Some(b'[') => {
                                image_mode = true;
                            }
                            _ => {}
                        }
                    }
                    clean_text.push(c);
                } else if c == b'\\' {
                    if !escape_mode {
                        // escape_mode must be turned on *only* if the '\' is
                        // immediately before a `[` or `]`. Otherwise, backslashes
                        // in other positions would trigger it.
                        let nextopt = text.as_bytes().get(bytepos + 1).copied();
                        match nextopt {
                            Some(b'[') | Some(b']') => {
                                escape_mode = true;
                            }
                            _ => {
                                clean_text.push(c);
                            }
                        }
                    }
                } else {
                    clean_text.push(c);
                }
            }
            match String::from_utf8(clean_text) {
                Ok(s) => s,
                Err(_) => {
                    return Err(ParserError::new(
                        "Cloze card contains invalid UTF-8.",
                        self.file_path.clone(),
                        start_line,
                    ));
                }
            }
        };

        let mut start = None;
        let mut index = 0;
        let mut image_mode = false;
        let mut escape_mode = false;
        for (bytepos, c) in text.bytes().enumerate() {
            if c == b'[' {
                if image_mode {
                    // We are in image mode, so this closing bracket is part of a markdown image.
                    index += 1;
                } else if escape_mode {
                    // We are in escape mode, so this closing bracket is part of a markdown text.
                    index += 1;
                    escape_mode = false;
                } else {
                    start = Some(index);
                }
            } else if c == b']' {
                if image_mode {
                    // We are in image mode, so this closing bracket is part of a markdown image.
                    image_mode = false;
                    index += 1;
                } else if escape_mode {
                    // We are in escape mode, so this closing bracket is part of a markdown text.
                    escape_mode = false;
                    index += 1;
                } else if let Some(s) = start {
                    let end = index;
                    let content = CardContent::new_cloze(clean_text.clone(), s, end - 1);
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
                if !image_mode {
                    // image_mode must be turned on *only* if the '!' is
                    // immediately before a `[`. Otherwise, exclamation
                    // marks in other positions would trigger it.
                    let nextopt = text.as_bytes().get(bytepos + 1).copied();
                    match nextopt {
                        Some(b'[') => {
                            image_mode = true;
                        }
                        _ => {}
                    }
                }
                index += 1;
            } else if c == b'\\' {
                if !escape_mode {
                    // escape_mode must be turned on *only* if the '\' is
                    // immediately before a `[` or `]`. Otherwise, backslashes
                    // in other positions would trigger it.
                    let nextopt = text.as_bytes().get(bytepos + 1).copied();
                    match nextopt {
                        Some(b'[') | Some(b']') => {
                            escape_mode = true;
                        }
                        _ => {
                            index += 1;
                        }
                    }
                }
            } else {
                index += 1;
            }
        }

        if cards.is_empty() {
            Err(ParserError::new(
                "Cloze card must contain at least one cloze deletion.",
                self.file_path.clone(),
                start_line,
            ))
        } else {
            Ok(cards)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::env::temp_dir;
    use std::fs::create_dir_all;

    use super::*;

    #[test]
    fn test_empty_string() -> Result<(), ParserError> {
        let input = "";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;
        assert_eq!(cards.len(), 0);
        Ok(())
    }

    #[test]
    fn test_whitespace_string() -> Result<(), ParserError> {
        let input = "\n\n\n";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;
        assert_eq!(cards.len(), 0);
        Ok(())
    }

    #[test]
    fn test_basic_card() -> Result<(), ParserError> {
        let input = "Q: What is Rust?\nA: A systems programming language.";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;

        assert_eq!(cards.len(), 1);
        assert!(matches!(
            &cards[0].content(),
            CardContent::Basic {
                question,
                answer,
            } if question == "What is Rust?" && answer == "A systems programming language."
        ));
        Ok(())
    }

    #[test]
    fn test_comments_ignored() -> Result<(), ParserError> {
        let input = "# Start\nQ: What is a question?\n#Center\nA: Not an answer!\n# End";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;

        assert_eq!(cards.len(), 1);
        assert!(matches!(
            cards[0].content(),
            CardContent::Basic {
                question,
                answer,
            } if question == "What is a question?" && answer == "Not an answer!"
        ));
        Ok(())
    }

    #[test]
    fn test_multiline_qa() -> Result<(), ParserError> {
        let input = "Q: foo\nbaz\nbaz\nA: FOO\nBAR\nBAZ";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;

        assert_eq!(cards.len(), 1);
        assert!(matches!(
            &cards[0].content(),
            CardContent::Basic {
                question,
                answer,
            } if question == "foo\nbaz\nbaz" && answer == "FOO\nBAR\nBAZ"
        ));
        Ok(())
    }

    #[test]
    fn test_two_questions() -> Result<(), ParserError> {
        let input = "Q: foo\nA: bar\n\nQ: baz\nA: quux\n\n";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;

        assert_eq!(cards.len(), 2);
        assert!(matches!(
            &cards[0].content(),
            CardContent::Basic {
                question,
                answer,
            } if question == "foo" && answer == "bar"
        ));
        assert!(matches!(
            &cards[1].content(),
            CardContent::Basic {
                question,
                answer,
            } if question == "baz" && answer == "quux"
        ));
        Ok(())
    }

    #[test]
    fn test_cloze_followed_by_question() -> Result<(), ParserError> {
        let input = "C: [foo]\nQ: Question\nA: Answer";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;

        assert_eq!(cards.len(), 2);
        assert_cloze(&cards[0..1], "foo", &[(0, 2)]);
        assert!(matches!(
            &cards[1].content(),
            CardContent::Basic {
                question,
                answer,
            } if question == "Question" && answer == "Answer"
        ));
        Ok(())
    }

    #[test]
    fn test_cloze_single() -> Result<(), ParserError> {
        let input = "C: Foo [bar] baz.";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;

        assert_cloze(&cards, "Foo bar baz.", &[(4, 6)]);
        Ok(())
    }

    #[test]
    fn test_cloze_multiple() -> Result<(), ParserError> {
        let input = "C: Foo [bar] baz [quux].";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;

        assert_cloze(&cards, "Foo bar baz quux.", &[(4, 6), (12, 15)]);
        Ok(())
    }

    #[test]
    fn test_cloze_with_image() -> Result<(), ParserError> {
        let input = "C: Foo [bar] ![](image.jpg) [quux].";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;

        assert_cloze(&cards, "Foo bar ![](image.jpg) quux.", &[(4, 6), (23, 26)]);
        Ok(())
    }

    #[test]
    fn test_cloze_with_escaped_square_bracket() -> Result<(), ParserError> {
        let input = "C: Key: [`\\[`]";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;

        assert_cloze(&cards, "Key: `[`", &[(5, 7)]);
        Ok(())
    }

    #[test]
    fn test_cloze_with_multiple_escaped_square_brackets() -> Result<(), ParserError> {
        let input = "C: \\[markdown\\] [`\\[cloze\\]`]";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;

        assert_cloze(&cards, "[markdown] `[cloze]`", &[(11, 19)]);
        Ok(())
    }

    #[test]
    fn test_multi_line_cloze() -> Result<(), ParserError> {
        let input = "C: [foo]\n[bar]\nbaz.";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;

        assert_cloze(&cards, "foo\nbar\nbaz.", &[(0, 2), (4, 6)]);
        Ok(())
    }

    #[test]
    fn test_two_clozes() -> Result<(), ParserError> {
        let input = "C: [foo]\nC: [bar]";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;

        assert_eq!(cards.len(), 2);
        assert_cloze(&cards[0..1], "foo", &[(0, 2)]);
        assert_cloze(&cards[1..2], "bar", &[(0, 2)]);
        Ok(())
    }

    #[test]
    fn test_question_without_answer() -> Result<(), ParserError> {
        let input = "Q: Question without answer";
        let parser = make_test_parser();
        let result = parser.parse(input);

        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_answer_without_question() -> Result<(), ParserError> {
        let input = "A: Answer without question";
        let parser = make_test_parser();
        let result = parser.parse(input);

        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_question_followed_by_cloze() -> Result<(), ParserError> {
        let input = "Q: Question\nC: Cloze";
        let parser = make_test_parser();
        let result = parser.parse(input);

        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_question_followed_by_question() -> Result<(), ParserError> {
        let input = "Q: Question\nQ: Another";
        let parser = make_test_parser();
        let result = parser.parse(input);

        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_multiple_answers() -> Result<(), ParserError> {
        let input = "Q: Question\nA: Answer\nA: Another answer";
        let parser = make_test_parser();
        let result = parser.parse(input);

        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_cloze_followed_by_answer() -> Result<(), ParserError> {
        let input = "C: Cloze\nA: Answer";
        let parser = make_test_parser();
        let result = parser.parse(input);

        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_cloze_without_deletions() -> Result<(), ParserError> {
        let input = "C: Cloze";
        let parser = make_test_parser();
        let result = parser.parse(input);

        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_cloze_with_initial_blank_line() -> Result<(), ParserError> {
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
        let directory = PathBuf::from("./test");
        let deck = parse_deck(&directory);

        assert!(deck.is_ok());
        let cards = deck?;
        assert_eq!(cards.len(), 2);
        Ok(())
    }

    #[test]
    fn test_identical_basic_cards() -> Result<(), ParserError> {
        let input = "Q: foo\nA: bar\n\nQ: foo\nA: bar\n\n";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;

        assert_eq!(cards.len(), 1);
        Ok(())
    }

    #[test]
    fn test_identical_cloze_cards() -> Result<(), ParserError> {
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
        create_dir_all(&directory).expect("Failed to create test directory");
        let file1 = directory.join("file1.md");
        let file2 = directory.join("file2.md");
        std::fs::write(&file1, "Q: foo\nA: bar").expect("Failed to write test file");
        std::fs::write(&file2, "Q: foo\nA: bar").expect("Failed to write test file");
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
            assert!(matches!(
                &cards[i].content(),
                CardContent::Cloze {
                    text,
                    start: s,
                    end: e,
                } if text == clean_text && *s == *start && *e == *end
            ));
        }
    }

    /// Parsing invalid UTF-8.
    ///
    /// This is tricky to test directly because Rust strings are UTF-8. We can
    /// simulate it by creating a byte array with invalid UTF-8, and using an
    /// unsafe method to convert it to a string without validation.
    #[test]
    fn test_invalid_utf8() {
        let input = unsafe {
            #[allow(invalid_from_utf8_unchecked)]
            std::str::from_utf8_unchecked(b"C: Valid text [\xFF\xFF]")
        };
        let parser = make_test_parser();
        let result = parser.parse(input);
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert_eq!(
            err.to_string(),
            "Cloze card contains invalid UTF-8. Location: test.md:1"
        );
    }

    /// See: <https://github.com/eudoxia0/hashcards/issues/29>
    #[test]
    fn test_cloze_deletion_with_exclamation_sign() -> Result<(), ParserError> {
        let input = "C: The notation [$n!$] means 'n factorial'.";
        let parser = make_test_parser();
        let result = parser.parse(input);
        let cards = result.unwrap();
        assert_eq!(cards.len(), 1);
        let card: Card = cards[0].clone();
        match &card.content() {
            CardContent::Cloze { text, .. } => {
                assert_eq!(text, "The notation $n!$ means 'n factorial'.");
            }
            _ => panic!("Expected cloze card."),
        }
        Ok(())
    }

    #[test]
    fn test_cloze_deletion_with_math() -> Result<(), ParserError> {
        let input = "C: The string `\\alpha` renders as [$\\alpha$].";
        let parser = make_test_parser();
        let result = parser.parse(input);
        let cards = result.unwrap();
        assert_eq!(cards.len(), 1);
        let card: Card = cards[0].clone();
        match &card.content() {
            CardContent::Cloze { text, .. } => {
                assert_eq!(text, "The string `\\alpha` renders as $\\alpha$.");
            }
            _ => panic!("Expected cloze card."),
        }
        Ok(())
    }

    #[test]
    fn test_extract_frontmatter_with_name() {
        let input = r#"---
name = "Custom Deck Name"
---

Q: What is Rust?
A: A systems programming language."#;

        let result = extract_frontmatter(input);
        assert!(result.is_ok());
        let (metadata, content) = result.unwrap();
        assert_eq!(metadata.name, Some("Custom Deck Name".to_string()));
        assert_eq!(
            content.trim(),
            "Q: What is Rust?\nA: A systems programming language."
        );
    }

    #[test]
    fn test_extract_frontmatter_without_name() {
        let input = r#"---
other_field = "value"
---

Q: What is Rust?
A: A systems programming language."#;

        let result = extract_frontmatter(input);
        assert!(result.is_ok());
        let (metadata, content) = result.unwrap();
        assert_eq!(metadata.name, None);
        assert_eq!(
            content.trim(),
            "Q: What is Rust?\nA: A systems programming language."
        );
    }

    #[test]
    fn test_extract_frontmatter_empty() {
        let input = r#"---
---

Q: What is Rust?
A: A systems programming language."#;

        let result = extract_frontmatter(input);
        assert!(result.is_ok());
        let (metadata, content) = result.unwrap();
        assert_eq!(metadata.name, None);
        assert_eq!(
            content.trim(),
            "Q: What is Rust?\nA: A systems programming language."
        );
    }

    #[test]
    fn test_no_frontmatter() {
        let input = "Q: What is Rust?\nA: A systems programming language.";
        let result = extract_frontmatter(input);
        assert!(result.is_ok());
        let (metadata, content) = result.unwrap();
        assert_eq!(metadata.name, None);
        assert_eq!(content, input);
    }

    #[test]
    fn test_frontmatter_unclosed() {
        let input = r#"---
name = "Custom Deck Name"

Q: What is Rust?
A: A systems programming language."#;

        let result = extract_frontmatter(input);
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.to_string().contains("no closing '---'"));
    }

    #[test]
    fn test_frontmatter_invalid_toml() {
        let input = r#"---
name = Custom Deck Name (missing quotes)
---

Q: What is Rust?"#;

        let result = extract_frontmatter(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_with_frontmatter() -> Result<(), ParserError> {
        let input = r#"---
name = "Custom Deck Name"
---

Q: What is Rust?
A: A systems programming language."#;

        let (metadata, content) = extract_frontmatter(input).unwrap();
        assert_eq!(metadata.name, Some("Custom Deck Name".to_string()));

        let parser = make_test_parser();
        let cards = parser.parse(content)?;
        assert_eq!(cards.len(), 1);
        Ok(())
    }

    #[test]
    fn test_parse_deck_with_frontmatter() -> Fallible<()> {
        let directory = temp_dir();
        let directory = directory.join("frontmatter_test");
        create_dir_all(&directory).expect("Failed to create test directory");

        let file1 = directory.join("ch1.md");
        let file2 = directory.join("ch2.md");

        std::fs::write(
            &file1,
            r#"---
name = "Cell Biology"
---

Q: What is a cell?
A: The basic unit of life."#,
        )
        .expect("Failed to write test file");

        std::fs::write(
            &file2,
            r#"---
name = "Cell Biology"
---

Q: What is DNA?
A: Genetic material."#,
        )
        .expect("Failed to write test file");

        let deck = parse_deck(&directory)?;

        // Both cards should have the custom deck name "Cell Biology"
        assert_eq!(deck.len(), 2);
        for card in &deck {
            assert_eq!(card.deck_name(), "Cell Biology");
        }

        // Clean up
        std::fs::remove_dir_all(&directory).ok();

        Ok(())
    }

    #[test]
    fn test_separator_between_basic_cards() -> Result<(), ParserError> {
        let input = "Q: foo\nA: bar\n---\nQ: baz\nA: quux";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;

        assert_eq!(cards.len(), 2);
        assert!(matches!(
            &cards[0].content(),
            CardContent::Basic {
                question,
                answer,
            } if question == "foo" && answer == "bar"
        ));
        assert!(matches!(
            &cards[1].content(),
            CardContent::Basic {
                question,
                answer,
            } if question == "baz" && answer == "quux"
        ));
        Ok(())
    }

    #[test]
    fn test_separator_after_cloze_card() -> Result<(), ParserError> {
        let input = "C: [foo]\n---\nQ: Question\nA: Answer";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;

        assert_eq!(cards.len(), 2);
        assert_cloze(&cards[0..1], "foo", &[(0, 2)]);
        assert!(matches!(
            &cards[1].content(),
            CardContent::Basic {
                question,
                answer,
            } if question == "Question" && answer == "Answer"
        ));
        Ok(())
    }

    #[test]
    fn test_separator_between_cloze_cards() -> Result<(), ParserError> {
        let input = "C: [foo]\n---\nC: [bar]";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;

        assert_eq!(cards.len(), 2);
        assert_cloze(&cards[0..1], "foo", &[(0, 2)]);
        assert_cloze(&cards[1..2], "bar", &[(0, 2)]);
        Ok(())
    }

    #[test]
    fn test_separator_in_question_errors() -> Result<(), ParserError> {
        let input = "Q: Question\n---\nA: Answer";
        let parser = make_test_parser();
        let result = parser.parse(input);

        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("separator"));
        }
        Ok(())
    }

    #[test]
    fn test_multiple_separators() -> Result<(), ParserError> {
        let input = "Q: foo\nA: bar\n---\n---\nQ: baz\nA: quux";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;

        assert_eq!(cards.len(), 2);
        assert!(matches!(
            &cards[0].content(),
            CardContent::Basic {
                question,
                answer,
            } if question == "foo" && answer == "bar"
        ));
        assert!(matches!(
            &cards[1].content(),
            CardContent::Basic {
                question,
                answer,
            } if question == "baz" && answer == "quux"
        ));
        Ok(())
    }

    #[test]
    fn test_separator_at_end() -> Result<(), ParserError> {
        let input = "Q: foo\nA: bar\n---";
        let parser = make_test_parser();
        let cards = parser.parse(input)?;

        assert_eq!(cards.len(), 1);
        assert!(matches!(
            &cards[0].content(),
            CardContent::Basic {
                question,
                answer,
            } if question == "foo" && answer == "bar"
        ));
        Ok(())
    }
}
