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

use axum::http::HeaderName;
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use maud::DOCTYPE;
use maud::Markup;
use maud::html;

use crate::cmd::drill::hljs::HLJS_CSS_URL;
use crate::cmd::drill::hljs::HLJS_JS_URL;
use crate::cmd::drill::katex::KATEX_CSS_URL;
use crate::cmd::drill::katex::KATEX_JS_URL;
use crate::cmd::drill::katex::KATEX_MHCHEM_JS_URL;

const MANIFEST_JSON: &str = r##"{
  "name": "hashcards",
  "short_name": "hashcards",
  "display": "standalone",
  "start_url": "/",
  "theme_color": "#000000",
  "background_color": "#ffffff",
  "icons": [
    { "src": "/icons/icon-192.png", "sizes": "192x192", "type": "image/png" },
    { "src": "/icons/icon-512.png", "sizes": "512x512", "type": "image/png", "purpose": "any maskable" }
  ]
}"##;

const ICON_192: &[u8] = include_bytes!("icon-192.png");
const ICON_512: &[u8] = include_bytes!("icon-512.png");

pub async fn manifest_handler() -> (StatusCode, [(HeaderName, &'static str); 1], &'static str) {
    (
        StatusCode::OK,
        [(CONTENT_TYPE, "application/manifest+json")],
        MANIFEST_JSON,
    )
}

pub async fn icon_192_handler() -> (StatusCode, [(HeaderName, &'static str); 1], &'static [u8]) {
    (StatusCode::OK, [(CONTENT_TYPE, "image/png")], ICON_192)
}

pub async fn icon_512_handler() -> (StatusCode, [(HeaderName, &'static str); 1], &'static [u8]) {
    (StatusCode::OK, [(CONTENT_TYPE, "image/png")], ICON_512)
}

pub fn page_template(body: Markup) -> Markup {
    page_template_with_script("/script.js", body)
}

pub fn page_template_with_script(script_url: &str, body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "hashcards" }
                link rel="manifest" href="/manifest.json";
                link rel="stylesheet" href=(KATEX_CSS_URL);
                link rel="stylesheet" href=(HLJS_CSS_URL);
                script defer src=(KATEX_JS_URL) {};
                script defer src=(KATEX_MHCHEM_JS_URL) {};
                script defer src=(HLJS_JS_URL) {};
                link rel="stylesheet" href="/style.css";
                style { ".card-content { opacity: 0; }" }
                noscript { style { ".card-content { opacity: 1; }" }}
            }
            body {
                (body)
                script src=(script_url) {};
            }
        }
    }
}
