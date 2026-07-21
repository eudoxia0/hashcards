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

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;

use crate::cmd::browse::shared::BrowseState;
use crate::cmd::browse::shared::internal_error_response;
use crate::cmd::browse::shared::ok_response;
use crate::cmd::browse::templates::Selection;
use crate::cmd::browse::templates::columns_page;

pub async fn index_handler(State(state): State<BrowseState>) -> (StatusCode, Html<String>) {
    let selection = Selection {
        deck: None,
        entry: None,
    };
    match columns_page(&state, selection, None) {
        Ok(markup) => ok_response(markup),
        Err(e) => internal_error_response(e),
    }
}
