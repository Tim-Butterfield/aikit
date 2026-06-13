//! aikit — personal CLI for deterministic AI-agent workflow support.
//!
//! The binary is agent-agnostic: no provider/model logic, no knowledge of any
//! specific AI agent, CLI, slash command, or model. `main` parses the CLI,
//! dispatches to a command, and maps errors to the documented exit codes.

mod batch;
mod cli;
mod errors;
mod formats;
mod inventory;
mod output;
mod repo;
mod review;

use clap::Parser;

use cli::{BatchCommand, Cli, Command, InventoryCommand, ReviewCommand};
use errors::AikitError;

fn main() {
    // clap handles --help/--version and invalid usage (exit code 2) itself.
    let cli = Cli::parse();
    if let Err(err) = run(cli) {
        err.report();
        std::process::exit(err.exit_code());
    }
}

fn run(cli: Cli) -> Result<(), AikitError> {
    match cli.command {
        Command::Batch(batch) => match batch.command {
            BatchCommand::Start(args) => batch::start(args),
            BatchCommand::Changed(args) => batch::changed(args),
        },
        Command::Inventory(inv) => match inv.command {
            InventoryCommand::Repo(args) => inventory::repo(args),
        },
        Command::Review(rev) => match rev.command {
            ReviewCommand::Generate(args) => review::generate(args),
        },
    }
}
