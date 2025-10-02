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

pub fn markdown_to_html(markdown: &str, port: u16) -> String {
    let parser = Parser::new(markdown);
    let parser = parser.map(|event| match event {
        Event::Start(Tag::Image {
            link_type,
            title,
            dest_url,
            id,
        }) => {
            let new_url = modify_url(&dest_url, port);
            Event::Start(Tag::Image {
                link_type,
                title,
                dest_url: CowStr::Boxed(new_url.into_boxed_str()),
                id,
            })
        }
        _ => event,
    });
    let mut html_output = String::new();
    push_html(&mut html_output, parser);
    html_output
}

pub fn markdown_to_html_inline(markdown: &str, port: u16) -> String {
    let text = markdown_to_html(markdown, port);
    if text.starts_with("<p>") && text.ends_with("</p>\n") {
        let len = text.len();
        text[3..len - 5].to_string()
    } else {
        text
    }
}

fn modify_url(url: &str, port: u16) -> String {
    format!("http://localhost:{port}/image/{url}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_to_html() {
        let markdown = "![alt](image.png)";
        let html = markdown_to_html(markdown, 1234);
        assert_eq!(
            html,
            "<p><img src=\"http://localhost:1234/image/image.png\" alt=\"alt\" /></p>\n"
        );
    }

    #[test]
    fn test_markdown_to_html_inline() {
        let markdown = "This is **bold** text.";
        let html = markdown_to_html_inline(markdown, 0);
        assert_eq!(html, "This is <strong>bold</strong> text.");
    }

    #[test]
    fn test_markdown_to_html_inline_heading() {
        let markdown = "# Foo";
        let html = markdown_to_html_inline(markdown, 0);
        assert_eq!(html, "<h1>Foo</h1>\n");
    }
}
