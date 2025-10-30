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

use std::process::exit;

use clap::Parser;
use clap::Subcommand;
use tokio::spawn;

use crate::cmd::check::check_collection;
use crate::cmd::drill::server::ServerConfig;
use crate::cmd::drill::server::start_server;
use crate::cmd::export::export_collection;
use crate::cmd::orphans::delete_orphans;
use crate::cmd::orphans::list_orphans;
use crate::cmd::stats::StatsFormat;
use crate::cmd::stats::print_stats;
use crate::error::Fallible;
use crate::types::timestamp::Timestamp;
use crate::utils::wait_for_server;

#[derive(Parser)]
#[command(version, about, long_about = None)]
enum Command {
    /// Drill cards through a web interface.
    Drill {
        /// Path to the collection directory. By default, the current working directory is used.
        directory: Option<String>,
        /// Maximum number of cards to drill in a session. By default, all cards due today are drilled.
        #[arg(long)]
        card_limit: Option<usize>,
        /// Maximum number of new cards to drill in a session.
        #[arg(long)]
        new_card_limit: Option<usize>,
        /// The port to use for the web server. Default is 8000.
        #[arg(long, default_value_t = 8000)]
        port: u16,
        /// Only drill cards from this deck.
        #[arg(long)]
        from_deck: Option<String>,
        /// Whether to open the browser automatically. Default is true.
        #[arg(long)]
        open_browser: Option<bool>,
    },
    /// Check the integrity of a collection.
    Check {
        /// Path to the collection directory. By default, the current working directory is used.
        directory: Option<String>,
    },
    /// Print collection statistics.
    Stats {
        /// Path to the collection directory. By default, the current working directory is used.
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
    /// Export a collection.
    Export {
        /// Path to the collection directory. By default, the current working directory is used.
        directory: Option<String>,
        /// Optional path to the output file. By default, the output is printed to stdout.
        #[arg(long)]
        output: Option<String>,
    },
}

#[derive(Subcommand)]
enum OrphanCommand {
    /// List the hashes of all orphan cards in the collection.
    List {
        /// Path to the collection directory. By default, the current working directory is used.
        directory: Option<String>,
    },
    /// Remove all orphan cards from the database.
    Delete {
        /// Path to the collection directory. By default, the current working directory is used.
        directory: Option<String>,
    },
}

pub async fn entrypoint() -> Fallible<()> {
    let cli: Command = Command::parse();
    match cli {
        Command::Drill {
            directory,
            card_limit,
            new_card_limit,
            port,
            from_deck,
            open_browser,
        } => {
            if open_browser.unwrap_or(true) {
                // Start a separate task to open the browser once the server is up.
                spawn(async move {
                    match wait_for_server(port).await {
                        Ok(_) => {
                            let _ = open::that(format!("http://127.0.0.1:{port}/"));
                        }
                        Err(e) => {
                            eprintln!("Failed to connect to server: {e}");
                            exit(-1)
                        }
                    }
                });
            }
            let config = ServerConfig {
                directory,
                port,
                session_started_at: Timestamp::now(),
                card_limit,
                new_card_limit,
                deck_filter: from_deck,
            };
            start_server(config).await
        }
        Command::Check { directory } => check_collection(directory),
        Command::Stats { directory, format } => print_stats(directory, format),
        Command::Orphans { command } => match command {
            OrphanCommand::List { directory } => list_orphans(directory),
            OrphanCommand::Delete { directory } => delete_orphans(directory),
        },
        Command::Export { directory, output } => export_collection(directory, output),
    }
}
