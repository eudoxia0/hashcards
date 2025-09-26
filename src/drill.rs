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

use std::collections::HashSet;
use std::path::PathBuf;

use blake3::Hash;
use chrono::NaiveDate;
use console::Term;
use csv::Reader;
use walkdir::WalkDir;

use crate::db::Database;
use crate::db::Performance;
use crate::error::Fallible;
use crate::error::fail;
use crate::fsrs::Grade;
use crate::parser::Card;
use crate::parser::CardContent;
use crate::parser::parse_cards;

pub fn drill(directory: PathBuf, today: NaiveDate) -> Fallible<()> {
    let term = Term::stdout();
    if !directory.exists() {
        return fail("directory does not exist.");
    }
    let db_path = directory.join("performance.csv");
    let mut db = if db_path.exists() {
        let mut reader = Reader::from_path(&db_path)?;
        Database::from_csv(&mut reader)?
    } else {
        Database::empty()
    };
    let mut all_cards = Vec::new();
    for entry in WalkDir::new(directory) {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "md") {
            let contents = std::fs::read_to_string(path)?;
            let cards = parse_cards(&contents);
            all_cards.extend(cards);
        }
    }
    let db_keys: HashSet<Hash> = db.keys();
    let dir_keys: HashSet<Hash> = all_cards.iter().map(|card| card.hash()).collect();
    // If a card is in the DB, but not in the directory, it was deleted. Therefore, remove it from the database.
    let to_remove: Vec<Hash> = db_keys.difference(&dir_keys).cloned().collect();
    for hash in to_remove {
        db.remove(&hash);
    }
    // If a card is in the directory, but not in the DB, it is new. Add it to the database.
    let to_add: Vec<Hash> = dir_keys.difference(&db_keys).cloned().collect();
    for hash in to_add {
        db.insert(hash, Performance::New);
    }
    // Find cards due today.
    let due_today = db.due_today(today);
    let mut due_today: Vec<Card> = all_cards
        .into_iter()
        .filter(|card| due_today.contains(&card.hash()))
        .collect::<Vec<_>>();
    if due_today.is_empty() {
        println!("No cards due today.");
        return Ok(());
    }
    while !due_today.is_empty() {
        // Pop the first card.
        let card = due_today.remove(0);
        term.clear_screen()?;
        let hash = card.hash();
        let performance = db.get(hash).unwrap();
        match &card.content {
            CardContent::Basic { question, answer } => {
                term.write_line(&format!("Q: {question}"))?;
                wait_for_keypress(&term)?;
                term.write_line(&format!("A: {answer}"))?;
            }
            CardContent::Cloze { text, start, end } => {
                let cloze_text = &text[*start..*end + 1];
                let mut prompt = text.clone();
                prompt.replace_range(*start..*end + 1, "[...]");
                term.write_line(&format!("Q: {prompt}"))?;
                wait_for_keypress(&term)?;
                let mut answer = text.clone();
                answer.replace_range(*start..*end + 1, &format!("[{cloze_text}]"));
                term.write_line(&format!("A: {answer}"))?;
            }
        }
        let grade: Grade = read_grade(&term)?;
        let performance = performance.update(grade, today);
        db.update(hash, performance);
        // Was the card forgotten? Put it at the back.
        if grade == Grade::Forgot {
            due_today.push(card);
        }
    }
    let mut writer = csv::Writer::from_path(&db_path)?;
    db.to_csv(&mut writer)?;
    Ok(())
}

fn wait_for_keypress(term: &Term) -> Fallible<()> {
    term.write_line("[press any key to reveal]")?;
    let _ = term.read_key()?;
    term.clear_line()?;
    Ok(())
}

fn read_grade(term: &Term) -> Fallible<Grade> {
    term.write_line("Grade: (1 = Forgot, 2 = Hard, 3 = Good, 4 = Easy)")?;
    loop {
        let c = term.read_char()?;
        match c {
            '1' => return Ok(Grade::Forgot),
            '2' => return Ok(Grade::Hard),
            '3' => return Ok(Grade::Good),
            '4' => return Ok(Grade::Easy),
            _ => (),
        }
    }
}
