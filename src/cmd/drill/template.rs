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

use maud::DOCTYPE;
use maud::Markup;
use maud::html;

use crate::cmd::drill::katex::KATEX_AUTO_RENDER_JS_URL;
use crate::cmd::drill::katex::KATEX_CSS_URL;
use crate::cmd::drill::katex::KATEX_JS_URL;

const HIGHLIGHT_JS_URL: &str = "https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/highlight.min.js";
const HIGHLIGHT_CSS_URL: &str = "https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/styles/github.min.css";

pub fn page_template(body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "hashcards" }
                link rel="stylesheet" href=(KATEX_CSS_URL);
                link rel="stylesheet" href=(HIGHLIGHT_CSS_URL);
                script defer src=(KATEX_JS_URL) {};
                script defer src=(KATEX_AUTO_RENDER_JS_URL) {};
                script defer src=(HIGHLIGHT_JS_URL) {};
                link rel="stylesheet" href="/style.css";
            }
            body {
                (body)
                script src="/script.js" {};
            }
        }
    }
}
