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
use maud::Markup;
use maud::html;

use crate::cmd::browse::render::performance_rows;
use crate::cmd::browse::render::render_config;
use crate::cmd::browse::render::render_history;
use crate::cmd::browse::render::source_rows;
use crate::cmd::browse::shared::BrowseState;
use crate::cmd::browse::shared::error_response;
use crate::cmd::browse::shared::internal_error_response;
use crate::cmd::browse::shared::ok_response;
use crate::cmd::browse::templates::EntryKey;
use crate::cmd::browse::templates::Selection;
use crate::cmd::browse::templates::columns_page;
use crate::error::Fallible;
use crate::types::card::Card;
use crate::types::card_hash::CardHash;

pub async fn basic_card_handler(
    State(state): State<BrowseState>,
    Path(hash): Path<String>,
) -> (StatusCode, Html<String>) {
    let hash = match CardHash::from_hex(&hash) {
        Ok(hash) => hash,
        Err(_) => {
            return error_response(
                StatusCode::NOT_FOUND,
                &format!("Invalid card hash '{hash}'."),
            );
        }
    };
    let card = state.cards.iter().find(|card| card.hash() == hash);
    let card = match card {
        Some(card) => card,
        None => {
            return error_response(
                StatusCode::NOT_FOUND,
                &format!("No card with hash '{hash}' in this collection."),
            );
        }
    };
    let detail = match render_basic_detail(&state, card) {
        Ok(detail) => detail,
        Err(e) => return internal_error_response(e),
    };
    let selection = Selection {
        deck: Some(card.deck_name()),
        entry: Some(EntryKey::Basic(hash)),
    };
    match columns_page(&state, selection, Some(detail)) {
        Ok(markup) => ok_response(markup),
        Err(e) => internal_error_response(e),
    }
}

fn render_basic_detail(state: &BrowseState, card: &Card) -> Fallible<Markup> {
    let config = render_config(state, card)?;
    Ok(html! {
        .pane-header {
            .pane-title {
                "Card"
            }
            .pane-sub {
                "Basic Card"
            }
        }
        .detail-pane-body {
            h2 { "Front" }
            div .browse-card {
                div .card-content {
                    div .prompt .rich-text {
                        (card.html_front(&config)?)
                    }
                }
            }
            h2 { "Back" }
            div .browse-card {
                div .card-content {
                    div .prompt .rich-text {
                        (card.html_back(&config)?)
                    }
                }
            }
            h2 { "Properties" }
            div .stats {
                table {
                    tbody {
                        (source_rows(state, card, "Basic")?)
                        tr {
                            td .key { "Hash" }
                            td .val { code { (card.hash()) } }
                        }
                        (performance_rows(state.performance_of(card.hash()), state.today))
                    }
                }
            }
            h2 { "History" }
            (render_history(state, card.hash()))
        }
    })
}
