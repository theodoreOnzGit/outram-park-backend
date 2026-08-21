//! `kovan tokens` — per-commit API-token accounting.
//!
//! A thin frontend over [`kovan_metrics::tokens`]. The write-side subcommands
//! are invoked by the git hooks (`.githooks/prepare-commit-msg` and
//! `post-commit`); `query` is the human/agent entry point for asking what a
//! window of history cost.
//!
//! **Exit-code contract.** Every subcommand here exits `0`, including on
//! internal failure. The hooks must never block a commit because accounting
//! failed — a missing trailer is recoverable, a blocked commit is not.

use std::path::PathBuf;

use clap::Subcommand;
use kovan_metrics::{date::Date, tokens};

/// Subcommands of `kovan tokens`.
#[derive(Subcommand)]
pub enum TokensCommand {
    /// Append the `API-Usage-*` trailers to a commit message file
    /// (`prepare-commit-msg`). Idempotent — safe on amend and rebase.
    Trailer {
        /// Path to the commit message file git is preparing.
        msgfile: PathBuf,
    },
    /// Advance the baseline and regenerate the ledger (`post-commit`).
    Record,
    /// Regenerate `docs/token-usage.md` from the commit trailers.
    Report,
    /// Stamp the baseline at the current cumulative reading (installer).
    Init,
    /// Print the live cumulative reading and the delta since the last commit.
    Show,
    /// Sum the usage recorded in commit trailers over a date window.
    Query {
        /// Window start, `DDMMYY` (day-month-year, 2-digit year).
        #[arg(long = "from")]
        from: Option<String>,
        /// Window end, `DDMMYY`.
        #[arg(long = "to")]
        to: Option<String>,
        /// Branch to report on.
        #[arg(long, default_value = "develop")]
        branch: String,
        /// Include a per-commit breakdown.
        #[arg(long)]
        per_commit: bool,
        /// Emit JSON instead of the human-facing summary.
        #[arg(long)]
        json: bool,
    },
}

/// Dispatch a `kovan tokens` subcommand.
///
/// Only `query` can return an error (a malformed `DDMMYY`), because it is the
/// interactive path. The hook-facing subcommands always succeed.
pub fn run(command: TokensCommand) -> Result<(), String> {
    match command {
        TokensCommand::Trailer { msgfile } => {
            tokens::stamp_trailer(&msgfile);
            Ok(())
        }
        TokensCommand::Record => {
            tokens::record();
            Ok(())
        }
        TokensCommand::Report => {
            tokens::report();
            Ok(())
        }
        TokensCommand::Init => {
            tokens::init();
            Ok(())
        }
        TokensCommand::Show => {
            tokens::show();
            Ok(())
        }
        TokensCommand::Query {
            from,
            to,
            branch,
            per_commit,
            json,
        } => {
            let parse = |s: Option<String>| -> Result<Option<Date>, String> {
                match s {
                    Some(v) => Date::parse_ddmmyy(&v).map(Some).map_err(|e| e.to_string()),
                    None => Ok(None),
                }
            };
            let result = tokens::query(parse(from)?, parse(to)?, &branch);
            if json {
                println!("{}", result.to_json(per_commit));
            } else {
                result.print(per_commit);
            }
            Ok(())
        }
    }
}
