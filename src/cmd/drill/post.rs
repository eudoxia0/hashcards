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

use axum::Form;
use axum::extract::State;
use axum::response::Redirect;
use serde::Deserialize;

use crate::cmd::drill::state::MutableState;
use crate::cmd::drill::state::Review;
use crate::cmd::drill::state::ServerState;
use crate::db::ReviewRecord;
use crate::error::Fallible;
use crate::fsrs::Grade;
use crate::types::card::Card;
use crate::types::card_hash::CardHash;
use crate::types::performance::Performance;
use crate::types::performance::ReviewedPerformance;
use crate::types::performance::update_performance;
use crate::types::timestamp::Timestamp;

#[derive(Debug, Deserialize)]
enum Action {
    Reveal,
    Undo,
    End,
    Forgot,
    Hard,
    Good,
    Easy,
}

impl Action {
    pub fn grade(&self) -> Grade {
        match self {
            Action::Forgot => Grade::Forgot,
            Action::Hard => Grade::Hard,
            Action::Good => Grade::Good,
            Action::Easy => Grade::Easy,
            _ => panic!("Action does not correspond to a grade"),
        }
    }
}

#[derive(Deserialize)]
pub struct FormData {
    action: Action,
}

pub async fn post_handler(
    State(state): State<ServerState>,
    Form(form): Form<FormData>,
) -> Redirect {
    match action_handler(state, form.action).await {
        Ok(_) => {}
        Err(e) => {
            log::error!("error: {e}");
        }
    }
    Redirect::to("/")
}

async fn action_handler(state: ServerState, action: Action) -> Fallible<()> {
    let mut mutable = state.mutable.lock().unwrap();
    match action {
        Action::Reveal => {
            if !mutable.reveal {
                mutable.reveal = true;
            }
        }
        Action::Undo => {
            if !mutable.reviews.is_empty() {
                let last_review: Review = mutable.reviews.pop().unwrap();
                if last_review.grade == Grade::Forgot || last_review.grade == Grade::Hard {
                    // Remove the card from the back of the queue.
                    mutable.cards.pop();
                }
                mutable.cards.insert(0, last_review.card);
                mutable.finished_at = None;
                mutable.reveal = false;
            }
        }
        Action::End => {
            finish_session(&mut mutable, &state)?;
        }
        Action::Forgot | Action::Hard | Action::Good | Action::Easy => {
            if mutable.reveal {
                let reviewed_at: Timestamp = Timestamp::now();
                let card: Card = mutable.cards.remove(0);
                let hash: CardHash = card.hash();
                let grade: Grade = action.grade();
                let performance: Performance = mutable.cache.get(hash)?;
                let performance: ReviewedPerformance =
                    update_performance(performance, grade, reviewed_at);
                let review = Review {
                    card: card.clone(),
                    reviewed_at,
                    grade,
                    stability: performance.stability,
                    difficulty: performance.difficulty,
                    interval_raw: performance.interval_raw,
                    due_date: performance.due_date,
                };
                mutable.reviews.push(review);
                mutable.cache.update(
                    hash,
                    reviewed_at,
                    performance.stability,
                    performance.difficulty,
                    performance.interval_raw,
                    performance.due_date,
                )?;

                // Cards graded `Forgot` or `Hard` are put at the back of the
                // queue.
                if grade == Grade::Forgot || grade == Grade::Hard {
                    mutable.cards.push(card.clone());
                }

                mutable.reveal = false;

                // Was this the last card?
                if mutable.cards.is_empty() {
                    finish_session(&mut mutable, &state)?;
                }
            }
        }
    }
    Ok(())
}

fn finish_session(mutable: &mut MutableState, state: &ServerState) -> Fallible<()> {
    log::debug!("Session completed");
    let session_ended_at = Timestamp::now();
    let reviews: Vec<Review> = mutable.reviews.clone();
    let reviews: Vec<ReviewRecord> = reviews.into_iter().map(Review::into_record).collect();
    mutable
        .db
        .save_session(state.session_started_at, session_ended_at, reviews)?;
    mutable.finished_at = Some(session_ended_at);
    for (card_hash, performance) in mutable.cache.iter() {
        mutable
            .db
            .update_card_performance(*card_hash, *performance)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_grade() {
        assert_eq!(Action::Forgot.grade(), Grade::Forgot);
        assert_eq!(Action::Hard.grade(), Grade::Hard);
        assert_eq!(Action::Good.grade(), Grade::Good);
        assert_eq!(Action::Easy.grade(), Grade::Easy);
    }
}
