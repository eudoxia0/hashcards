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

use maud::Markup;
use maud::html;

use crate::cmd::browse::state::BrowseState;
use crate::error::Fallible;
use crate::error::fail;
use crate::markdown::MarkdownRenderConfig;
use crate::media::resolve::MediaResolverBuilder;
use crate::types::card::Card;
use crate::types::card::CardContent;
use crate::types::card::html_cloze_family;

/// Build the Markdown render configuration for the given card, for the
/// detail pane. Media paths are resolved relative to the file the card was
/// parsed from.
pub fn render_config(state: &BrowseState, card: &Card) -> Fallible<MarkdownRenderConfig> {
    build_config(state, card, true)
}

/// Build the Markdown render configuration for a card-list label: images and
/// audio are stripped.
pub fn label_config(state: &BrowseState, card: &Card) -> Fallible<MarkdownRenderConfig> {
    build_config(state, card, false)
}

fn build_config(
    state: &BrowseState,
    card: &Card,
    render_media: bool,
) -> Fallible<MarkdownRenderConfig> {
    let coll_path = state.directory.clone();
    let deck_path = card.relative_file_path(&coll_path)?;
    Ok(MarkdownRenderConfig {
        resolver: MediaResolverBuilder::new()
            .with_collection_path(coll_path)?
            .with_deck_path(deck_path)?
            .build()?,
        resource_hostname: state.resource_hostname.clone(),
        port: state.port,
        autoplay_audio: false,
        render_media,
    })
}

/// Render a cloze family's shared text with every deletion revealed. The
/// siblings must be sorted by deletion position.
pub fn render_family_revealed(
    siblings: &[Card],
    config: &MarkdownRenderConfig,
) -> Fallible<Markup> {
    let text = family_text(siblings)?;
    let deletions = family_deletions(siblings);
    let html = html_cloze_family(config, text, &deletions)?;
    Ok(html! {
        div .card-content {
            div .prompt .rich-text {
                (html)
            }
        }
    })
}

/// The text shared by the cards of a cloze family.
fn family_text(siblings: &[Card]) -> Fallible<&str> {
    match siblings.first().map(|card| card.content()) {
        Some(CardContent::Cloze { text, .. }) => Ok(text),
        _ => fail("cloze family has no cloze cards."),
    }
}

/// The deletion ranges of the cards of a cloze family.
fn family_deletions(siblings: &[Card]) -> Vec<(usize, usize)> {
    siblings
        .iter()
        .filter_map(|card| match card.content() {
            CardContent::Cloze { start, end, .. } => Some((*start, *end)),
            CardContent::Basic { .. } => None,
        })
        .collect()
}
