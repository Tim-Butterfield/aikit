//! aikit — personal CLI for deterministic AI-agent workflow support.
//!
//! The binary is agent-agnostic: no provider/model logic, no knowledge of any
//! specific AI agent, CLI, slash command, or model. `main` parses the CLI,
//! dispatches to a command, and maps errors to the documented exit codes.

mod batch;
mod cli;
mod config;
mod diff;
mod env;
mod errors;
mod formats;
mod inventory;
mod mcp;
mod output;
mod output_cmd;
mod policy;
mod repo;
mod review;
mod scan;
mod script;
mod version;

use clap::Parser;

use cli::{
    BatchCommand, Cli, Command, ConfigCommand, EnvCommand, InventoryCommand, OutputCommand,
    ReviewCommand, ScanCommand, ScriptCommand,
};
use errors::AikitError;

fn main() {
    // clap handles --help/--version and invalid usage (exit code 2) itself.
    let cli = Cli::parse();

    // Captured before anything can fail, so even the `--cwd` rejection below knows which
    // channel the caller asked for. `run` consumes the CLI, so this cannot wait.
    let wants_json = cli.wants_json();

    // Applied before dispatch: every command resolves its root from the working directory,
    // so this has to take effect first rather than being threaded through each command.
    //
    // Exit 2 (invalid usage), because a directory that cannot be entered is an invalid
    // argument *value* — the same class as `--fail-on bogus`, which clap rejects with 2.
    // clap only misses this one because validating it needs the filesystem.
    //
    // Unlike clap's own rejections, aikit is running here, so it can also honour `--json`.
    // The record is therefore emitted where the parser physically cannot emit one.
    if let Some(dir) = &cli.cwd {
        if let Err(e) = std::env::set_current_dir(dir) {
            let message = format!("--cwd {dir:?} could not be entered: {e}");
            if wants_json {
                // `blocked_state` is null: this is not one of the named blocked states, and
                // null is exactly how a caller tells "the environment did not cooperate"
                // from "a mechanical precondition was refused".
                errors::print_error_json(None, 2, &message);
            }
            eprintln!("error: {message}");
            std::process::exit(2);
        }
    }
    if let Err(err) = run(cli) {
        // stdout is the machine channel. Under `--json` a failure emits a record there too,
        // so a caller never has to scrape prose from stderr to learn what happened — unless
        // the command already printed its own record, in which case a second document would
        // make stdout unparseable.
        if wants_json && !err.json_emitted() {
            err.report_json();
        }
        // Prose always goes to stderr, whatever the mode.
        err.report();
        std::process::exit(err.exit_code());
    }
}

fn run(cli: Cli) -> Result<(), AikitError> {
    match cli.command {
        Command::Batch(batch) => match batch.command {
            BatchCommand::Start(args) => batch::start(args),
            BatchCommand::Changed(args) => batch::changed(args),
            BatchCommand::List(args) => batch::list(args),
            BatchCommand::Show(args) => batch::show(args),
            // Implemented in `diff`, surfaced under `batch`: it operates on a batch anchor,
            // and a top-level `diff` namespace promised a general facility aikit lacks.
            BatchCommand::Diff(args) => diff::anchor(args),
        },
        Command::Env(env_cli) => match env_cli.command {
            EnvCommand::Snapshot(args) => env::snapshot(args),
        },
        Command::Scan(scan_cli) => match scan_cli.command {
            ScanCommand::Secrets(args) => scan::secrets(args),
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
        Command::Config(cfg_cli) => match cfg_cli.command {
            ConfigCommand::Show(args) => config::show(args),
        },
        Command::Init(args) => repo::init(args),
        Command::Doctor(args) => repo::doctor(args),
        Command::AgentsMd => match mcp::resolve_agents_md() {
            Ok((content, _source)) => {
                print!("{content}");
                Ok(())
            }
            Err(msg) => Err(AikitError::other(msg)),
        },
        Command::Script(script_cli) => match script_cli.command {
            ScriptCommand::Run(args) => script::run(args),
            ScriptCommand::Check(args) => script::check(args),
        },
        Command::Mcp(args) => mcp::serve(args),
        Command::Version(args) => version::version(args),
    }
}
