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

use std::path::Path;
use std::path::PathBuf;

use pulldown_cmark::Event;
use pulldown_cmark::Parser;
use pulldown_cmark::Tag;

use crate::error::Fallible;
use crate::types::card::Card;
use crate::types::card::CardContent;

/// The media canonicalizer processes cards and canonicalizes media paths in
/// their markdown content.
pub struct MediaCanonicalizer {
    /// Path to the collection root directory.
    root: PathBuf,
}

impl MediaCanonicalizer {
    /// Create a new media canonicalizer.
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Canonicalize media paths in a vector of cards.
    ///
    /// Returns a new vector of cards with canonicalized media paths in their
    /// markdown content.
    pub fn canonicalize_cards(&self, cards: Vec<Card>) -> Fallible<Vec<Card>> {
        let mut result = Vec::new();
        for card in cards {
            result.push(self.canonicalize_card(card)?);
        }
        Ok(result)
    }

    /// Canonicalize media paths in a single card.
    fn canonicalize_card(&self, card: Card) -> Fallible<Card> {
        let deck_file_dir = card
            .file_path()
            .parent()
            .expect("Card file path should have a parent directory");

        let new_content = match card.content() {
            CardContent::Basic { question, answer } => {
                let new_question = self.canonicalize_markdown(question, deck_file_dir)?;
                let new_answer = self.canonicalize_markdown(answer, deck_file_dir)?;
                CardContent::new_basic(new_question, new_answer)
            }
            CardContent::Cloze { text, start, end } => {
                let (new_text, new_start, new_end) =
                    self.canonicalize_markdown_with_positions(text, *start, *end, deck_file_dir)?;
                CardContent::new_cloze(new_text, new_start, new_end)
            }
        };

        Ok(Card::new(
            card.deck_name().clone(),
            card.file_path().clone(),
            card.range(),
            new_content,
        ))
    }

    /// Canonicalize media paths in markdown text.
    ///
    /// Rules:
    /// - External URLs (containing "://") are left intact.
    /// - Paths starting with "@/" are left intact (collection-relative).
    /// - Relative paths are merged with the deck file directory to create
    ///   "@/" prefixed (collection-relative) paths.
    fn canonicalize_markdown(&self, markdown: &str, deck_file_dir: &Path) -> Fallible<String> {
        // Collect all image URLs and their replacements
        let parser = Parser::new(markdown);
        let mut replacements: Vec<(String, String)> = Vec::new();

        for event in parser {
            if let Event::Start(Tag::Image { dest_url, .. }) = event {
                let original = dest_url.to_string();
                let canonical = self.canonicalize_path(&original, deck_file_dir)?;
                if original != canonical {
                    replacements.push((original, canonical));
                }
            }
        }

        // Apply replacements to the markdown text
        let mut result = markdown.to_string();
        for (original, canonical) in replacements {
            // Replace image URLs in markdown: ![alt](url) -> ![alt](canonical_url)
            // We need to be careful to only replace the URL part, not arbitrary occurrences
            // of the string. We'll look for the pattern ]( + url + )
            let pattern = format!("]({})", original);
            let replacement = format!("]({})", canonical);
            result = result.replace(&pattern, &replacement);
        }

        Ok(result)
    }

    /// Canonicalize media paths in markdown text and adjust byte positions.
    ///
    /// This is used for cloze cards where we need to maintain the validity of
    /// byte positions after text modifications.
    fn canonicalize_markdown_with_positions(
        &self,
        markdown: &str,
        start: usize,
        end: usize,
        deck_file_dir: &Path,
    ) -> Fallible<(String, usize, usize)> {
        // Collect all image URLs and their byte positions
        let parser = Parser::new(markdown);
        let mut replacements: Vec<(usize, String, String)> = Vec::new();

        // We need to find the byte position of each URL in the markdown
        for event in parser.into_offset_iter() {
            if let (Event::Start(Tag::Image { dest_url, .. }), range) = event {
                let original = dest_url.to_string();
                let canonical = self.canonicalize_path(&original, deck_file_dir)?;
                if original != canonical {
                    // Find the position of the URL in the markdown
                    // The URL appears after "](" in the image syntax
                    let pattern = format!("]({})", original);
                    if let Some(pos) = markdown[range.start..].find(&pattern) {
                        let url_start = range.start + pos + 2; // +2 for "]("
                        replacements.push((url_start, original, canonical));
                    }
                }
            }
        }

        // Sort replacements by position (descending) so we can apply them
        // from end to start without invalidating positions
        replacements.sort_by(|a, b| b.0.cmp(&a.0));

        let mut result = markdown.to_string();
        let mut new_start = start;
        let mut new_end = end;

        for (url_start, original, canonical) in replacements {
            let url_end = url_start + original.len();
            let delta = canonical.len() as i64 - original.len() as i64;

            // Replace the URL in the text
            result.replace_range(url_start..url_end, &canonical);

            // Adjust cloze positions if they come after this replacement
            if url_start <= start {
                new_start = (new_start as i64 + delta) as usize;
                new_end = (new_end as i64 + delta) as usize;
            } else if url_start <= end {
                // The replacement is within the cloze deletion range
                // This is unusual but we should handle it
                new_end = (new_end as i64 + delta) as usize;
            }
        }

        Ok((result, new_start, new_end))
    }

    /// Canonicalize a single media path.
    ///
    /// Rules:
    /// - External URLs (containing "://") are left intact.
    /// - Paths starting with "@/" are left intact (collection-relative).
    /// - Relative paths are merged with the deck file directory to create
    ///   "@/" prefixed (collection-relative) paths.
    fn canonicalize_path(&self, path: &str, deck_file_dir: &Path) -> Fallible<String> {
        // External URLs are left intact
        if path.contains("://") {
            return Ok(path.to_string());
        }

        // Paths starting with "@/" are already canonical
        if path.starts_with("@/") {
            return Ok(path.to_string());
        }

        // Relative paths: merge with deck file directory
        let full_path = deck_file_dir.join(path);

        // Convert to a path relative to the collection root
        let relative_to_root = full_path.strip_prefix(&self.root).map_err(|_| {
            crate::error::ErrorReport::new(&format!(
                "Path {} is outside the collection directory",
                full_path.display()
            ))
        })?;

        // Convert to @/ prefixed path
        let canonical = format!("@/{}", relative_to_root.display());

        Ok(canonical)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helper::create_tmp_directory;
    use std::fs::create_dir_all;

    #[test]
    fn test_canonicalize_path_external_url() -> Fallible<()> {
        let root = create_tmp_directory()?;
        let canonicalizer = MediaCanonicalizer::new(root.clone());
        let deck_file_dir = root.join("decks");

        let url = "https://example.com/image.jpg";
        let result = canonicalizer.canonicalize_path(url, &deck_file_dir)?;
        assert_eq!(result, url);

        Ok(())
    }

    #[test]
    fn test_canonicalize_path_already_canonical() -> Fallible<()> {
        let root = create_tmp_directory()?;
        let canonicalizer = MediaCanonicalizer::new(root.clone());
        let deck_file_dir = root.join("decks");

        let path = "@/images/photo.jpg";
        let result = canonicalizer.canonicalize_path(path, &deck_file_dir)?;
        assert_eq!(result, path);

        Ok(())
    }

    #[test]
    fn test_canonicalize_path_relative() -> Fallible<()> {
        let root = create_tmp_directory()?;
        let deck_dir = root.join("decks");
        create_dir_all(&deck_dir)?;

        let canonicalizer = MediaCanonicalizer::new(root.clone());

        // Relative path in the same directory as the deck
        let result = canonicalizer.canonicalize_path("image.jpg", &deck_dir)?;
        assert_eq!(result, "@/decks/image.jpg");

        Ok(())
    }

    #[test]
    fn test_canonicalize_path_relative_subdirectory() -> Fallible<()> {
        let root = create_tmp_directory()?;
        let deck_dir = root.join("decks");
        create_dir_all(&deck_dir)?;

        let canonicalizer = MediaCanonicalizer::new(root.clone());

        // Relative path to a subdirectory
        let result = canonicalizer.canonicalize_path("images/photo.jpg", &deck_dir)?;
        assert_eq!(result, "@/decks/images/photo.jpg");

        Ok(())
    }

    #[test]
    fn test_canonicalize_path_relative_parent() -> Fallible<()> {
        let root = create_tmp_directory()?;
        let deck_dir = root.join("decks").join("subdeck");
        create_dir_all(&deck_dir)?;

        let canonicalizer = MediaCanonicalizer::new(root.clone());

        // Relative path going up to parent directory
        let result = canonicalizer.canonicalize_path("../image.jpg", &deck_dir)?;
        assert_eq!(result, "@/decks/image.jpg");

        Ok(())
    }

    #[test]
    fn test_canonicalize_markdown_basic() -> Fallible<()> {
        let root = create_tmp_directory()?;
        let deck_dir = root.join("decks");
        create_dir_all(&deck_dir)?;

        let canonicalizer = MediaCanonicalizer::new(root.clone());

        let markdown = "Here is an image: ![alt](image.jpg) and another ![](photo.png)";
        let result = canonicalizer.canonicalize_markdown(markdown, &deck_dir)?;

        assert_eq!(
            result,
            "Here is an image: ![alt](@/decks/image.jpg) and another ![](@/decks/photo.png)"
        );

        Ok(())
    }

    #[test]
    fn test_canonicalize_markdown_mixed_paths() -> Fallible<()> {
        let root = create_tmp_directory()?;
        let deck_dir = root.join("decks");
        create_dir_all(&deck_dir)?;

        let canonicalizer = MediaCanonicalizer::new(root.clone());

        let markdown =
            "External: ![](https://example.com/img.jpg) Canonical: ![](@/media/img.jpg) Relative: ![](local.jpg)";
        let result = canonicalizer.canonicalize_markdown(markdown, &deck_dir)?;

        assert_eq!(
            result,
            "External: ![](https://example.com/img.jpg) Canonical: ![](@/media/img.jpg) Relative: ![](@/decks/local.jpg)"
        );

        Ok(())
    }

    #[test]
    fn test_canonicalize_card_basic() -> Fallible<()> {
        let root = create_tmp_directory()?;
        let deck_file = root.join("decks").join("test.md");
        create_dir_all(deck_file.parent().unwrap())?;

        let canonicalizer = MediaCanonicalizer::new(root.clone());

        let card = Card::new(
            "test".to_string(),
            deck_file.clone(),
            (1, 10),
            CardContent::new_basic(
                "What is this? ![](question.jpg)",
                "Answer: ![](answer.jpg)",
            ),
        );

        let result = canonicalizer.canonicalize_card(card)?;

        match result.content() {
            CardContent::Basic { question, answer } => {
                assert_eq!(question, "What is this? ![](@/decks/question.jpg)");
                assert_eq!(answer, "Answer: ![](@/decks/answer.jpg)");
            }
            _ => panic!("Expected Basic card"),
        }

        Ok(())
    }

    #[test]
    fn test_canonicalize_card_cloze() -> Fallible<()> {
        let root = create_tmp_directory()?;
        let deck_file = root.join("decks").join("test.md");
        create_dir_all(deck_file.parent().unwrap())?;

        let canonicalizer = MediaCanonicalizer::new(root.clone());

        // Text: "The [capital] of France is Paris"
        // The cloze deletion is "capital" at byte positions 4-11
        let text = "The [capital] of France is Paris";
        let card = Card::new(
            "test".to_string(),
            deck_file.clone(),
            (1, 10),
            CardContent::new_cloze(text, 5, 11),
        );

        let result = canonicalizer.canonicalize_card(card)?;

        match result.content() {
            CardContent::Cloze { text, start, end } => {
                assert_eq!(text, "The [capital] of France is Paris");
                assert_eq!(*start, 5);
                assert_eq!(*end, 11);
            }
            _ => panic!("Expected Cloze card"),
        }

        Ok(())
    }

    #[test]
    fn test_canonicalize_card_cloze_with_image() -> Fallible<()> {
        let root = create_tmp_directory()?;
        let deck_file = root.join("decks").join("test.md");
        create_dir_all(deck_file.parent().unwrap())?;

        let canonicalizer = MediaCanonicalizer::new(root.clone());

        // Text with image before cloze: "See ![](img.jpg) The [capital] of France"
        // The cloze deletion "capital" starts at byte 21 (after "See ![](img.jpg) The [")
        let text = "See ![](img.jpg) The [capital] of France";
        let start = 21;
        let end = 27; // "capital"

        let card = Card::new(
            "test".to_string(),
            deck_file.clone(),
            (1, 10),
            CardContent::new_cloze(text, start, end),
        );

        let result = canonicalizer.canonicalize_card(card)?;

        match result.content() {
            CardContent::Cloze {
                text: new_text,
                start: new_start,
                end: new_end,
            } => {
                // The image URL changed from "img.jpg" (7 chars) to "@/decks/img.jpg" (15 chars)
                // That's a delta of +8 bytes
                assert_eq!(new_text, "See ![](@/decks/img.jpg) The [capital] of France");
                assert_eq!(*new_start, start + 8);
                assert_eq!(*new_end, end + 8);
            }
            _ => panic!("Expected Cloze card"),
        }

        Ok(())
    }

    #[test]
    fn test_canonicalize_cards_batch() -> Fallible<()> {
        let root = create_tmp_directory()?;
        let deck_file = root.join("decks").join("test.md");
        create_dir_all(deck_file.parent().unwrap())?;

        let canonicalizer = MediaCanonicalizer::new(root.clone());

        let cards = vec![
            Card::new(
                "test".to_string(),
                deck_file.clone(),
                (1, 5),
                CardContent::new_basic("Q: ![](q1.jpg)", "A: ![](a1.jpg)"),
            ),
            Card::new(
                "test".to_string(),
                deck_file.clone(),
                (6, 10),
                CardContent::new_basic("Q: ![](q2.jpg)", "A: ![](a2.jpg)"),
            ),
        ];

        let results = canonicalizer.canonicalize_cards(cards)?;

        assert_eq!(results.len(), 2);

        match results[0].content() {
            CardContent::Basic { question, answer } => {
                assert_eq!(question, "Q: ![](@/decks/q1.jpg)");
                assert_eq!(answer, "A: ![](@/decks/a1.jpg)");
            }
            _ => panic!("Expected Basic card"),
        }

        match results[1].content() {
            CardContent::Basic { question, answer } => {
                assert_eq!(question, "Q: ![](@/decks/q2.jpg)");
                assert_eq!(answer, "A: ![](@/decks/a2.jpg)");
            }
            _ => panic!("Expected Basic card"),
        }

        Ok(())
    }
}
