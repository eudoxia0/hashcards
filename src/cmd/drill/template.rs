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

pub fn page_template(body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "hashcards" }
                link rel="stylesheet" href="/katex/katex.css";
                script defer src="/katex/katex.js" {};
                script defer src="/katex/katex-auto-render.js" {};
                link rel="stylesheet" href="/style.css";
            }
            body {
                (body)
                script src="/script.js" {};
            }
        }
    }
}
