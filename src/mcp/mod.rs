//! Model Context Protocol server over stdio (JSON-RPC 2.0), and the `install` wiring command.
//!
//! The protocol layer is deliberately small: `initialize`, `ping`, `tools/list`, `tools/call`,
//! and the `notifications/initialized` notification. Every tool wraps the same core the
//! interactive CLI uses; the only difference is that questions are answered from tool arguments
//! through a [`crate::prompt::ScriptedPrompter`] instead of a terminal.

pub mod install;
pub mod jsonrpc;
pub mod server;
pub mod tools;

pub use server::{SUPPORTED_PROTOCOL_VERSIONS, Server};
pub use tools::{Registry, Tool, ToolError};

use crate::cli::InstallArgs;

/// Run the MCP server over stdin/stdout until the client disconnects.
pub fn serve() -> anyhow::Result<()> {
    server::serve_stdio()
}

/// Write the MCP wiring for this binary into an agent configuration file.
pub fn run_install(args: &InstallArgs) -> anyhow::Result<()> {
    let outcome = install::install(args.scope, &install::default_target(args.scope)?)?;
    println!("{outcome}");
    Ok(())
}
