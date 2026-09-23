//! Command-line surface: argument parsing and the interactive run flow.
//!
//! The root command (no subcommand) is the interactive workflow run the baseline offered. The
//! subcommands are the additions of the rewrite: `mcp`, `upgrade`, `versions`, `use`,
//! `uninstall`, `install`, `config` and `completion`. Their argument structs live here so
//! `main.rs` can dispatch to the owning module by reference.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

pub mod config_dir;
pub mod config_init;
pub mod doctor;
pub mod interactive;
pub mod prompter;

use crate::runner::Action;

/// Merge git branches according to configurable release workflows.
#[derive(Debug, Parser)]
#[command(
    name = "merge-pipeline",
    version = crate::VERSION,
    about,
    long_about = None,
    args_conflicts_with_subcommands = true
)]
pub struct Cli {
    #[command(flatten)]
    pub run: RunArgs,

    #[command(subcommand)]
    pub command: Option<Command>,
}

/// The action to perform on the selected workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliAction {
    /// Merge each step and push the targets (asks about pushing unless --auto-push)
    Run,
    /// Merge each step locally; never pushes automatically
    DryRun,
    /// Only resolve the pipeline to branches and print the steps
    Test,
}

impl From<CliAction> for Action {
    fn from(value: CliAction) -> Self {
        match value {
            CliAction::Run => Action::Run,
            CliAction::DryRun => Action::DryRun,
            CliAction::Test => Action::Test,
        }
    }
}

/// Flags for the interactive run (the baseline's yargs options).
#[derive(Debug, Clone, Default, Args)]
pub struct RunArgs {
    /// Working directory of the git repository (asked interactively when omitted)
    #[arg(short = 'c', long, value_name = "DIR")]
    pub cwd: Option<PathBuf>,

    /// Select a workflow by name
    #[arg(short = 'w', long, value_name = "NAME")]
    pub workflow: Option<String>,

    /// Automatically push each branch to origin
    #[arg(short = 'p', long = "auto-push", alias = "auto_push")]
    pub auto_push: bool,

    /// Directory containing workflow JSON files
    #[arg(short = 'f', long, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// What would you like to do
    #[arg(short = 'a', long, value_enum, value_name = "ACTION")]
    pub action: Option<CliAction>,

    /// Answer every confirmation with its default (merge each step, push each branch)
    #[arg(short = 'y', long)]
    pub yes: bool,

    /// Skip pruning stale local branches and fetching new ones before the workflow
    #[arg(long)]
    pub no_sync: bool,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Serve the Model Context Protocol over stdio, for agents
    Mcp,
    /// Update merge-pipeline in place, or check whether an update is available
    Upgrade(UpgradeArgs),
    /// List installed versions; `remove` and `prune` delete old ones
    Versions(VersionsArgs),
    /// Switch to an installed version, downloading it first if needed
    Use(UseArgs),
    /// Remove the managed installation from this machine
    Uninstall(UninstallArgs),
    /// Wire the MCP server into an agent's .mcp.json and create the default workflow files
    Install(InstallArgs),
    /// Inspect configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Print a shell completion script
    Completion {
        /// The shell to generate for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(Debug, Clone, Default, Args)]
pub struct UpgradeArgs {
    /// Install this version instead of the newest
    pub target: Option<String>,

    /// Report whether an update is available, changing nothing
    #[arg(long)]
    pub check: bool,

    /// How many versions to keep on disk
    #[arg(long, default_value_t = 2, value_name = "N")]
    pub keep: usize,

    /// Reinstall even if already on the target version
    #[arg(long)]
    pub force: bool,

    /// Machine-readable output
    #[arg(long)]
    pub json: bool,
}

/// `versions`: the list by default, or one of its subcommands.
#[derive(Debug, Clone, Default, Args)]
pub struct VersionsArgs {
    #[command(subcommand)]
    pub command: Option<VersionsCommand>,

    /// Ask GitHub for the newest release instead of using the last check's result
    #[arg(long)]
    pub check: bool,

    /// Machine-readable output
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Subcommand)]
pub enum VersionsCommand {
    /// Delete installed versions (never the active one or the one running)
    Remove {
        /// Versions to delete, e.g. 0.1.0 or v0.1.0
        #[arg(required = true, value_name = "VERSION")]
        versions: Vec<String>,
        /// Machine-readable output
        #[arg(long)]
        json: bool,
    },
    /// Delete all but the newest N versions (never the active one or the one running)
    Prune {
        /// How many versions to keep on disk
        #[arg(long, default_value_t = 2, value_name = "N")]
        keep: usize,
        /// Machine-readable output
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Clone, Default, Args)]
pub struct UseArgs {
    /// Version to switch to, e.g. 0.2.0 or v0.2.0
    #[arg(value_name = "VERSION")]
    pub version: String,

    /// Machine-readable output
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Default, Args)]
pub struct UninstallArgs {
    /// Skip the confirmation
    #[arg(short = 'y', long)]
    pub yes: bool,

    /// Machine-readable output
    #[arg(long)]
    pub json: bool,
}

/// Where `install` writes the MCP wiring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum InstallScope {
    /// `.mcp.json` in the current directory
    #[default]
    Project,
    /// The user-level agent configuration
    User,
}

#[derive(Debug, Clone, Default, Args)]
pub struct InstallArgs {
    /// Where to write the wiring (and the default workflow files)
    #[arg(long, value_enum, default_value_t = InstallScope::Project)]
    pub scope: InstallScope,

    /// Only wire the MCP server; do not create the workflow directory
    #[arg(long)]
    pub no_config: bool,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ConfigCommand {
    /// Print the workflow directory that would be used, and which rule chose it
    Path {
        /// Directory containing workflow JSON files (same as the root --config)
        #[arg(short = 'f', long, value_name = "PATH")]
        config: Option<PathBuf>,
        /// Working directory to resolve <cwd>/.merge-pipeline against
        #[arg(short = 'c', long, value_name = "DIR")]
        cwd: Option<PathBuf>,
    },
    /// Validate the workflow files: JSON, names, regexes, ordering, and branch matches
    Doctor {
        /// Directory containing workflow JSON files (same as the root --config)
        #[arg(short = 'f', long, value_name = "PATH")]
        config: Option<PathBuf>,
        /// Git repository to check the patterns against (also resolves <cwd>/.merge-pipeline)
        #[arg(short = 'c', long, value_name = "DIR")]
        cwd: Option<PathBuf>,
        /// Machine-readable output
        #[arg(long)]
        json: bool,
    },
    /// Create the workflow directory with the default workflow files
    Init {
        /// Where to create it: <cwd>/.merge-pipeline or the user config directory
        #[arg(long, value_enum, default_value_t = InstallScope::Project)]
        scope: InstallScope,
        /// Working directory for project scope (default: the current directory)
        #[arg(short = 'c', long, value_name = "DIR")]
        cwd: Option<PathBuf>,
        /// Overwrite files that already exist
        #[arg(long)]
        force: bool,
    },
}

/// Write the completion script for `shell` to stdout.
pub fn print_completion(shell: clap_complete::Shell) {
    use clap::CommandFactory;
    let mut command = Cli::command();
    clap_complete::generate(
        shell,
        &mut command,
        "merge-pipeline",
        &mut std::io::stdout(),
    );
}

/// The interactive flow. Returns the process exit code; errors are already rendered.
pub fn run_interactive(args: &RunArgs) -> i32 {
    interactive::run(args)
}

/// `config path`: print the workflow directory that would be used and the rule that chose it.
pub fn run_config_path(config: Option<&PathBuf>, cwd: Option<&PathBuf>) -> anyhow::Result<()> {
    let resolved =
        config_dir::resolve_from_process(config.map(PathBuf::as_path), cwd.map(PathBuf::as_path))?;
    println!("{}", resolved.path.display());
    println!("  chosen by: {}", resolved.rule.describe());
    Ok(())
}

/// `config doctor`: run every check and print the report. Returns the exit code: 0 when there
/// are no errors (warnings are allowed), 1 otherwise.
pub fn run_config_doctor(
    config: Option<&PathBuf>,
    cwd: Option<&PathBuf>,
    json: bool,
) -> anyhow::Result<i32> {
    let resolved =
        config_dir::resolve_from_process(config.map(PathBuf::as_path), cwd.map(PathBuf::as_path))?;

    // Only a repository named explicitly is inspected; the process cwd is not assumed to be one.
    let branches = match cwd {
        Some(repo) if repo.join(".git").exists() => {
            Some(crate::git::Git::new(repo.clone()).branch_list()?)
        }
        _ => None,
    };

    let report = doctor::diagnose(
        &resolved.path,
        resolved.rule.describe(),
        branches.as_deref(),
    );
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print!("{}", doctor::render(crate::ui::Palette::detect(), &report));
    }
    Ok(if report.ok { 0 } else { 1 })
}

/// `config init`, also run by `install`: write the default workflows for `scope`.
pub fn run_config_init(
    scope: InstallScope,
    cwd: Option<&PathBuf>,
    force: bool,
) -> anyhow::Result<()> {
    let cwd = match cwd {
        Some(cwd) => cwd.clone(),
        None => std::env::current_dir()?,
    };
    let dir = config_init::target_dir(scope, &cwd)?;
    let report = config_init::write_defaults(&dir, force)?;
    print!("{}", config_init::render(&report));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn debug_assert_the_command_definition() {
        Cli::command().debug_assert();
    }

    #[test]
    fn root_flags_parse_with_short_and_long_forms() {
        let cli = Cli::try_parse_from([
            "merge-pipeline",
            "-c",
            "/repo",
            "-w",
            "Patch",
            "-p",
            "-f",
            "/cfg",
            "-a",
            "dry-run",
        ])
        .unwrap();
        assert!(cli.command.is_none());
        assert_eq!(cli.run.cwd, Some(PathBuf::from("/repo")));
        assert_eq!(cli.run.workflow.as_deref(), Some("Patch"));
        assert!(cli.run.auto_push);
        assert_eq!(cli.run.config, Some(PathBuf::from("/cfg")));
        assert_eq!(cli.run.action, Some(CliAction::DryRun));
    }

    #[test]
    fn no_sync_flag_parses_and_defaults_off() {
        assert!(!Cli::try_parse_from(["merge-pipeline"]).unwrap().run.no_sync);
        assert!(
            Cli::try_parse_from(["merge-pipeline", "--no-sync"])
                .unwrap()
                .run
                .no_sync
        );
    }

    #[test]
    fn legacy_auto_push_spelling_is_accepted() {
        let cli = Cli::try_parse_from(["merge-pipeline", "--auto_push"]).unwrap();
        assert!(cli.run.auto_push);
        let cli = Cli::try_parse_from(["merge-pipeline", "--auto-push"]).unwrap();
        assert!(cli.run.auto_push);
    }

    #[test]
    fn invalid_action_is_rejected() {
        assert!(Cli::try_parse_from(["merge-pipeline", "-a", "nope"]).is_err());
    }

    #[test]
    fn upgrade_defaults_and_flags() {
        let cli = Cli::try_parse_from(["merge-pipeline", "upgrade"]).unwrap();
        match cli.command {
            Some(Command::Upgrade(args)) => {
                assert_eq!(args.keep, 2);
                assert!(!args.check && !args.force && !args.json);
                assert_eq!(args.target, None);
            }
            other => panic!("unexpected {other:?}"),
        }
        let cli = Cli::try_parse_from([
            "merge-pipeline",
            "upgrade",
            "0.2.0",
            "--check",
            "--keep",
            "3",
            "--force",
            "--json",
        ])
        .unwrap();
        match cli.command {
            Some(Command::Upgrade(args)) => {
                assert_eq!(args.target.as_deref(), Some("0.2.0"));
                assert_eq!(args.keep, 3);
                assert!(args.check && args.force && args.json);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn versions_and_use_parse() {
        let cli = Cli::try_parse_from(["merge-pipeline", "versions", "--json", "--check"]).unwrap();
        match cli.command {
            Some(Command::Versions(args)) => {
                assert!(args.command.is_none());
                assert!(args.json && args.check);
            }
            other => panic!("unexpected {other:?}"),
        }
        let cli = Cli::try_parse_from([
            "merge-pipeline",
            "versions",
            "remove",
            "0.1.0",
            "v0.0.9",
            "--json",
        ])
        .unwrap();
        match cli.command {
            Some(Command::Versions(VersionsArgs {
                command: Some(VersionsCommand::Remove { versions, json }),
                ..
            })) => {
                assert_eq!(versions, vec!["0.1.0", "v0.0.9"]);
                assert!(json);
            }
            other => panic!("unexpected {other:?}"),
        }
        assert!(
            Cli::try_parse_from(["merge-pipeline", "versions", "remove"]).is_err(),
            "remove needs at least one version"
        );
        let cli =
            Cli::try_parse_from(["merge-pipeline", "versions", "prune", "--keep", "1"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Versions(VersionsArgs {
                command: Some(VersionsCommand::Prune {
                    keep: 1,
                    json: false
                }),
                ..
            }))
        ));
        let cli = Cli::try_parse_from(["merge-pipeline", "use", "0.2.0"]).unwrap();
        match cli.command {
            Some(Command::Use(args)) => {
                assert_eq!(args.version, "0.2.0");
                assert!(!args.json);
            }
            other => panic!("unexpected {other:?}"),
        }
        assert!(Cli::try_parse_from(["merge-pipeline", "use"]).is_err());
    }

    #[test]
    fn other_subcommands_parse() {
        assert!(matches!(
            Cli::try_parse_from(["merge-pipeline", "mcp"])
                .unwrap()
                .command,
            Some(Command::Mcp)
        ));
        assert!(matches!(
            Cli::try_parse_from(["merge-pipeline", "uninstall", "-y", "--json"])
                .unwrap()
                .command,
            Some(Command::Uninstall(UninstallArgs {
                yes: true,
                json: true
            }))
        ));
        assert!(matches!(
            Cli::try_parse_from(["merge-pipeline", "install", "--scope", "user"])
                .unwrap()
                .command,
            Some(Command::Install(InstallArgs {
                scope: InstallScope::User,
                no_config: false
            }))
        ));
        assert!(matches!(
            Cli::try_parse_from(["merge-pipeline", "install", "--no-config"])
                .unwrap()
                .command,
            Some(Command::Install(InstallArgs {
                scope: InstallScope::Project,
                no_config: true
            }))
        ));
        assert!(matches!(
            Cli::try_parse_from(["merge-pipeline", "config", "init", "--force"])
                .unwrap()
                .command,
            Some(Command::Config {
                command: ConfigCommand::Init { force: true, .. }
            })
        ));
        assert!(matches!(
            Cli::try_parse_from(["merge-pipeline", "config", "path"])
                .unwrap()
                .command,
            Some(Command::Config {
                command: ConfigCommand::Path { .. }
            })
        ));
        assert!(matches!(
            Cli::try_parse_from(["merge-pipeline", "config", "doctor", "-f", "/cfg", "--json"])
                .unwrap()
                .command,
            Some(Command::Config {
                command: ConfigCommand::Doctor { json: true, .. }
            })
        ));
        assert!(matches!(
            Cli::try_parse_from(["merge-pipeline", "completion", "zsh"])
                .unwrap()
                .command,
            Some(Command::Completion {
                shell: clap_complete::Shell::Zsh
            })
        ));
    }
}
