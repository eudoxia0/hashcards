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

use serde::Serialize;

use crate::collection::Collection;
use crate::error::Fallible;
use crate::fsrs::Difficulty;
use crate::fsrs::Grade;
use crate::fsrs::Stability;
use crate::fsrs::T;
use crate::types::card::CardContent;
use crate::types::card_hash::CardHash;
use crate::types::date::Date;
use crate::types::timestamp::Timestamp;

pub fn export_collection(directory: Option<String>) -> Fallible<()> {
    let coll: Collection = Collection::new(directory)?;
    let export: Export = get_export(coll)?;
    let json: String = serde_json::to_string_pretty(&export)?;
    println!("{json}");
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Export {
    cards: Vec<CardExport>,
    sessions: Vec<SessionExport>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CardExport {
    hash: CardHash,
    family_hash: Option<CardHash>,
    deck_name: String,
    location: LocationExport,
    content: CardContentExport,
    performance: Option<PerformanceExport>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LocationExport {
    file_path: String,
    line_start: usize,
    line_end: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum CardContentExport {
    Basic {
        question: String,
        answer: String,
    },
    Cloze {
        text: String,
        start: usize,
        end: usize,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PerformanceExport {
    last_reviewed_at: Timestamp,
    stability: Stability,
    difficulty: Difficulty,
    interval_raw: T,
    interval_days: usize,
    due_date: Date,
    review_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionExport {
    started_at: Timestamp,
    ended_at: Timestamp,
    reviews: Vec<ReviewExport>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewExport {
    hash: CardHash,
    reviewed_at: Timestamp,
    grade: Grade,
    stability: Stability,
    difficulty: Difficulty,
    interval_raw: T,
    interval_days: usize,
    due_date: Date,
}

fn get_export(coll: Collection) -> Fallible<Export> {
    let cards: Vec<CardExport> = get_card_export(&coll)?;
    let sessions: Vec<SessionExport> = get_session_export(&coll)?;
    Ok(Export { cards, sessions })
}

fn get_card_export(coll: &Collection) -> Fallible<Vec<CardExport>> {
    let mut cards: Vec<CardExport> = Vec::new();
    for card in coll.cards.iter() {
        let ce = CardExport {
            hash: card.hash(),
            family_hash: card.family_hash(),
            deck_name: card.deck_name().to_owned(),
            location: LocationExport {
                file_path: card.file_path().clone().display().to_string(),
                line_start: card.range().0,
                line_end: card.range().1,
            },
            content: match card.content() {
                CardContent::Basic { question, answer } => CardContentExport::Basic {
                    question: question.clone(),
                    answer: answer.clone(),
                },
                CardContent::Cloze { text, start, end } => CardContentExport::Cloze {
                    text: text.clone(),
                    start: *start,
                    end: *end,
                },
            },
            performance: todo!(),
        };
        cards.push(ce);
    }
    Ok(cards)
}

fn get_session_export(coll: &Collection) -> Fallible<Vec<SessionExport>> {
    todo!()
}
