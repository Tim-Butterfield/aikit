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
mod output_cmd;
mod policy;
mod repo;
mod review;
mod script;

use clap::Parser;

use cli::{
    BatchCommand, Cli, Command, InventoryCommand, OutputCommand, RepoCommand, ReviewCommand,
    ScriptCommand,
};
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
        Command::Output(out) => match out.command {
            OutputCommand::List(args) => output_cmd::list(args),
            OutputCommand::Show(args) => output_cmd::show(args),
            OutputCommand::Clean(args) => output_cmd::clean(args),
        },
        Command::Review(rev) => match rev.command {
            ReviewCommand::Generate(args) => review::generate(args),
        },
        Command::Repo(repo_cli) => match repo_cli.command {
            RepoCommand::Init(args) => repo::init(args),
            RepoCommand::Doctor(args) => repo::doctor(args),
        },
        Command::Script(script_cli) => match script_cli.command {
            ScriptCommand::Run(args) => script::run(args),
            ScriptCommand::Check(args) => script::check(args),
        },
    }
}
