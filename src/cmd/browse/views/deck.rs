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

use crate::cmd::browse::shared::BrowseState;
use crate::cmd::browse::shared::error_response;
use crate::cmd::browse::shared::internal_error_response;
use crate::cmd::browse::shared::ok_response;
use crate::cmd::browse::templates::Selection;
use crate::cmd::browse::templates::columns_page;

pub async fn deck_handler(
    State(state): State<BrowseState>,
    Path(name): Path<String>,
) -> (StatusCode, Html<String>) {
    let exists = state.cards.iter().any(|card| card.deck_name() == &name);
    if !exists {
        return error_response(
            StatusCode::NOT_FOUND,
            &format!("No deck named '{name}' in this collection."),
        );
    }
    let selection = Selection {
        deck: Some(&name),
        entry: None,
    };
    match columns_page(&state, selection, None) {
        Ok(markup) => ok_response(markup),
        Err(e) => internal_error_response(e),
    }
}
