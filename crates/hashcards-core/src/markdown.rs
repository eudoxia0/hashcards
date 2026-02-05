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
use pulldown_cmark::Options;
use pulldown_cmark::Parser;
use pulldown_cmark::Tag;
use pulldown_cmark::html::push_html;

use crate::error::Fallible;

const AUDIO_EXTENSIONS: [&str; 3] = ["mp3", "wav", "ogg"];

fn is_audio_file(url: &str) -> bool {
    if let Some(ext) = url.split('.').next_back() {
        AUDIO_EXTENSIONS.contains(&ext)
    } else {
        false
    }
}

/// Convert Markdown to HTML.
///
/// The optional `url_rewriter` function can be used to rewrite URLs in images/audio.
pub fn markdown_to_html(
    markdown: &str,
    url_rewriter: Option<&dyn Fn(&str) -> String>,
) -> Fallible<String> {
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
                let url = match url_rewriter {
                    Some(rewriter) => rewriter(&dest_url),
                    None => dest_url.to_string(),
                };
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

pub fn markdown_to_html_inline(
    markdown: &str,
    url_rewriter: Option<&dyn Fn(&str) -> String>,
) -> Fallible<String> {
    let text = markdown_to_html(markdown, url_rewriter)?;
    if text.starts_with("<p>") && text.ends_with("</p>\n") {
        let len = text.len();
        Ok(text[3..len - 5].to_string())
    } else {
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_to_html_basic() -> Fallible<()> {
        let markdown = "This is **bold** text.";
        let html = markdown_to_html(markdown, None)?;
        assert_eq!(html, "<p>This is <strong>bold</strong> text.</p>\n");
        Ok(())
    }

    #[test]
    fn test_markdown_to_html_inline() -> Fallible<()> {
        let markdown = "This is **bold** text.";
        let html = markdown_to_html_inline(markdown, None)?;
        assert_eq!(html, "This is <strong>bold</strong> text.");
        Ok(())
    }

    #[test]
    fn test_markdown_to_html_inline_heading() -> Fallible<()> {
        let markdown = "# Foo";
        let html = markdown_to_html_inline(markdown, None)?;
        assert_eq!(html, "<h1>Foo</h1>\n");
        Ok(())
    }

    #[test]
    fn test_markdown_with_url_rewriter() -> Fallible<()> {
        let markdown = "![alt](image.png)";
        let rewriter = |url: &str| format!("/media/{}", url);
        let html = markdown_to_html(markdown, Some(&rewriter))?;
        assert!(html.contains("/media/image.png"));
        Ok(())
    }

    #[test]
    fn test_markdown_audio_file() -> Fallible<()> {
        let markdown = "![](audio.mp3)";
        let html = markdown_to_html(markdown, None)?;
        assert!(html.contains("<audio"));
        assert!(html.contains("audio.mp3"));
        Ok(())
    }
}
