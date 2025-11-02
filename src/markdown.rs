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

use pulldown_cmark::CowStr;
use pulldown_cmark::Event;
use pulldown_cmark::Parser;
use pulldown_cmark::Tag;
use pulldown_cmark::html::push_html;

use crate::media::resolve::MediaResolver;

const AUDIO_EXTENSIONS: [&str; 3] = ["mp3", "wav", "ogg"];

fn is_audio_file(url: &str) -> bool {
    if let Some(ext) = url.split('.').next_back() {
        AUDIO_EXTENSIONS.contains(&ext)
    } else {
        false
    }
}

pub fn markdown_to_html(
    markdown: &str,
    port: u16,
    collection_root: &Path,
    deck_path: &Path,
) -> String {
    let resolver = MediaResolver {
        root: collection_root.to_path_buf(),
    };
    let parser = Parser::new(markdown);
    let parser = parser.map(|event| match event {
        Event::Start(Tag::Image {
            link_type,
            title,
            dest_url,
            id,
        }) => {
            let url = modify_url(&dest_url, port, &resolver, deck_path);
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
    html_output
}

pub fn markdown_to_html_inline(
    markdown: &str,
    port: u16,
    collection_root: &Path,
    deck_path: &Path,
) -> String {
    let text = markdown_to_html(markdown, port, collection_root, deck_path);
    if text.starts_with("<p>") && text.ends_with("</p>\n") {
        let len = text.len();
        text[3..len - 5].to_string()
    } else {
        text
    }
}

fn modify_url(url: &str, port: u16, resolver: &MediaResolver, deck_path: &Path) -> String {
    if url.contains("://") {
        // Leave external URLs alone.
        url.to_string()
    } else {
        // Normalize the path (deck-relative or collection-relative)
        let normalized = resolver.normalize_path(url, deck_path).unwrap_or_else(|_| {
            // If normalization fails, use the original URL
            // This can happen for invalid paths, which will be caught during validation
            url.to_string()
        });
        format!("http://localhost:{port}/file/{normalized}")
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn test_markdown_to_html() {
        let collection_root = PathBuf::from("/tmp/collection");
        let deck_path = collection_root.join("deck.md");
        let markdown = "![alt](image.png)";
        let html = markdown_to_html(markdown, 1234, &collection_root, &deck_path);
        assert_eq!(
            html,
            "<p><img src=\"http://localhost:1234/file/image.png\" alt=\"alt\" /></p>\n"
        );
    }

    #[test]
    fn test_markdown_to_html_inline() {
        let collection_root = PathBuf::from("/tmp/collection");
        let deck_path = collection_root.join("deck.md");
        let markdown = "This is **bold** text.";
        let html = markdown_to_html_inline(markdown, 0, &collection_root, &deck_path);
        assert_eq!(html, "This is <strong>bold</strong> text.");
    }

    #[test]
    fn test_markdown_to_html_inline_heading() {
        let collection_root = PathBuf::from("/tmp/collection");
        let deck_path = collection_root.join("deck.md");
        let markdown = "# Foo";
        let html = markdown_to_html_inline(markdown, 0, &collection_root, &deck_path);
        assert_eq!(html, "<h1>Foo</h1>\n");
    }

    #[test]
    fn test_external_url_is_unchanged() {
        let collection_root = PathBuf::from("/tmp/collection");
        let deck_path = collection_root.join("deck.md");
        let url = "https://upload.wikimedia.org/wikipedia/commons/6/63/Circe_Invidiosa_-_John_William_Waterhouse.jpg";
        let markdown = format!("![alt]({url})");
        let html = markdown_to_html(&markdown, 1234, &collection_root, &deck_path);
        assert_eq!(html, format!("<p><img src=\"{url}\" alt=\"alt\" /></p>\n"));
    }

    #[test]
    fn test_deck_relative_path() {
        let collection_root = PathBuf::from("/tmp/collection");
        let deck_path = collection_root.join("foo/bar/deck.md");
        let markdown = "![alt](img/test.jpg)";
        let html = markdown_to_html(markdown, 1234, &collection_root, &deck_path);
        assert_eq!(
            html,
            "<p><img src=\"http://localhost:1234/file/foo/bar/img/test.jpg\" alt=\"alt\" /></p>\n"
        );
    }

    #[test]
    fn test_collection_relative_path() {
        let collection_root = PathBuf::from("/tmp/collection");
        let deck_path = collection_root.join("foo/bar/deck.md");
        let markdown = "![alt](@/img/test.jpg)";
        let html = markdown_to_html(markdown, 1234, &collection_root, &deck_path);
        assert_eq!(
            html,
            "<p><img src=\"http://localhost:1234/file/img/test.jpg\" alt=\"alt\" /></p>\n"
        );
    }
}
