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

use std::path::PathBuf;

use clap::Parser;
use walkdir::WalkDir;

use crate::error::Fallible;
use crate::error::fail;

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
            for entry in WalkDir::new(directory) {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() && path.extension().map_or(false, |ext| ext == "md") {
                    let contents = std::fs::read_to_string(path)?;
                }
            }
            Ok(())
        }
    }
}
