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
pub enum Action {
    Reveal,
    Undo,
    End,
    Forgot,
    Hard,
    Good,
    Easy,
    Shutdown,
    Home,
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
    pub action: Action,
}

/// Result of handling an action on the drill session.
pub enum ActionResult {
    /// Continue drilling (redirect back to the same page).
    Continue,
    /// The session finished (all cards done or user pressed End).
    SessionFinished,
    /// The user requested server shutdown (drill mode only).
    Shutdown,
    /// The user requested to go back to the collection list (serve mode).
    Home,
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
    let result = handle_action(&mut mutable, state.session_started_at, action)?;
    match result {
        ActionResult::Shutdown => {
            // Release the lock before sending shutdown signal.
            drop(mutable);
            let mut shutdown_tx = state.shutdown_tx.lock().unwrap();
            if let Some(tx) = shutdown_tx.take() {
                let _ = tx.send(());
            }
        }
        _ => {}
    }
    Ok(())
}

/// Core action handling logic, reusable by both drill and serve modes.
pub fn handle_action(
    mutable: &mut MutableState,
    session_started_at: Timestamp,
    action: Action,
) -> Fallible<ActionResult> {
    match action {
        Action::Reveal => {
            if !mutable.reveal {
                mutable.reveal = true;
            }
            Ok(ActionResult::Continue)
        }
        Action::Undo => {
            if !mutable.reviews.is_empty() {
                let last_review: Review = mutable.reviews.pop().unwrap();
                if last_review.should_repeat() {
                    // Remove the card from the back of the queue.
                    mutable.cards.pop();
                }
                let card: Card = last_review.card;
                let hash: CardHash = card.hash();
                mutable.cards.insert(0, card);
                // Restore the performance cache to the value in the database
                // if it exists.
                let performance = mutable.db.get_card_performance(hash)?;
                mutable.cache.update(hash, performance)?;
                mutable.finished_at = None;
                mutable.reveal = false;
            }
            Ok(ActionResult::Continue)
        }
        Action::End => {
            finish_session(mutable, session_started_at)?;
            Ok(ActionResult::SessionFinished)
        }
        Action::Shutdown => {
            if mutable.finished_at.is_some() {
                Ok(ActionResult::Shutdown)
            } else {
                Ok(ActionResult::Continue)
            }
        }
        Action::Home => {
            Ok(ActionResult::Home)
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
                    interval_days: performance.interval_days,
                    due_date: performance.due_date,
                };

                mutable
                    .cache
                    .update(hash, Performance::Reviewed(performance))?;
                if review.should_repeat() {
                    mutable.cards.push(card.clone());
                }
                mutable.reviews.push(review);
                mutable.reveal = false;

                // Was this the last card?
                if mutable.cards.is_empty() {
                    finish_session(mutable, session_started_at)?;
                    return Ok(ActionResult::SessionFinished);
                }
            }
            Ok(ActionResult::Continue)
        }
    }
}

fn finish_session(mutable: &mut MutableState, session_started_at: Timestamp) -> Fallible<()> {
    log::debug!("Session completed");
    let session_ended_at = Timestamp::now();
    let reviews: Vec<Review> = mutable.reviews.clone();
    let reviews: Vec<ReviewRecord> = reviews.into_iter().map(Review::into_record).collect();
    mutable
        .db
        .save_session(session_started_at, session_ended_at, reviews)?;
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
    use crate::cmd::drill::cache::Cache;
    use crate::cmd::drill::state::MutableState;
    use crate::db::Database;

    fn make_mutable() -> MutableState {
        MutableState {
            reveal: false,
            db: Database::new(":memory:").unwrap(),
            cache: Cache::new(),
            cards: Vec::new(),
            reviews: Vec::new(),
            finished_at: None,
        }
    }

    #[test]
    fn test_action_grade() {
        assert_eq!(Action::Forgot.grade(), Grade::Forgot);
        assert_eq!(Action::Hard.grade(), Grade::Hard);
        assert_eq!(Action::Good.grade(), Grade::Good);
        assert_eq!(Action::Easy.grade(), Grade::Easy);
    }

    #[test]
    fn test_home_returns_home() {
        let mut mutable = make_mutable();
        let now = Timestamp::now();
        let result = handle_action(&mut mutable, now, Action::Home).unwrap();
        assert!(matches!(result, ActionResult::Home));
    }

    #[test]
    fn test_shutdown_returns_continue_when_unfinished() {
        let mut mutable = make_mutable();
        assert!(mutable.finished_at.is_none());
        let now = Timestamp::now();
        let result = handle_action(&mut mutable, now, Action::Shutdown).unwrap();
        assert!(matches!(result, ActionResult::Continue));
    }

    #[test]
    fn test_reveal_sets_flag() {
        let mut mutable = make_mutable();
        let now = Timestamp::now();
        assert!(!mutable.reveal);
        let result = handle_action(&mut mutable, now, Action::Reveal).unwrap();
        assert!(matches!(result, ActionResult::Continue));
        assert!(mutable.reveal);
    }

    #[test]
    fn test_end_finishes_session() {
        let mut mutable = make_mutable();
        let now = Timestamp::now();
        let result = handle_action(&mut mutable, now, Action::End).unwrap();
        assert!(matches!(result, ActionResult::SessionFinished));
        assert!(mutable.finished_at.is_some());
    }
}
