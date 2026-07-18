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

use percent_encoding::NON_ALPHANUMERIC;
use percent_encoding::utf8_percent_encode;

use crate::types::aliases::DeckName;
use crate::types::card_hash::CardHash;

/// Generate the URl of a deck from its name.
pub fn deck_url(name: &DeckName) -> String {
    format!("/deck/{}", utf8_percent_encode(name, NON_ALPHANUMERIC))
}

/// The URL of a basic card.
pub fn basic_card_url(hash: CardHash) -> String {
    format!("/card/basic/{}", hash.to_hex())
}

/// The URL of a cloze card's family.
pub fn cloze_family_url(family: CardHash) -> String {
    format!("/card/cloze/{}", family.to_hex())
}
