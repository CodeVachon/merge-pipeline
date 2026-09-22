//! merge-pipeline core library.
//!
//! Everything that decides *what* to do lives here and performs no terminal I/O, so the same
//! implementation serves the interactive CLI (`src/main.rs`) and the MCP server.

pub mod cli;
pub mod config;
pub mod conflicts;
pub mod git;
pub mod mcp;
pub mod pipeline;
pub mod prompt;
pub mod runner;
pub mod selfupdate;
pub mod text;
pub mod ui;

/// The version reported by `--version`, baked in from Cargo.toml at compile time.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
