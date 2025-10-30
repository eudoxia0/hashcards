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

use pulldown_cmark::CowStr;
use pulldown_cmark::Event;
use pulldown_cmark::Parser;
use pulldown_cmark::Tag;
use pulldown_cmark::html::push_html;

use crate::error::ErrorReport;
use crate::error::Fallible;

const AUDIO_EXTENSIONS: [&str; 3] = ["mp3", "wav", "ogg"];

fn is_audio_file(url: &str) -> bool {
    if let Some(ext) = url.split('.').next_back() {
        AUDIO_EXTENSIONS.contains(&ext)
    } else {
        false
    }
}

/// Configuration for the Markdown renderer.
pub struct MarkdownRendererConfig {
    /// The port where the server is running. This is used to construct URLs
    /// for media files.
    pub port: u16,
}

/// Render Markdown to HTML.
pub fn markdown_to_html(markdown: &str, config: &MarkdownRendererConfig) -> Fallible<String> {
    let parser = Parser::new(markdown);
    let mut errors = Vec::new();

    let parser = parser.map(|event| match event {
        Event::Start(Tag::Image {
            link_type,
            title,
            dest_url,
            id,
        }) => {
            let url = match modify_url(&dest_url, config) {
                Ok(url) => url,
                Err(e) => {
                    errors.push(e);
                    dest_url.to_string()
                }
            };
            // Does the URL point to an audio file?
            if is_audio_file(&url) {
                // If so, render it as an HTML5 audio element.
                Event::Html(CowStr::Boxed(
                    format!(
                        r#"<audio controls src="{}" title="{}"></audio>"#,
                        url, title
                    )
                    .into_boxed_str(),
                ))
            } else {
                // Treat it as a normal image.
                Event::Start(Tag::Image {
                    link_type,
                    title,
                    dest_url: CowStr::Boxed(url.into_boxed_str()),
                    id,
                })
            }
        }
        _ => event,
    });
    let mut html_output = String::new();
    push_html(&mut html_output, parser);

    if !errors.is_empty() {
        let error_messages: Vec<String> = errors.into_iter().map(|e| e.to_string()).collect();
        let combined_message = format!(
            "Failed to resolve media paths:\n{}",
            error_messages.join("\n")
        );
        return Err(ErrorReport::new(&combined_message));
    }

    Ok(html_output)
}

pub fn markdown_to_html_inline(
    markdown: &str,
    config: &MarkdownRendererConfig,
) -> Fallible<String> {
    let text = markdown_to_html(markdown, config)?;
    if text.starts_with("<p>") && text.ends_with("</p>\n") {
        let len = text.len();
        Ok(text[3..len - 5].to_string())
    } else {
        Ok(text)
    }
}

fn modify_url(url: &str, config: &MarkdownRendererConfig) -> Fallible<String> {
    // Skip external URLs
    if url.contains("://") {
        Ok(url.to_string())
    } else {
        let port = config.port;
        Ok(format!("http://localhost:{port}/file/{url}"))
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn test_markdown_to_html() {
        // Create a temporary directory structure for testing
        let temp_dir = tempdir().unwrap();
        let deck_file = temp_dir.path().join("deck.md");
        File::create(&deck_file).unwrap();
        let image_file = temp_dir.path().join("image.png");
        File::create(&image_file).unwrap();

        let markdown = "![alt](image.png)";
        let config = MarkdownRendererConfig { port: 1234 };
        let html = markdown_to_html(markdown, &config).unwrap();
        assert_eq!(
            html,
            "<p><img src=\"http://localhost:1234/file/image.png\" alt=\"alt\" /></p>\n"
        );
    }

    #[test]
    fn test_markdown_to_html_inline() {
        // Create a temporary directory structure for testing
        let temp_dir = tempdir().unwrap();
        let deck_file = temp_dir.path().join("deck.md");
        File::create(&deck_file).unwrap();

        let markdown = "This is **bold** text.";
        let config = MarkdownRendererConfig { port: 0 };
        let html = markdown_to_html_inline(markdown, &config).unwrap();
        assert_eq!(html, "This is <strong>bold</strong> text.");
    }

    #[test]
    fn test_markdown_to_html_inline_heading() {
        // Create a temporary directory structure for testing
        let temp_dir = tempdir().unwrap();
        let deck_file = temp_dir.path().join("deck.md");
        File::create(&deck_file).unwrap();

        let markdown = "# Foo";
        let config = MarkdownRendererConfig { port: 0 };
        let html = markdown_to_html_inline(markdown, &config).unwrap();
        assert_eq!(html, "<h1>Foo</h1>\n");
    }

    #[test]
    fn test_markdown_to_html_deck_relative_subdir() {
        // Create a temporary directory structure:
        // collection/
        //   biology/
        //     chapter1.md
        //     images/
        //       cell.png
        let temp_dir = tempdir().unwrap();
        let biology_dir = temp_dir.path().join("biology");
        let images_dir = biology_dir.join("images");
        std::fs::create_dir_all(&images_dir).unwrap();

        let deck_file = biology_dir.join("chapter1.md");
        File::create(&deck_file).unwrap();
        let image_file = images_dir.join("cell.png");
        File::create(&image_file).unwrap();

        let markdown = "![Cell Structure](images/cell.png)";
        let config = MarkdownRendererConfig { port: 8080 };
        let html = markdown_to_html(markdown, &config).unwrap();

        // Path should be resolved relative to deck file, resulting in biology/images/cell.png
        assert!(html.contains("http://localhost:8080/file/biology/images/cell.png"));
    }

    #[test]
    fn test_markdown_to_html_deck_relative_parent() {
        // Create a temporary directory structure:
        // collection/
        //   shared_images/
        //     logo.png
        //   decks/
        //     deck.md
        let temp_dir = tempdir().unwrap();
        let shared_images = temp_dir.path().join("shared_images");
        let decks_dir = temp_dir.path().join("decks");
        std::fs::create_dir_all(&shared_images).unwrap();
        std::fs::create_dir_all(&decks_dir).unwrap();

        let deck_file = decks_dir.join("deck.md");
        File::create(&deck_file).unwrap();
        let image_file = shared_images.join("logo.png");
        File::create(&image_file).unwrap();

        let markdown = "![Logo](../shared_images/logo.png)";
        let config = MarkdownRendererConfig { port: 8080 };
        let html = markdown_to_html(markdown, &config).unwrap();

        // Path with .. should be resolved relative to deck file
        assert!(html.contains("http://localhost:8080/file/shared_images/logo.png"));
    }

    #[test]
    fn test_markdown_to_html_collection_relative() {
        // Create a temporary directory structure:
        // collection/
        //   global/
        //     banner.jpg
        //   deep/
        //     nested/
        //       deck.md
        let temp_dir = tempdir().unwrap();
        let global_dir = temp_dir.path().join("global");
        let nested_dir = temp_dir.path().join("deep/nested");
        std::fs::create_dir_all(&global_dir).unwrap();
        std::fs::create_dir_all(&nested_dir).unwrap();

        let deck_file = nested_dir.join("deck.md");
        File::create(&deck_file).unwrap();
        let image_file = global_dir.join("banner.jpg");
        File::create(&image_file).unwrap();

        let markdown = "![Banner](@/global/banner.jpg)";
        let config = MarkdownRendererConfig { port: 8080 };
        let html = markdown_to_html(markdown, &config).unwrap();

        // Path starting with @/ should be resolved relative to collection root
        assert!(html.contains("http://localhost:8080/file/global/banner.jpg"));
    }

    #[test]
    fn test_markdown_to_html_external_url_unchanged() {
        // External URLs should pass through unchanged
        let temp_dir = tempdir().unwrap();
        let deck_file = temp_dir.path().join("deck.md");
        File::create(&deck_file).unwrap();

        let markdown = "![Remote](https://example.com/image.png)";
        let config = MarkdownRendererConfig { port: 8080 };
        let html = markdown_to_html(markdown, &config).unwrap();

        // External URLs should be passed through as-is
        assert!(html.contains("https://example.com/image.png"));
    }
}
