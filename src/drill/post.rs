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

use crate::drill::state::ServerState;
use crate::error::Fallible;
use crate::fsrs::Grade;
use crate::types::review::Review;
use crate::types::review::update_card;
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
    let today = state.session_started_at.local_date();
    match action {
        Action::Reveal => {
            if mutable.reveal {
                log::error!("Revealing a card that is already revealed.");
            } else {
                mutable.reveal = true;
            }
        }
        Action::Undo => {
            if !mutable.reviewed.is_empty() {
                let r = mutable.reviews.pop().unwrap();
                let card = if r.grade == Grade::Forgot || r.grade == Grade::Hard {
                    let _ = mutable.reviewed.pop().unwrap();
                    mutable.cards.pop().unwrap()
                } else {
                    mutable.reviewed.pop().unwrap()
                };
                mutable.cards.insert(0, card);
            } else {
                log::error!("No reviewed cards to undo.");
            }
        }
        Action::End => {
            log::debug!("Session completed");
            let session_ended_at = Timestamp::now();
            let reviews = mutable.reviews.clone();
            mutable
                .db
                .save_session(state.session_started_at, session_ended_at, reviews)?;
            mutable.finished = true;
        }
        Action::Forgot | Action::Hard | Action::Good | Action::Easy => {
            if !mutable.reveal {
                log::error!("Answering a card that is not revealed.");
            } else {
                let card = mutable.cards.remove(0);
                let hash = card.hash();
                let latest_review = match mutable.db.get_latest_review(hash)? {
                    Some(r) => Some(r),
                    None => {
                        // Look through the in-memory review database.
                        let mut found: Option<Review> = None;
                        for r in mutable.reviews.iter().rev() {
                            if r.card_hash == hash {
                                found = Some(r.clone());
                                break;
                            }
                        }
                        found
                    }
                };
                let grade: Grade = match action {
                    Action::Forgot => Grade::Forgot,
                    Action::Hard => Grade::Hard,
                    Action::Good => Grade::Good,
                    Action::Easy => Grade::Easy,
                    _ => unreachable!(),
                };
                let parameters = update_card(latest_review, grade, today);
                let review = Review {
                    card_hash: hash,
                    reviewed_at: Timestamp::now(),
                    grade,
                    stability: parameters.stability,
                    difficulty: parameters.difficulty,
                    due_date: parameters.due_date,
                };
                mutable.reviews.push(review);

                let diff_percent = ((parameters.difficulty - 1.0) / (9.0)) * 100.0;
                log::debug!(
                    "{} {} S={:.2}d D={:.2}% due={}",
                    &hash.to_hex()[..8],
                    grade.as_str(),
                    parameters.stability,
                    diff_percent,
                    parameters.due_date.into_inner()
                );

                // Cards graded `Forgot` or `Hard` are put at the back of the
                // queue.
                if grade == Grade::Forgot || grade == Grade::Hard {
                    mutable.cards.push(card.clone());
                }

                mutable.reviewed.push(card);

                mutable.reveal = false;

                // Was this the last card?
                if mutable.cards.is_empty() {
                    log::debug!("Session completed");
                    let session_ended_at = Timestamp::now();
                    let reviews = mutable.reviews.clone();
                    mutable
                        .db
                        .save_session(state.session_started_at, session_ended_at, reviews)?;
                    mutable.finished = true;
                }
            }
        }
    }
    Ok(())
}
