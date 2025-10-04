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

use std::fs::write;

use serde::Serialize;

use crate::collection::Collection;
use crate::db::ReviewRow;
use crate::db::SessionRow;
use crate::error::Fallible;
use crate::fsrs::Difficulty;
use crate::fsrs::Grade;
use crate::fsrs::Interval;
use crate::fsrs::Stability;
use crate::types::aliases::DeckName;
use crate::types::card::CardContent;
use crate::types::card_hash::CardHash;
use crate::types::date::Date;
use crate::types::performance::Performance;
use crate::types::performance::ReviewedPerformance;
use crate::types::timestamp::Timestamp;

pub fn export_collection(directory: Option<String>, output: Option<String>) -> Fallible<()> {
    let coll: Collection = Collection::new(directory)?;
    let export: Export = get_export(coll)?;
    let json = serde_json::to_string_pretty(&export)?;
    match output {
        Some(path) => write(path, json)?,
        None => println!("{}", json),
    }
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
    deck_name: DeckName,
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
    interval_raw: Interval,
    interval_days: usize,
    due_date: Date,
    review_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionExport {
    session_id: i64,
    started_at: Timestamp,
    ended_at: Timestamp,
    reviews: Vec<ReviewExport>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewExport {
    review_id: i64,
    hash: CardHash,
    reviewed_at: Timestamp,
    grade: Grade,
    stability: Stability,
    difficulty: Difficulty,
    interval_raw: Interval,
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
        let p = coll.db.get_card_performance_opt(card.hash())?;
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
            performance: export_performance(p),
        };
        cards.push(ce);
    }
    Ok(cards)
}

fn export_performance(p: Option<Performance>) -> Option<PerformanceExport> {
    match p {
        Some(p) => match p {
            Performance::New => None,
            Performance::Reviewed(ReviewedPerformance {
                last_reviewed_at,
                stability,
                difficulty,
                interval_raw,
                interval_days,
                due_date,
                review_count,
            }) => Some(PerformanceExport {
                last_reviewed_at,
                stability,
                difficulty,
                interval_raw,
                interval_days,
                due_date,
                review_count,
            }),
        },
        None => None,
    }
}

fn get_session_export(coll: &Collection) -> Fallible<Vec<SessionExport>> {
    let sessions = coll.db.get_all_sessions()?;
    let mut session_exports: Vec<SessionExport> = Vec::new();
    for session in sessions.into_iter() {
        let session_export = export_session(coll, session)?;
        session_exports.push(session_export);
    }
    Ok(session_exports)
}

fn export_session(coll: &Collection, session: SessionRow) -> Fallible<SessionExport> {
    let reviews = coll.db.get_reviews_for_session(session.session_id)?;
    let mut review_exports: Vec<ReviewExport> = Vec::new();
    for review in reviews.into_iter() {
        let review_export = export_review(review);
        review_exports.push(review_export)
    }
    Ok(SessionExport {
        session_id: session.session_id,
        started_at: session.started_at,
        ended_at: session.ended_at,
        reviews: review_exports,
    })
}

fn export_review(review: ReviewRow) -> ReviewExport {
    ReviewExport {
        review_id: review.review_id,
        hash: review.data.card_hash,
        reviewed_at: review.data.reviewed_at,
        grade: review.data.grade,
        stability: review.data.stability,
        difficulty: review.data.difficulty,
        interval_raw: review.data.interval_raw,
        interval_days: review.data.interval_days,
        due_date: review.data.due_date,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::db::ReviewRecord;
    use crate::helper::create_tmp_copy_of_test_directory;
    use crate::helper::create_tmp_directory;
    use crate::parser::parse_deck;

    #[test]
    fn test_full_export() -> Fallible<()> {
        let dir = create_tmp_copy_of_test_directory()?;
        let mut coll = Collection::new(Some(dir.clone()))?;
        let deck = parse_deck(&PathBuf::from(dir.clone()))?;
        let now = Timestamp::now();
        let mut reviews = Vec::new();
        for card in deck {
            coll.db.insert_card(card.hash(), now)?;
            let performance = Performance::Reviewed(ReviewedPerformance {
                last_reviewed_at: now,
                stability: 1.0,
                difficulty: 3.0,
                interval_raw: 1.0,
                interval_days: 1,
                due_date: now.date(),
                review_count: 1,
            });
            coll.db.update_card_performance(card.hash(), performance)?;
            let review = ReviewRecord {
                card_hash: card.hash(),
                reviewed_at: now,
                grade: Grade::Easy,
                stability: 1.0,
                difficulty: 3.0,
                interval_raw: 1.0,
                interval_days: 1,
                due_date: now.date(),
            };
            reviews.push(review);
        }
        coll.db.save_session(now, now, reviews)?;
        // Export.
        export_collection(Some(dir.clone()), None)?;
        let tmp = create_tmp_directory()?;
        let output = tmp.join("export.json").display().to_string();
        export_collection(Some(dir), Some(output))?;
        Ok(())
    }
}
