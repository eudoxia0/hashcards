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
use maud::PreEscaped;
use maud::html;

use crate::cmd::browse::render::performance_rows;
use crate::cmd::browse::render::render_config;
use crate::cmd::browse::render::render_family_revealed;
use crate::cmd::browse::render::render_history;
use crate::cmd::browse::render::source_rows;
use crate::cmd::browse::shared::BrowseState;
use crate::cmd::browse::shared::error_response;
use crate::cmd::browse::shared::internal_error_response;
use crate::cmd::browse::shared::ok_response;
use crate::cmd::browse::templates::EntryKey;
use crate::cmd::browse::templates::Selection;
use crate::cmd::browse::templates::columns_page;
use crate::error::ErrorReport;
use crate::error::Fallible;
use crate::error::fail;
use crate::markdown::MarkdownRenderConfig;
use crate::markdown::markdown_to_html_inline;
use crate::types::card::Card;
use crate::types::card_hash::CardHash;

pub async fn cloze_family_handler(
    State(state): State<BrowseState>,
    Path(hash): Path<String>,
) -> (StatusCode, Html<String>) {
    let family = match CardHash::from_hex(&hash) {
        Ok(hash) => hash,
        Err(_) => {
            return error_response(
                StatusCode::NOT_FOUND,
                &format!("Invalid family hash '{hash}'."),
            );
        }
    };
    let siblings = match state.families.get(&family) {
        Some(siblings) => siblings,
        None => {
            return error_response(
                StatusCode::NOT_FOUND,
                &format!("No cloze card with family hash '{family}' in this collection."),
            );
        }
    };
    let first = match siblings.first() {
        Some(first) => first,
        None => {
            return internal_error_response(ErrorReport::new("cloze family has no cards."));
        }
    };
    let detail = match render_cloze_detail(&state, family, siblings) {
        Ok(detail) => detail,
        Err(e) => return internal_error_response(e),
    };
    let selection = Selection {
        deck: Some(first.deck_name()),
        entry: Some(EntryKey::Family(family)),
    };
    match columns_page(&state, selection, Some(detail)) {
        Ok(markup) => ok_response(markup),
        Err(e) => internal_error_response(e),
    }
}

fn render_cloze_detail(
    state: &BrowseState,
    family: CardHash,
    siblings: &[Card],
) -> Fallible<Markup> {
    let first = match siblings.first() {
        Some(first) => first,
        None => return fail("cloze family has no cards."),
    };
    let config = render_config(state, first)?;
    Ok(html! {
        h2 { "Text" }
        div .browse-card {
            (render_family_revealed(siblings, &config)?)
        }
        h2 { "Details" }
        div .stats {
            table {
                tbody {
                    (source_rows(state, first, "Cloze")?)
                    tr {
                        td .key { "Family Hash" }
                        td .val { code { (family) } }
                    }
                    tr {
                        td .key { "Clozes" }
                        td .val { (siblings.len()) }
                    }
                }
            }
        }
        h2 { "Clozes" }
        div .card-list {
            @for (index, sibling) in siblings.iter().enumerate() {
                (render_sibling(state, sibling, index, &config)?)
            }
        }
    })
}

/// One section per cloze card in the family: the deleted text, and the card's
/// own review stats and history.
fn render_sibling(
    state: &BrowseState,
    sibling: &Card,
    index: usize,
    config: &MarkdownRenderConfig,
) -> Fallible<Markup> {
    let deleted = match sibling.content().deletion_text()? {
        Some(deleted) => deleted,
        None => return fail("cloze family contains a non-cloze card."),
    };
    let deleted = markdown_to_html_inline(config, &deleted)?;
    Ok(html! {
        div .browse-card .cloze-sibling {
            div .cloze-sibling-header {
                span .badge { "Cloze " (index + 1) }
                span .deletion { (PreEscaped(deleted)) }
            }
            div .stats {
                table {
                    tbody {
                        tr {
                            td .key { "Hash" }
                            td .val { code { (sibling.hash()) } }
                        }
                        (performance_rows(state.performance_of(sibling.hash()), state.today))
                    }
                }
            }
            div .history-container {
                (render_history(state, sibling.hash()))
            }
        }
    })
}
