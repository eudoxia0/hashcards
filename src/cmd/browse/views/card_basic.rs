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

use axum::extract::Path;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use axum::response::IntoResponse;
use maud::html;

use crate::cmd::browse::server::BrowseState;
use crate::cmd::browse::templates::page_template;

pub async fn card_basic_handler(
    State(state): State<BrowseState>,
    Path(card_hash): Path<String>,
) -> impl IntoResponse {
    let body = html! {
        h1 {
            "Hello, world!"
        }
    };
    let html = page_template("hashcards", body);
    (StatusCode::OK, Html(html.into_string()))
}
