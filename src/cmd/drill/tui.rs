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
use std::io;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event;
use ratatui::crossterm::event::Event;
use ratatui::crossterm::event::KeyCode;
use ratatui::crossterm::event::KeyEventKind;
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::EnterAlternateScreen;
use ratatui::crossterm::terminal::LeaveAlternateScreen;
use ratatui::crossterm::terminal::disable_raw_mode;
use ratatui::crossterm::terminal::enable_raw_mode;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;
use ratatui::widgets::Block;
use ratatui::widgets::Gauge;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;

use crate::cmd::drill::cache::Cache;
use crate::cmd::drill::server::AnswerControls;
use crate::cmd::drill::server::bury_siblings;
use crate::cmd::drill::server::filter_deck;
use crate::cmd::drill::state::Review;
use crate::collection::Collection;
use crate::db::ReviewRecord;
use crate::error::Fallible;
use crate::error::fail;
use crate::fsrs::Grade;
use crate::rng::TinyRng;
use crate::rng::shuffle;
use crate::types::card::Card;
use crate::types::card::CardType;
use crate::types::card_hash::CardHash;
use crate::types::date::Date;
use crate::types::performance::Performance;
use crate::types::performance::update_performance;
use crate::types::timestamp::Timestamp;

pub struct TuiConfig {
    pub directory: Option<String>,
    pub session_started_at: Timestamp,
    pub card_limit: Option<usize>,
    pub new_card_limit: Option<usize>,
    pub deck_filter: Option<String>,
    pub answer_controls: AnswerControls,
    pub bury_siblings: bool,
}

struct DrillState {
    reveal: bool,
    db: crate::db::Database,
    cache: Cache,
    cards: Vec<Card>,
    reviews: Vec<Review>,
    finished_at: Option<Timestamp>,
    total_cards: usize,
    session_started_at: Timestamp,
    answer_controls: AnswerControls,
}

pub fn start_tui(config: TuiConfig) -> Fallible<()> {
    let Collection {
        directory: _,
        db,
        cards,
        macros: _,
    } = Collection::new(config.directory)?;

    let today: Date = config.session_started_at.date();

    let db_hashes: HashSet<CardHash> = db.card_hashes()?;
    for card in cards.iter() {
        if !db_hashes.contains(&card.hash()) {
            db.insert_card(card.hash(), config.session_started_at)?;
        }
    }

    let due_today: HashSet<CardHash> = db.due_today(today)?;
    let due_today: Vec<Card> = cards
        .into_iter()
        .filter(|card| due_today.contains(&card.hash()))
        .collect();

    let due_today: Vec<Card> = filter_deck(
        &db,
        due_today,
        config.card_limit,
        config.new_card_limit,
        config.deck_filter,
    )?;

    let due_today: Vec<Card> = if config.bury_siblings {
        bury_siblings(due_today)
    } else {
        due_today
    };

    if due_today.is_empty() {
        return run_empty_tui();
    }

    let seed = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_nanos() as u64,
        Err(_) => return fail("system clock error"),
    };
    let mut rng = TinyRng::from_seed(seed);
    let due_today: Vec<Card> = shuffle(due_today, &mut rng);

    let mut cache = Cache::new();
    for card in due_today.iter() {
        let performance = db.get_card_performance(card.hash())?;
        cache.insert(card.hash(), performance)?;
    }

    let total_cards = due_today.len();
    let mut state = DrillState {
        reveal: false,
        db,
        cache,
        cards: due_today,
        reviews: Vec::new(),
        finished_at: None,
        total_cards,
        session_started_at: config.session_started_at,
        answer_controls: config.answer_controls,
    };

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_tui(&mut terminal, &mut state);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    result
}

fn run_empty_tui() -> Fallible<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = (|| -> Fallible<()> {
        loop {
            terminal.draw(|frame| {
                let area = frame.area();
                let lines = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "  No cards due today.",
                        Style::default().fg(Color::Green),
                    )),
                    Line::from(""),
                    Line::from("  [q] Quit"),
                ];
                let paragraph = Paragraph::new(Text::from(lines))
                    .block(Block::bordered())
                    .wrap(Wrap { trim: false });
                frame.render_widget(paragraph, area);
            })?;

            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                    return Ok(());
                }
            }
        }
    })();

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    result
}

fn run_tui(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut DrillState,
) -> Fallible<()> {
    loop {
        terminal.draw(|frame| draw(frame, state))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            if state.finished_at.is_some() {
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                    return Ok(());
                }
            } else if state.reveal {
                match key.code {
                    KeyCode::Char('1') => handle_grade(state, Grade::Forgot)?,
                    KeyCode::Char('2') if state.answer_controls == AnswerControls::Full => {
                        handle_grade(state, Grade::Hard)?;
                    }
                    KeyCode::Char('3') => handle_grade(state, Grade::Good)?,
                    KeyCode::Char('4') if state.answer_controls == AnswerControls::Full => {
                        handle_grade(state, Grade::Easy)?;
                    }
                    KeyCode::Char('u') => handle_undo(state)?,
                    KeyCode::Char('q') | KeyCode::Esc => finish_session(state)?,
                    _ => {}
                }
            } else {
                match key.code {
                    KeyCode::Char(' ') | KeyCode::Enter => {
                        state.reveal = true;
                    }
                    KeyCode::Char('u') => handle_undo(state)?,
                    KeyCode::Char('q') | KeyCode::Esc => finish_session(state)?,
                    _ => {}
                }
            }
        }
    }
}

fn handle_grade(state: &mut DrillState, grade: Grade) -> Fallible<()> {
    let reviewed_at = Timestamp::now();
    let card: Card = state.cards.remove(0);
    let hash: CardHash = card.hash();
    let performance: Performance = state.cache.get(hash)?;
    let performance = update_performance(performance, grade, reviewed_at);
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

    state
        .cache
        .update(hash, Performance::Reviewed(performance))?;
    if review.should_repeat() {
        state.cards.push(card);
    }
    state.reviews.push(review);
    state.reveal = false;

    if state.cards.is_empty() {
        finish_session(state)?;
    }
    Ok(())
}

fn handle_undo(state: &mut DrillState) -> Fallible<()> {
    if state.reviews.is_empty() {
        return Ok(());
    }
    let last_review: Review = state.reviews.pop().ok_or_else(|| {
        crate::error::ErrorReport::new("no reviews to undo")
    })?;
    if last_review.should_repeat() {
        state.cards.pop();
    }
    let card: Card = last_review.card;
    let hash: CardHash = card.hash();
    state.cards.insert(0, card);
    let performance = state.db.get_card_performance(hash)?;
    state.cache.update(hash, performance)?;
    state.finished_at = None;
    state.reveal = false;
    Ok(())
}

fn finish_session(state: &mut DrillState) -> Fallible<()> {
    let session_ended_at = Timestamp::now();
    let reviews: Vec<Review> = state.reviews.clone();
    let reviews: Vec<ReviewRecord> = reviews.into_iter().map(Review::into_record).collect();
    state
        .db
        .save_session(state.session_started_at, session_ended_at, reviews)?;
    state.finished_at = Some(session_ended_at);
    for (card_hash, performance) in state.cache.iter() {
        state.db.update_card_performance(*card_hash, *performance)?;
    }
    Ok(())
}

fn draw(frame: &mut Frame, state: &DrillState) {
    if state.finished_at.is_some() {
        draw_completion(frame, state);
    } else {
        draw_session(frame, state);
    }
}

fn draw_session(frame: &mut Frame, state: &DrillState) {
    let area = frame.area();

    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(3),
        Constraint::Length(3),
    ])
    .split(area);

    // Progress bar
    let cards_done = state.total_cards - state.cards.len();
    let percent = if state.total_cards > 0 {
        ((cards_done * 100) / state.total_cards) as u16
    } else {
        100
    };
    let gauge = Gauge::default()
        .block(Block::bordered().title(format!(" {cards_done}/{} ", state.total_cards)))
        .gauge_style(Style::default().fg(Color::Green))
        .percent(percent);
    frame.render_widget(gauge, chunks[0]);

    // Card content
    let card = &state.cards[0];
    let deck_name = card.deck_name().clone();
    let front = card.text_front().unwrap_or_default();

    let card_text = if state.reveal {
        match card.card_type() {
            CardType::Basic => {
                let back = card.text_back();
                let mut lines: Vec<Line<'_>> = Vec::new();
                for line in front.lines() {
                    lines.push(Line::from(line.to_string()));
                }
                lines.push(Line::from(""));
                lines.push(Line::from(
                    "─".repeat(area.width.saturating_sub(4) as usize),
                ));
                lines.push(Line::from(""));
                for line in back.lines() {
                    lines.push(Line::from(Span::styled(
                        line.to_string(),
                        Style::default().fg(Color::Yellow),
                    )));
                }
                Text::from(lines)
            }
            CardType::Cloze => {
                let back = card.text_back();
                let mut lines: Vec<Line<'_>> = Vec::new();
                for line in back.lines() {
                    lines.push(Line::from(Span::styled(
                        line.to_string(),
                        Style::default().fg(Color::Yellow),
                    )));
                }
                Text::from(lines)
            }
        }
    } else {
        let mut lines: Vec<Line<'_>> = Vec::new();
        for line in front.lines() {
            lines.push(Line::from(line.to_string()));
        }
        Text::from(lines)
    };

    let card_widget = Paragraph::new(card_text)
        .block(Block::bordered().title(format!(" {deck_name} ")))
        .wrap(Wrap { trim: false });
    frame.render_widget(card_widget, chunks[1]);

    // Controls
    let controls_text = if state.reveal {
        match state.answer_controls {
            AnswerControls::Full => {
                "[1] Forgot  [2] Hard  [3] Good  [4] Easy  |  [u] Undo  [q] End"
            }
            AnswerControls::Binary => "[1] Forgot  [3] Good  |  [u] Undo  [q] End",
        }
    } else if state.reviews.is_empty() {
        "[Space] Reveal  |  [q] End"
    } else {
        "[Space] Reveal  |  [u] Undo  [q] End"
    };
    let controls = Paragraph::new(controls_text).block(Block::bordered());
    frame.render_widget(controls, chunks[2]);
}

fn draw_completion(frame: &mut Frame, state: &DrillState) {
    let area = frame.area();

    let cards_reviewed = state.total_cards - state.cards.len();
    let start = state.session_started_at.into_inner();
    let end = match state.finished_at {
        Some(ts) => ts.into_inner(),
        None => return,
    };
    let duration_s = (end - start).num_seconds();
    let pace: f64 = if cards_reviewed == 0 {
        0.0
    } else {
        duration_s as f64 / cards_reviewed as f64
    };

    let mut lines: Vec<Line<'_>> = Vec::new();
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Session Complete!",
        Style::default().fg(Color::Green),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(format!(
        "  Reviewed {cards_reviewed} cards in {duration_s} seconds."
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(format!(
        "  Total Cards:       {}",
        state.total_cards
    )));
    lines.push(Line::from(format!(
        "  Cards Reviewed:    {cards_reviewed}"
    )));
    lines.push(Line::from(format!(
        "  Duration:          {duration_s}s"
    )));
    lines.push(Line::from(format!(
        "  Pace:              {pace:.2} s/card"
    )));
    lines.push(Line::from(""));
    lines.push(Line::from("  [q] Quit"));

    let paragraph = Paragraph::new(Text::from(lines))
        .block(Block::bordered())
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}
