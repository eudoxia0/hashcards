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
use std::path::Path;
use std::path::PathBuf;

use pulldown_cmark::Event;
use pulldown_cmark::Parser;
use pulldown_cmark::Tag;

use crate::error::ErrorReport;
use crate::error::Fallible;
use crate::types::card::Card;
use crate::types::card::CardContent;
use crate::utils::resolve_media_path;

/// Represents a missing media file reference.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MissingMedia {
    pub file_path: String,
    pub card_file: PathBuf,
    pub card_lines: (usize, usize),
}

/// Extract all media file paths from markdown text.
fn extract_media_paths(markdown: &str) -> Vec<String> {
    let parser = Parser::new(markdown);
    let mut paths = Vec::new();

    for event in parser {
        if let Event::Start(Tag::Image { dest_url, .. }) = event {
            paths.push(dest_url.to_string());
        }
    }

    paths
}

/// Validate that all media files referenced in cards exist.
pub fn validate_media_files(cards: &[Card], base_dir: &Path) -> Fallible<()> {
    let mut missing = HashSet::new();

    for card in cards {
        // Extract markdown content from the card.
        //
        // TODO: perhaps this should be lifted to a method of the `CardContent`
        // enum.
        let markdown_texts = match card.content() {
            CardContent::Basic { question, answer } => vec![question.as_str(), answer.as_str()],
            CardContent::Cloze { text, .. } => vec![text.as_str()],
        };

        for markdown in markdown_texts {
            for path in extract_media_paths(markdown) {
                // Skip URLs (http://, https://, etc.)
                if path.contains("://") {
                    continue;
                }

                // Resolve path according to hashcards rules
                let resolved_path = match resolve_media_path(card.file_path(), base_dir, &path) {
                    Ok(p) => base_dir.join(p),
                    Err(_) => {
                        // If resolution fails, mark as missing
                        missing.insert(MissingMedia {
                            file_path: path.clone(),
                            card_file: card.file_path().clone(),
                            card_lines: card.range(),
                        });
                        continue;
                    }
                };

                // Check if file exists
                if !resolved_path.exists() {
                    missing.insert(MissingMedia {
                        file_path: path,
                        card_file: card.file_path().clone(),
                        card_lines: card.range(),
                    });
                }
            }
        }
    }

    if !missing.is_empty() {
        // Sort missing files for consistent error messages.
        let mut missing: Vec<MissingMedia> = missing.into_iter().collect();
        missing.sort();

        // Build error message.
        let mut msg = String::from("Missing media files referenced in cards:\n");
        for m in missing {
            msg.push_str(&format!(
                "  - {} (referenced in {}:{})\n",
                m.file_path,
                m.card_file.display(),
                m.card_lines.0
            ));
        }

        return Err(ErrorReport::new(&msg));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::env::temp_dir;
    use std::fs::create_dir_all;

    use super::*;
    use crate::parser::Parser as CardParser;

    #[test]
    fn test_extract_media_paths() {
        let markdown = "Here is an image: ![alt](foo.jpg)\nAnd another: ![](bar.png)";
        let paths = extract_media_paths(markdown);
        assert_eq!(paths, vec!["foo.jpg", "bar.png"]);
    }

    #[test]
    fn test_extract_media_paths_with_audio() {
        let markdown = "Audio file: ![](sound.mp3)";
        let paths = extract_media_paths(markdown);
        assert_eq!(paths, vec!["sound.mp3"]);
    }

    #[test]
    fn test_extract_media_paths_no_media() {
        let markdown = "Just some **bold** text.";
        let paths = extract_media_paths(markdown);
        assert!(paths.is_empty());
    }

    #[test]
    fn test_extract_media_paths_with_urls() {
        let markdown = "![](https://example.com/image.jpg) and ![](local.png)";
        let paths = extract_media_paths(markdown);
        assert_eq!(paths, vec!["https://example.com/image.jpg", "local.png"]);
    }

    #[test]
    fn test_validate_media_files_with_missing_files() {
        // Create a temporary directory for the test
        let test_dir = temp_dir().join("hashcards_media_test");
        create_dir_all(&test_dir).expect("Failed to create test directory");

        // Create the deck file (needed for path canonicalization)
        let card_file = test_dir.join("test_deck.md");
        std::fs::write(&card_file, b"").expect("Failed to create deck file");

        // Parse cards from markdown with missing media references
        let markdown = "Q: What is this image?\n\n![](missing_image.jpg)\n\nA: Unknown\n\nQ: What is this audio?\nA: ![](missing_audio.mp3)";
        let parser = CardParser::new("test_deck".to_string(), card_file.clone());
        let cards = parser.parse(markdown).expect("Failed to parse cards");

        // Validate media files - should return an error
        let result = validate_media_files(&cards, &test_dir);

        // Assert that validation failed
        assert!(result.is_err());

        // Assert the error message contains expected information
        let err = result.err().unwrap();
        let err_msg = err.to_string();

        assert!(err_msg.contains("Missing media files referenced in cards:"));
        assert!(err_msg.contains("missing_image.jpg"));
        assert!(err_msg.contains("missing_audio.mp3"));
        assert!(err_msg.contains("test_deck.md"));
    }

    #[test]
    fn test_validate_media_files_with_existing_files() {
        // Create a temporary directory for the test
        let test_dir = temp_dir().join("hashcards_media_test_existing");
        create_dir_all(&test_dir).expect("Failed to create test directory");

        // Create actual media files
        let image_path = test_dir.join("existing_image.jpg");
        std::fs::write(&image_path, b"fake image data").expect("Failed to create test image");

        // Create the deck file (needed for path canonicalization)
        let card_file = test_dir.join("test_deck.md");
        std::fs::write(&card_file, b"").expect("Failed to create deck file");

        // Parse cards from markdown with existing media reference
        let markdown = "Q: What is this image?\n\n![](existing_image.jpg)\n\nA: A test image";
        let parser = CardParser::new("test_deck".to_string(), card_file.clone());
        let cards = parser.parse(markdown).expect("Failed to parse cards");

        // Validate media files - should succeed
        let result = validate_media_files(&cards, &test_dir);

        // Assert that validation succeeded
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_media_files_skips_urls() {
        // Create a temporary directory for the test
        let test_dir = temp_dir().join("hashcards_media_test_urls");
        create_dir_all(&test_dir).expect("Failed to create test directory");

        // Create the deck file (needed for path canonicalization)
        let card_file = test_dir.join("test_deck.md");
        std::fs::write(&card_file, b"").expect("Failed to create deck file");

        // Parse cards from markdown with external URL (should be skipped)
        let markdown = "Q: What is this?\nA: ![](https://example.com/image.jpg)";
        let parser = CardParser::new("test_deck".to_string(), card_file.clone());
        let cards = parser.parse(markdown).expect("Failed to parse cards");

        // Validate media files - should succeed because URLs are skipped
        let result = validate_media_files(&cards, &test_dir);

        // Assert that validation succeeded
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_media_files_with_cloze_cards() {
        // Create a temporary directory for the test
        let test_dir = temp_dir().join("hashcards_media_test_cloze");
        create_dir_all(&test_dir).expect("Failed to create test directory");

        // Create the deck file (needed for path canonicalization)
        let card_file = test_dir.join("test_deck.md");
        std::fs::write(&card_file, b"").expect("Failed to create deck file");

        // Parse cloze card with missing media reference
        let markdown = "C: The capital of [France] is ![](paris.jpg)";
        let parser = CardParser::new("test_deck".to_string(), card_file.clone());
        let cards = parser.parse(markdown).expect("Failed to parse cards");

        // Validate media files - should fail
        let result = validate_media_files(&cards, &test_dir);

        assert!(result.is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(err_msg.contains("paris.jpg"));
    }
}
