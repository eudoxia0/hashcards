// Copyright 2025–2026 Fernando Borretti
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

use std::ops::Range;

use pulldown_cmark::CowStr;
use pulldown_cmark::Event;
use pulldown_cmark::Options;
use pulldown_cmark::Parser;
use pulldown_cmark::Tag;
use pulldown_cmark::TagEnd;
use pulldown_cmark::html::push_html;

use crate::error::ErrorReport;
use crate::error::Fallible;
use crate::media::resolve::MediaResolver;

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
    /// The hostname where linked media resources are exposed.
    pub resource_hostname: String,
    /// The port where the server is exposed.
    pub port: u16,
}

pub fn markdown_to_html(config: &MarkdownRenderConfig, markdown: &str) -> Fallible<String> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_MATH);
    let rewritten = rewrite_latex_delimiters(markdown);
    let parser = Parser::new_ext(&rewritten, options);
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
                            r#"<audio autoplay controls src="{}" title="{}"></audio>"#,
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

/// Rewrites LaTeX-style math delimiters into the pulldown-cmark/KaTeX dollar
/// form: `\(...\)` → `$...$` and `\[...\]` → `$$...$$`. Content inside fenced
/// code blocks and inline code spans is left untouched.
fn rewrite_latex_delimiters(markdown: &str) -> String {
    let protected = protected_code_ranges(markdown);
    let bytes = markdown.as_bytes();
    let mut out = String::with_capacity(bytes.len());
    let mut copied = 0;
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] != b'\\' || is_in_ranges(i, &protected) {
            i += 1;
            continue;
        }
        let next = bytes[i + 1];
        if next == b'\\' {
            // Escaped backslash; skip both bytes so we don't misread `\\(` as `\(`.
            i += 2;
            continue;
        }
        let (close_char, dollars) = match next {
            b'(' => (b')', "$"),
            b'[' => (b']', "$$"),
            _ => {
                i += 1;
                continue;
            }
        };
        if let Some(close_pos) = find_close(bytes, i + 2, close_char, &protected) {
            out.push_str(&markdown[copied..i]);
            out.push_str(dollars);
            out.push_str(&markdown[i + 2..close_pos]);
            out.push_str(dollars);
            i = close_pos + 2;
            copied = i;
        } else {
            i += 1;
        }
    }
    out.push_str(&markdown[copied..]);
    out
}

fn find_close(
    bytes: &[u8],
    from: usize,
    close_char: u8,
    protected: &[Range<usize>],
) -> Option<usize> {
    let mut i = from;
    while i + 1 < bytes.len() {
        if bytes[i] == b'\\' && !is_in_ranges(i, protected) {
            if bytes[i + 1] == b'\\' {
                i += 2;
                continue;
            }
            if bytes[i + 1] == close_char {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

fn protected_code_ranges(markdown: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut code_block_start: Option<usize> = None;
    let parser = Parser::new_ext(markdown, Options::empty()).into_offset_iter();
    for (event, range) in parser {
        match event {
            Event::Start(Tag::CodeBlock(_)) => {
                code_block_start = Some(range.start);
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some(start) = code_block_start.take() {
                    ranges.push(start..range.end);
                }
            }
            Event::Code(_) => {
                ranges.push(range);
            }
            _ => {}
        }
    }
    ranges
}

fn is_in_ranges(pos: usize, ranges: &[Range<usize>]) -> bool {
    ranges.iter().any(|r| pos >= r.start && pos < r.end)
}

fn modify_url(url: &str, config: &MarkdownRenderConfig) -> Fallible<String> {
    let port = config.port;
    let hostname = &config.resource_hostname;
    let path: String = config
        .resolver
        .resolve(url)
        .map_err(|err| {
            ErrorReport::new(format!("Failed to resolve media path '{}': {}", url, err))
        })?
        .display()
        .to_string();
    Ok(format!("http://{hostname}:{port}/file/{path}"))
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
            resource_hostname: "localhost".to_string(),
            port: 1234,
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
    fn test_markdown_to_html_custom_resource_hostname() -> Fallible<()> {
        let markdown = "![alt](@/image.png)";
        let mut config = make_test_config()?;
        config.resource_hostname = "host.containers.internal".to_string();
        let html = markdown_to_html(&config, markdown)?;
        assert_eq!(
            html,
            "<p><img src=\"http://host.containers.internal:1234/file/image.png\" alt=\"alt\" /></p>\n"
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
