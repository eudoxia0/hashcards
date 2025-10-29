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

/// Represents a missing media file reference.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
        let markdown_texts = match card.content() {
            CardContent::Basic { question, answer } => vec![question.as_str(), answer.as_str()],
            CardContent::Cloze { text, .. } => vec![text.as_str()],
        };

        // Extract and validate media paths.
        for markdown in markdown_texts {
            for path in extract_media_paths(markdown) {
                // Skip URLs (http://, https://, etc.)
                if path.contains("://") {
                    continue;
                }

                // Check if file exists.
                let full_path = base_dir.join(&path);
                if !full_path.exists() {
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
        let mut missing: Vec<_> = missing.into_iter().collect();
        missing.sort_by(|a, b| {
            a.card_file
                .cmp(&b.card_file)
                .then_with(|| a.card_lines.cmp(&b.card_lines))
                .then_with(|| a.file_path.cmp(&b.file_path))
        });

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
    use super::*;

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
}
