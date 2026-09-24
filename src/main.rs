//! Entry point for merge-pipeline.

use clap::Parser;
use merge_pipeline::cli::{self, Cli, Command};

fn main() {
    let cli = Cli::parse();

    let code = match cli.command {
        None => cli::run_interactive(&cli.run),
        Some(command) => match run_subcommand(command) {
            Ok(code) => code,
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

fn run_subcommand(command: Command) -> anyhow::Result<i32> {
    match command {
        Command::Mcp => merge_pipeline::mcp::serve().map(|()| 0),
        Command::Upgrade(args) => merge_pipeline::selfupdate::run_upgrade(&args).map(|()| 0),
        Command::Versions(args) => merge_pipeline::selfupdate::run_versions(&args).map(|()| 0),
        Command::Use(args) => merge_pipeline::selfupdate::run_use(&args).map(|()| 0),
        Command::Uninstall(args) => merge_pipeline::selfupdate::run_uninstall(&args).map(|()| 0),
        Command::Install(args) => {
            merge_pipeline::mcp::run_install(&args)?;
            // Wiring the agent without any workflows to run would leave a fresh repo unusable, so
            // the default workflow files are created too unless told not to. Done here, so the
            // MCP module stays unaware of workflow directories.
            if !args.no_config {
                cli::run_config_init(args.scope, None, false)?;
            }
            Ok(0)
        }
        Command::Config { command } => match command {
            cli::ConfigCommand::Path { config, cwd } => {
                cli::run_config_path(config.as_ref(), cwd.as_ref()).map(|()| 0)
            }
            cli::ConfigCommand::Doctor { config, cwd, json } => {
                cli::run_config_doctor(config.as_ref(), cwd.as_ref(), json)
            }
            cli::ConfigCommand::Init { scope, cwd, force } => {
                cli::run_config_init(scope, cwd.as_ref(), force).map(|()| 0)
            }
        },
        Command::Completion { shell } => {
            cli::print_completion(shell);
            Ok(0)
        }
    }
}
