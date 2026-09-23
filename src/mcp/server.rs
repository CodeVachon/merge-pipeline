//! The MCP request loop: newline-delimited JSON-RPC over any `BufRead`/`Write` pair.
//!
//! stdout carries protocol messages only; anything diagnostic goes to stderr.

use std::io::{BufRead, Write};

use serde_json::{Value, json};

use super::jsonrpc::{self, Request, RpcError};
use super::tools::Registry;

/// Protocol revisions this server speaks. The client's choice is echoed when it is one of these;
/// otherwise the newest is offered, as the specification's version negotiation prescribes.
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];

/// What an agent is told about this server at `initialize`.
const INSTRUCTIONS: &str = "merge-pipeline merges git branches along configured pipelines. \
Start with list_workflows and inspect_repo, use plan_workflow to resolve a pipeline to concrete \
branches (supplying `selections` for any pattern reported as ambiguous), and only then call \
run_workflow with confirm=true. run_workflow refuses dirty repositories and never prompts: \
anything it cannot decide from its arguments is returned as an error naming the open question.";

pub struct Server {
    registry: Registry,
    protocol_version: Option<String>,
}

impl Default for Server {
    fn default() -> Self {
        Self::new(Registry::default())
    }
}

impl Server {
    pub fn new(registry: Registry) -> Self {
        Self {
            registry,
            protocol_version: None,
        }
    }

    /// The protocol version negotiated at `initialize`, once it has happened.
    pub fn protocol_version(&self) -> Option<&str> {
        self.protocol_version.as_deref()
    }

    /// Read lines from `reader` until EOF, writing one response per request to `writer`.
    pub fn run<R: BufRead, W: Write>(&mut self, reader: R, mut writer: W) -> std::io::Result<()> {
        for line in reader.lines() {
            let line = line?;
            if let Some(response) = self.handle_line(&line) {
                serde_json::to_writer(&mut writer, &response)?;
                writer.write_all(b"\n")?;
                writer.flush()?;
            }
        }
        Ok(())
    }

    /// Handle one raw line. `None` means nothing is sent back (blank line or notification).
    pub fn handle_line(&mut self, line: &str) -> Option<Value> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }
        let request = match jsonrpc::parse_line(trimmed) {
            Ok(request) => request,
            Err(error_response) => return Some(error_response),
        };
        self.handle(request)
    }

    /// Dispatch a parsed request. Notifications never get a reply, whatever their method.
    pub fn handle(&mut self, request: Request) -> Option<Value> {
        if request.is_notification() {
            self.handle_notification(&request);
            return None;
        }

        let id = request.id_value();
        let params = request.params.clone().unwrap_or(Value::Null);
        let outcome = match request.method.as_str() {
            "initialize" => Ok(self.initialize(&params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(self.registry.list()),
            "tools/call" => self.call_tool(&params),
            other => Err(RpcError::method_not_found(other)),
        };

        Some(match outcome {
            Ok(result) => jsonrpc::success(id, result),
            Err(error) => jsonrpc::failure(id, error),
        })
    }

    fn handle_notification(&mut self, request: &Request) {
        match request.method.as_str() {
            "notifications/initialized" | "notifications/cancelled" => {}
            other => eprintln!("merge-pipeline mcp: ignoring notification {other}"),
        }
    }

    fn initialize(&mut self, params: &Value) -> Value {
        let requested = params
            .get("protocolVersion")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let negotiated = if SUPPORTED_PROTOCOL_VERSIONS.contains(&requested) {
            requested
        } else {
            SUPPORTED_PROTOCOL_VERSIONS[0]
        };
        self.protocol_version = Some(negotiated.to_string());

        json!({
            "protocolVersion": negotiated,
            "capabilities": { "tools": {} },
            "serverInfo": {
                "name": "merge-pipeline",
                "version": crate::VERSION
            },
            "instructions": INSTRUCTIONS
        })
    }

    fn call_tool(&self, params: &Value) -> Result<Value, RpcError> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| RpcError::invalid_params("tools/call requires a string `name`"))?;
        let arguments = match params.get("arguments") {
            None | Some(Value::Null) => json!({}),
            Some(Value::Object(map)) => Value::Object(map.clone()),
            Some(_) => {
                return Err(RpcError::invalid_params(
                    "tools/call `arguments` must be an object",
                ));
            }
        };

        self.registry
            .call(name, &arguments)
            .ok_or_else(|| RpcError::invalid_params(format!("unknown tool: {name}")))
    }
}

/// Serve over the process's stdin/stdout until the client closes the pipe.
pub fn serve_stdio() -> anyhow::Result<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut server = Server::default();
    server.run(stdin.lock(), stdout.lock())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn request(id: u64, method: &str, params: Value) -> String {
        json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string()
    }

    #[test]
    fn handshake_negotiates_a_known_version_and_reports_server_info() {
        let mut server = Server::default();
        let response = server
            .handle_line(&request(
                1,
                "initialize",
                json!({"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}),
            ))
            .unwrap();
        assert_eq!(response["id"], json!(1));
        assert_eq!(response["result"]["protocolVersion"], json!("2025-03-26"));
        assert_eq!(
            response["result"]["serverInfo"]["name"],
            json!("merge-pipeline")
        );
        assert_eq!(
            response["result"]["serverInfo"]["version"],
            json!(crate::VERSION)
        );
        assert!(response["result"]["capabilities"]["tools"].is_object());
        assert_eq!(server.protocol_version(), Some("2025-03-26"));

        let unknown = server
            .handle_line(&request(
                2,
                "initialize",
                json!({"protocolVersion": "1999-01-01"}),
            ))
            .unwrap();
        assert_eq!(unknown["result"]["protocolVersion"], json!("2025-06-18"));
    }

    #[test]
    fn notifications_get_no_reply_and_ping_returns_empty_object() {
        let mut server = Server::default();
        assert!(
            server
                .handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
                .is_none()
        );
        assert!(
            server
                .handle_line(r#"{"jsonrpc":"2.0","method":"tools/list"}"#)
                .is_none(),
            "a notification for a request method is still a notification"
        );
        assert!(server.handle_line("   ").is_none());
        let pong = server.handle_line(&request(7, "ping", json!({}))).unwrap();
        assert_eq!(pong["result"], json!({}));
    }

    #[test]
    fn unknown_method_malformed_line_and_bad_tool_call_map_to_the_right_codes() {
        let mut server = Server::default();
        let missing = server
            .handle_line(&request(1, "resources/list", json!({})))
            .unwrap();
        assert_eq!(missing["error"]["code"], json!(jsonrpc::METHOD_NOT_FOUND));

        let malformed = server.handle_line("{oops").unwrap();
        assert_eq!(malformed["error"]["code"], json!(jsonrpc::PARSE_ERROR));
        assert_eq!(malformed["id"], Value::Null);

        let unknown_tool = server
            .handle_line(&request(2, "tools/call", json!({"name": "nope"})))
            .unwrap();
        assert_eq!(
            unknown_tool["error"]["code"],
            json!(jsonrpc::INVALID_PARAMS)
        );

        let no_name = server
            .handle_line(&request(3, "tools/call", json!({"arguments": {}})))
            .unwrap();
        assert_eq!(no_name["error"]["code"], json!(jsonrpc::INVALID_PARAMS));

        let bad_args = server
            .handle_line(&request(
                4,
                "tools/call",
                json!({"name": "list_workflows", "arguments": []}),
            ))
            .unwrap();
        assert_eq!(bad_args["error"]["code"], json!(jsonrpc::INVALID_PARAMS));
    }

    #[test]
    fn tools_list_over_in_memory_pipes_writes_one_line_per_request() {
        let input = format!(
            "{}\n{}\n{}\n",
            request(1, "initialize", json!({"protocolVersion": "2025-06-18"})),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            request(2, "tools/list", json!({}))
        );
        let mut output = Vec::new();
        Server::default()
            .run(Cursor::new(input), &mut output)
            .unwrap();
        let lines: Vec<Value> = String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(lines.len(), 2, "the notification produced no line");
        assert_eq!(lines[1]["id"], json!(2));
        let names: Vec<&str> = lines[1]["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            vec![
                "list_workflows",
                "inspect_repo",
                "plan_workflow",
                "run_workflow",
                "doctor_config",
                "init_config"
            ]
        );
    }

    #[test]
    fn tool_errors_are_results_with_is_error_not_protocol_errors() {
        let mut server = Server::default();
        let response = server
            .handle_line(&request(
                1,
                "tools/call",
                json!({"name": "plan_workflow", "arguments": {"cwd": "/no/such/dir", "workflow": "x"}}),
            ))
            .unwrap();
        assert!(response.get("error").is_none());
        assert_eq!(response["result"]["isError"], json!(true));
        assert!(
            response["result"]["structuredContent"]["error"]
                .as_str()
                .unwrap()
                .contains("not a directory")
        );
    }
}
