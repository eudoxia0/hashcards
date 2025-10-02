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

use std::time::Duration;

use clap::Parser;
use tokio::net::TcpStream;
use tokio::spawn;
use tokio::time::sleep;

use crate::cmd::check::check_deck;
use crate::cmd::drill::server::start_server;
use crate::cmd::orphans::OrphanCommand;
use crate::cmd::orphans::delete_orphans;
use crate::cmd::orphans::list_orphans;
use crate::cmd::stats::StatsFormat;
use crate::cmd::stats::print_deck_stats;
use crate::error::Fallible;
use crate::types::timestamp::Timestamp;

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub enum Command {
    /// Drill cards through a web interface.
    Drill {
        /// Path to the deck directory. By default, the current working directory is used.
        directory: Option<String>,
        /// Maximum number of cards to drill in a session. By default, all cards due today are drilled.
        #[arg(long)]
        card_limit: Option<usize>,
        /// Maximum number of new cards to drill in a session.
        #[arg(long)]
        new_card_limit: Option<usize>,
    },
    /// Check the integrity of a deck.
    Check {
        /// Path to the deck directory. By default, the current working directory is used.
        directory: Option<String>,
    },
    /// Print deck statistics.
    Stats {
        /// Path to the deck directory. By default, the current working directory is used.
        directory: Option<String>,
        /// Which output format to use.
        #[arg(long, default_value_t = StatsFormat::Html)]
        format: StatsFormat,
    },
    /// Commands relating to orphan cards.
    Orphans {
        #[command(subcommand)]
        command: OrphanCommand,
    },
}

pub async fn entrypoint() -> Fallible<()> {
    let cli: Command = Command::parse();
    match cli {
        Command::Drill {
            directory,
            card_limit,
            new_card_limit,
        } => {
            // Start a separate task to open the browser once the server is up.
            spawn(async move {
                loop {
                    if let Ok(stream) = TcpStream::connect("0.0.0.0:8000").await {
                        drop(stream);
                        break;
                    }
                    sleep(Duration::from_millis(1)).await;
                }
                let _ = open::that("http://0.0.0.0:8000/");
            });
            start_server(directory, Timestamp::now(), card_limit, new_card_limit).await
        }
        Command::Check { directory } => check_deck(directory),
        Command::Stats { directory, format } => print_deck_stats(directory, format),
        Command::Orphans { command } => match command {
            OrphanCommand::List { directory } => list_orphans(directory),
            OrphanCommand::Delete { directory } => delete_orphans(directory),
        },
    }
}
