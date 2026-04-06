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

use percent_encoding::AsciiSet;
use percent_encoding::CONTROLS;
use percent_encoding::utf8_percent_encode;
use pulldown_cmark::CowStr;
use pulldown_cmark::Event;
use pulldown_cmark::Options;
use pulldown_cmark::Parser;
use pulldown_cmark::Tag;
use pulldown_cmark::html::push_html;

use crate::error::ErrorReport;
use crate::error::Fallible;
use crate::media::resolve::MediaResolver;

/// Characters that must be percent-encoded in a URL path segment.
/// Encodes control characters plus space, #, ?, %, and / (RFC 3986).
const PATH_SEGMENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'#')
    .add(b'?')
    .add(b'%')
    .add(b'/');

const AUDIO_EXTENSIONS: [&str; 4] = ["mp3", "wav", "ogg", "m4a"];

fn is_audio_file(url: &str) -> bool {
    if let Some(ext) = url.split('.').next_back() {
        AUDIO_EXTENSIONS.contains(&ext)
    } else {
        false
    }
}

/// Configuration for Markdown rendering.
pub struct MarkdownRenderConfig {
    /// A media resolver.
    pub resolver: MediaResolver,
    /// URL prefix for file serving (e.g. "http://localhost:8000/file" or "/collection/slug/file").
    pub file_url_prefix: String,
}

pub fn markdown_to_html(config: &MarkdownRenderConfig, markdown: &str) -> Fallible<String> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_MATH);
    let parser = Parser::new_ext(markdown, options);
    let events: Vec<Event<'_>> = parser
        .map(|event| match event {
            Event::Start(Tag::Image {
                link_type,
                title,
                dest_url,
                id,
            }) => {
                let url = modify_url(&dest_url, config)?;
                // Does the URL point to an audio file?
                let ev = if is_audio_file(&url) {
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
                };
                Ok(ev)
            }
            _ => Ok(event),
        })
        .collect::<Fallible<Vec<_>>>()?;
    let mut html_output: String = String::new();
    push_html(&mut html_output, events.into_iter());
    Ok(html_output)
}

pub fn markdown_to_html_inline(config: &MarkdownRenderConfig, markdown: &str) -> Fallible<String> {
    let text = markdown_to_html(config, markdown)?;
    if text.starts_with("<p>") && text.ends_with("</p>\n") {
        let len = text.len();
        Ok(text[3..len - 5].to_string())
    } else {
        Ok(text)
    }
}

fn modify_url(url: &str, config: &MarkdownRenderConfig) -> Fallible<String> {
    use crate::media::resolve::ResolveError;
    let prefix = config.file_url_prefix.trim_end_matches('/');
    let resolved = match config.resolver.resolve(url) {
        Ok(p) => p,
        // External URLs (e.g. HedgeDoc image uploads) are passed through as-is;
        // the browser fetches them directly.
        Err(ResolveError::ExternalUrl) => return Ok(url.to_string()),
        Err(err) => {
            return Err(ErrorReport::new(format!(
                "Failed to resolve media path '{}': {}",
                url, err
            )));
        }
    };
    // Build a percent-encoded, forward-slash-separated URL path.
    let path: String = resolved
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .map(|seg| utf8_percent_encode(&seg, PATH_SEGMENT).to_string())
        .collect::<Vec<_>>()
        .join("/");
    Ok(format!("{prefix}/{path}"))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::helper::create_tmp_directory;
    use crate::media::resolve::MediaResolverBuilder;

    fn make_test_config() -> Fallible<MarkdownRenderConfig> {
        let coll_path: PathBuf = create_tmp_directory()?;
        let abs_deck_path: PathBuf = coll_path.join("deck.md");
        let image_path: PathBuf = coll_path.join("image.png");
        std::fs::write(&abs_deck_path, "")?;
        std::fs::write(&image_path, "")?;
        let config = MarkdownRenderConfig {
            resolver: MediaResolverBuilder::new()
                .with_collection_path(coll_path)?
                .with_deck_path(PathBuf::from("deck.md"))?
                .build()?,
            file_url_prefix: "http://localhost:1234/file".to_string(),
        };
        Ok(config)
    }

    #[test]
    fn test_markdown_to_html() -> Fallible<()> {
        let markdown = "![alt](@/image.png)";
        let config = make_test_config()?;
        let html = markdown_to_html(&config, markdown)?;
        assert_eq!(
            html,
            "<p><img src=\"http://localhost:1234/file/image.png\" alt=\"alt\" /></p>\n"
        );
        Ok(())
    }

    #[test]
    fn test_markdown_to_html_inline() -> Fallible<()> {
        let markdown = "This is **bold** text.";
        let config = make_test_config()?;
        let html = markdown_to_html_inline(&config, markdown)?;
        assert_eq!(html, "This is <strong>bold</strong> text.");
        Ok(())
    }

    #[test]
    fn test_markdown_to_html_inline_heading() -> Fallible<()> {
        let markdown = "# Foo";
        let config = make_test_config()?;
        let html = markdown_to_html_inline(&config, markdown)?;
        assert_eq!(html, "<h1>Foo</h1>\n");
        Ok(())
    }
}
