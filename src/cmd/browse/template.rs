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

use axum::http::StatusCode;
use axum::response::Html;
use maud::DOCTYPE;
use maud::Markup;
use maud::html;

use crate::cmd::drill::highlight::HIGHLIGHT_CSS_URL;
use crate::cmd::drill::highlight::HIGHLIGHT_JS_URL;
use crate::cmd::drill::katex::KATEX_CSS_URL;
use crate::cmd::drill::katex::KATEX_JS_URL;
use crate::cmd::drill::katex::KATEX_MHCHEM_JS_URL;
use crate::error::ErrorReport;

/// Page template.
pub fn page_template(title: &str, body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) }
                link rel="stylesheet" href=(KATEX_CSS_URL);
                link rel="stylesheet" href=(HIGHLIGHT_CSS_URL);
                script defer src=(KATEX_JS_URL) {};
                script defer src=(KATEX_MHCHEM_JS_URL) {};
                script defer src=(HIGHLIGHT_JS_URL) {};
                link rel="stylesheet" href="/common.css";
                link rel="stylesheet" href="/browse.css";
                // See `script.js`. To prevent a flash of un-rendered TeX and
                // un-highlighted source code, we make the card content
                // invisible until the math rendering and syntax highlighting
                // are done. If the browser has JavaScript disabled, however,
                // we keep the content visible.
                style { ".card-content { opacity: 0; }" }
                noscript { style { ".card-content { opacity: 1; }" }}
            }
            body {
                (body)
                script src="/script.js" {};
            }
        }
    }
}

/// A successful HTML response.
pub fn ok_response(markup: Markup) -> (StatusCode, Html<String>) {
    (StatusCode::OK, Html(markup.into_string()))
}

/// An HTML error page with the given status code and message.
pub fn error_response(status: StatusCode, message: &str) -> (StatusCode, Html<String>) {
    let markup = page_template(
        "Error — hashcards",
        html! {
            main .error-page {
                h1 { "Error" }
                p { (message) }
            }
        },
    );
    (status, Html(markup.into_string()))
}

/// An internal server error page from an error report.
pub fn internal_error_response(e: ErrorReport) -> (StatusCode, Html<String>) {
    error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string())
}
