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

use std::collections::HashSet;
use std::slice::from_ref;

use maud::Markup;
use maud::PreEscaped;
use maud::html;

use crate::cmd::browse::render::label_config;
use crate::cmd::browse::shared::BrowseState;
use crate::error::Fallible;
use crate::error::fail;
use crate::markdown::markdown_to_html_inline;
use crate::types::card::Card;
use crate::types::card::CardContent;
use crate::types::card_hash::CardHash;
use crate::types::date::Date;
use crate::types::performance::Performance;

/// A card as written in a deck file: either a basic card, or a family of
/// cloze cards sharing the same text.
pub enum DeckEntry<'a> {
    Basic(&'a Card),
    ClozeFamily(CardHash, &'a [Card]),
}

/// Identifies the entry selected in the card list.
#[derive(Clone, Copy, PartialEq)]
pub enum EntryKey {
    Basic(CardHash),
    Family(CardHash),
}

/// The entries of a deck, in case-insensitive alphabetical order of their
/// text.
pub fn deck_entries<'a>(state: &'a BrowseState, deck: &str) -> Vec<DeckEntry<'a>> {
    let mut cards: Vec<&Card> = state
        .cards
        .iter()
        .filter(|card| card.deck_name() == deck)
        .collect();
    // Sort by source position first, so that grouping is deterministic.
    cards.sort_by_key(|card| (card.file_path().clone(), card.range().0));
    // Group cloze siblings into one entry per family.
    let mut entries: Vec<DeckEntry> = Vec::new();
    let mut seen_families: HashSet<CardHash> = HashSet::new();
    for card in &cards {
        match card.family_hash() {
            None => entries.push(DeckEntry::Basic(*card)),
            Some(family) => {
                if seen_families.insert(family) {
                    if let Some(siblings) = state.families.get(&family) {
                        entries.push(DeckEntry::ClozeFamily(family, siblings));
                    }
                }
            }
        }
    }
    entries.sort_by_key(|entry| entry_label(entry).to_lowercase());
    entries
}

/// The text identifying an entry in the card list: the question of a basic
/// card, or the shared text of a cloze family.
pub fn entry_label<'a>(entry: &DeckEntry<'a>) -> &'a str {
    match entry {
        DeckEntry::Basic(card) => match card.content() {
            CardContent::Basic { question, .. } => question,
            CardContent::Cloze { text, .. } => text,
        },
        DeckEntry::ClozeFamily(_, siblings) => match siblings.first().map(|card| card.content()) {
            Some(CardContent::Cloze { text, .. }) => text,
            _ => "",
        },
    }
}

/// The label of an entry as shown in the card list: the first line of its
/// text, rendered as inline Markdown with media stripped (an image's alt
/// text, if any, is kept). Falls back to a placeholder for entries whose
/// first line is nothing but media.
pub fn entry_label_html(state: &BrowseState, entry: &DeckEntry) -> Fallible<Markup> {
    let card = entry_card(entry)?;
    let config = label_config(state, card)?;
    let first_line = entry_label(entry).lines().next().unwrap_or("");
    let html = markdown_to_html_inline(&config, first_line)?;
    if html.trim().is_empty() {
        Ok(html! {
            span .untitled { "(untitled)" }
        })
    } else {
        Ok(html! {
            (PreEscaped(html))
        })
    }
}

/// The first card of an entry.
fn entry_card<'a>(entry: &DeckEntry<'a>) -> Fallible<&'a Card> {
    match entry {
        DeckEntry::Basic(card) => Ok(card),
        DeckEntry::ClozeFamily(_, siblings) => match siblings.first() {
            Some(first) => Ok(first),
            None => fail("cloze family has no cards."),
        },
    }
}

/// The URL of an entry's detail page.
pub fn entry_url(entry: &DeckEntry) -> String {
    let segment: &str = match entry {
        DeckEntry::Basic(_) => "basic",
        DeckEntry::ClozeFamily(_, _) => "cloze",
    };
    let hash: String = match entry {
        DeckEntry::Basic(card) => card.hash().to_hex(),
        DeckEntry::ClozeFamily(family_hash, _) => family_hash.to_hex(),
    };
    format!("/card/{segment}/{hash}")
}

/// The key identifying an entry, for selection highlighting.
pub fn entry_key(entry: &DeckEntry) -> EntryKey {
    match entry {
        DeckEntry::Basic(card) => EntryKey::Basic(card.hash()),
        DeckEntry::ClozeFamily(family, _) => EntryKey::Family(*family),
    }
}

/// The entry's type, as shown in the card list: "Basic", "Cloze", or
/// "Cloze × n".
pub fn entry_type_label(entry: &DeckEntry) -> String {
    match entry {
        DeckEntry::Basic(_) => "Basic".to_string(),
        DeckEntry::ClozeFamily(_, siblings) => {
            if siblings.len() == 1 {
                "Cloze".to_string()
            } else {
                format!("Cloze × {}", siblings.len())
            }
        }
    }
}

/// Summarize the schedule of an entry: "New" if any of its cards has never
/// been reviewed (new cards are due immediately), otherwise the earliest due
/// date.
pub fn entry_schedule(state: &BrowseState, entry: &DeckEntry) -> String {
    let cards: &[Card] = match entry {
        DeckEntry::Basic(card) => from_ref(*card),
        DeckEntry::ClozeFamily(_, siblings) => siblings,
    };
    let mut earliest: Option<Date> = None;
    for card in cards {
        match state.performance_of(card.hash()) {
            Performance::New => return "New".to_string(),
            Performance::Reviewed(rp) => {
                earliest = match earliest {
                    Some(date) => Some(date.min(rp.due_date)),
                    None => Some(rp.due_date),
                };
            }
        }
    }
    match earliest {
        Some(date) => format!("Due {date}"),
        None => "New".to_string(),
    }
}
