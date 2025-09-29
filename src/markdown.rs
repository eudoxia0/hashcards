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

pub fn markdown_to_html(markdown: &str) -> String {
    let parser = Parser::new(markdown);
    let parser = parser.map(|event| match event {
        Event::Start(Tag::Image {
            link_type,
            title,
            dest_url,
            id,
        }) => {
            let new_url = modify_url(&dest_url);
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

fn modify_url(url: &str) -> String {
    format!("http://localhost:8000/image/{}", url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_to_html() {
        let markdown = "![alt](image.png)";
        let html = markdown_to_html(markdown);
        assert_eq!(
            html,
            "<p><img src=\"http://localhost:8000/image/image.png\" alt=\"alt\" /></p>\n"
        );
    }
}
