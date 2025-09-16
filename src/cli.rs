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
use std::io::Read;
use std::path::PathBuf;

use blake3::Hash;
use clap::Parser;
use walkdir::WalkDir;

use crate::db::Database;
use crate::db::Performance;
use crate::error::Fallible;
use crate::error::fail;
use crate::fsrs::Grade;
use crate::parser::Card;
use crate::parser::parse_cards;

#[derive(Parser)]
#[command(version, about, long_about = None)]
enum Command {
    /// Drill cards.
    Drill {
        /// Optional path to the deck directory.
        directory: Option<String>,
    },
}

pub fn entrypoint() -> Fallible<()> {
    let cli: Command = Command::parse();
    match cli {
        Command::Drill { directory } => {
            let directory: PathBuf = match directory {
                Some(dir) => PathBuf::from(dir),
                None => std::env::current_dir()?,
            };
            println!("Drilling in {directory:?}.");
            if !directory.exists() {
                return fail("directory does not exist.");
            }
            let db_path = directory.join("performance.csv");
            let mut db = if db_path.exists() {
                Database::from_csv(&db_path)?
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
            println!("Found {} cards.", all_cards.len());
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
            let due_today = db.due_today();
            let due_today: Vec<Card> = all_cards
                .into_iter()
                .filter(|card| due_today.contains(&card.hash()))
                .collect::<Vec<_>>();
            for card in due_today.into_iter() {
                match card {
                    Card::Basic { question, answer } => {
                        println!("Q: {question}");
                        println!("[press space to reveal]");
                        wait_for_space();
                        println!("A: {answer}");
                    }
                    Card::Cloze { text, start, end } => {
                        let cloze_text = &text[start..end + 1];
                        let mut prompt = text.clone();
                        prompt.replace_range(start..end + 1, "[...]");
                        println!("Q: {prompt}");
                        println!("[press space to reveal]");
                        wait_for_space();
                        let mut answer = text.clone();
                        answer.replace_range(start..end + 1, &format!("[{cloze_text}]"));
                        println!("A: {answer}");
                    }
                }
                let _grade: Grade = read_grade();
            }
            Ok(())
        }
    }
}

fn wait_for_space() {
    loop {
        let ch = std::io::stdin().bytes().next();
        if let Some(Ok(b' ')) = ch {
            break;
        }
    }
}

fn read_grade() -> Grade {
    loop {
        println!("Grade: (1 = Forgot, 2 = Hard, 3 = Good, 4 = Easy)");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        match input.trim().parse::<u8>() {
            Ok(1) => return Grade::Forgot,
            Ok(2) => return Grade::Hard,
            Ok(3) => return Grade::Good,
            Ok(4) => return Grade::Easy,
            _ => println!("Invalid input. Please enter a number between 1 and 4."),
        }
    }
}
