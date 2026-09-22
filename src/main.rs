//! Entry point for merge-pipeline.

use std::io::IsTerminal;

use clap::Parser;
use merge_pipeline::cli::{self, Cli, Command};
use merge_pipeline::ui::Palette;

fn main() {
    let cli = Cli::parse();

    let code = match cli.command {
        None => {
            let code = cli::run_interactive(&cli.run);
            update_nudge();
            code
        }
        Some(command) => match run_subcommand(command) {
            Ok(()) => 0,
            Err(error) => {
                eprintln!("Error: {error:#}");
                1
            }
        },
    };

    if code != 0 {
        std::process::exit(code);
    }
}

fn run_subcommand(command: Command) -> anyhow::Result<()> {
    match command {
        Command::Mcp => merge_pipeline::mcp::serve(),
        Command::Upgrade(args) => merge_pipeline::selfupdate::run_upgrade(&args),
        Command::Uninstall(args) => merge_pipeline::selfupdate::run_uninstall(&args),
        Command::Install(args) => merge_pipeline::mcp::run_install(&args),
        Command::Config { command } => match command {
            cli::ConfigCommand::Path { config, cwd } => {
                cli::run_config_path(config.as_ref(), cwd.as_ref())
            }
        },
        Command::Completion { shell } => {
            cli::print_completion(shell);
            Ok(())
        }
    }
}

/// After an interactive run on a terminal, mention a newer release if one is known. Best effort:
/// a panic or slow lookup inside the check must never affect the run's outcome.
fn update_nudge() {
    if !std::io::stdout().is_terminal() {
        return;
    }
    let line = std::panic::catch_unwind(merge_pipeline::selfupdate::nudge::maybe_line)
        .ok()
        .flatten();
    if let Some(line) = line {
        eprintln!("{}", Palette::detect().dim(&line));
    }
}
