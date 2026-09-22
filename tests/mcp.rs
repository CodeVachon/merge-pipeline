//! The MCP server driven end to end: spawn the binary with `mcp`, speak JSON-RPC over its pipes.
//! Additions: the baseline had no MCP surface.

mod common;

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use common::Fixture;
use serde_json::{Value, json};

fn write_configs(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(
        dir.join("patch.json"),
        r#"{"name":"Patch to Staging","order":1,"pipeline":["^Patch-v0.1.1$","^staging-patch$"]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("any-patch.json"),
        r#"{"name":"Any Patch to Staging","order":2,"pipeline":["^Patch-*","^staging-patch$"]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("default.json"),
        r#"{"disabled":true,"name":"Default Pipeline","order":100,"pipeline":["staging","main"]}"#,
    )
    .unwrap();
    std::fs::write(dir.join("broken.json"), "{not json").unwrap();
}

struct McpClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl McpClient {
    fn spawn(fixture: &Fixture) -> Self {
        let mut command = Command::new(assert_cmd::cargo::cargo_bin("merge-pipeline"));
        command
            .arg("mcp")
            .env_remove("MERGE_PIPELINE_CONFIG")
            .env("NO_COLOR", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        for (key, value) in common::isolation_env(&fixture.hooks) {
            command.env(key, value);
        }
        let mut child = command.spawn().expect("server starts");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            stdin,
            stdout,
            next_id: 1,
        }
    }

    fn send(&mut self, message: Value) {
        writeln!(self.stdin, "{message}").unwrap();
        self.stdin.flush().unwrap();
    }

    fn read(&mut self) -> Value {
        let mut line = String::new();
        let read = self.stdout.read_line(&mut line).expect("read response");
        assert!(read > 0, "server closed stdout unexpectedly");
        serde_json::from_str(&line).unwrap_or_else(|_| panic!("not JSON: {line}"))
    }

    fn call(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let response = self.read();
        assert_eq!(response["jsonrpc"], json!("2.0"));
        assert_eq!(response["id"], json!(id), "response id echoes request id");
        response
    }

    fn notify(&mut self, method: &str) {
        self.send(json!({"jsonrpc": "2.0", "method": method}));
    }

    fn handshake(&mut self) -> Value {
        let response = self.call(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "test", "version": "0"}
            }),
        );
        self.notify("notifications/initialized");
        response["result"].clone()
    }

    /// `tools/call` and return the tool's structured payload plus its isError flag.
    fn tool(&mut self, name: &str, arguments: Value) -> (Value, bool) {
        let response = self.call("tools/call", json!({"name": name, "arguments": arguments}));
        assert!(
            response.get("error").is_none(),
            "tool errors must be results, got {response}"
        );
        let result = &response["result"];
        assert_eq!(result["content"][0]["type"], json!("text"));
        let text: Value = serde_json::from_str(result["content"][0]["text"].as_str().unwrap())
            .expect("text content is JSON");
        assert_eq!(&text, &result["structuredContent"]);
        (
            result["structuredContent"].clone(),
            result["isError"].as_bool().unwrap(),
        )
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn setup() -> (Fixture, std::path::PathBuf) {
    let fixture = Fixture::new();
    let config = fixture.temp.path().join("config");
    write_configs(&config);
    (fixture, config)
}

fn common_args(fixture: &Fixture, config: &Path) -> Value {
    json!({
        "cwd": fixture.work.to_string_lossy(),
        "config": config.to_string_lossy(),
    })
}

fn with(mut base: Value, extra: Value) -> Value {
    base.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    base
}

#[test]
fn handshake_tools_list_and_list_workflows() {
    let (fixture, config) = setup();
    let mut client = McpClient::spawn(&fixture);

    let init = client.handshake();
    assert_eq!(init["protocolVersion"], json!("2025-06-18"));
    assert_eq!(init["serverInfo"]["name"], json!("merge-pipeline"));
    assert_eq!(
        init["serverInfo"]["version"],
        json!(env!("CARGO_PKG_VERSION"))
    );
    assert!(init["capabilities"]["tools"].is_object());

    let pong = client.call("ping", json!({}));
    assert_eq!(pong["result"], json!({}));

    let listed = client.call("tools/list", json!({}));
    let names: Vec<&str> = listed["result"]["tools"]
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
            "run_workflow"
        ]
    );
    let run_schema = &listed["result"]["tools"][3]["inputSchema"];
    assert_eq!(run_schema["required"], json!(["workflow", "confirm"]));
    assert_eq!(
        run_schema["properties"]["version_strategy"]["enum"],
        json!(["higher", "lower", "fail"])
    );

    let (payload, is_error) = client.tool("list_workflows", common_args(&fixture, &config));
    assert!(!is_error);
    assert_eq!(payload["root"], json!(config.to_string_lossy()));
    assert_eq!(
        payload["enabled"],
        json!(["Patch to Staging", "Any Patch to Staging"])
    );
    let workflows = payload["workflows"].as_array().unwrap();
    assert_eq!(workflows.len(), 3, "disabled workflows are listed too");
    assert_eq!(workflows[2]["disabled"], json!(true));
    assert_eq!(
        workflows[0]["pipeline"],
        json!(["^Patch-v0.1.1$", "^staging-patch$"])
    );
    assert!(
        workflows[0]["source"]
            .as_str()
            .unwrap()
            .ends_with("patch.json")
    );
    let errors = payload["errors"].as_array().unwrap();
    assert_eq!(errors.len(), 1);
    assert!(errors[0]["path"].as_str().unwrap().ends_with("broken.json"));

    let unknown = client.call("resources/list", json!({}));
    assert_eq!(unknown["error"]["code"], json!(-32601));
}

#[test]
fn inspect_repo_reports_cleanliness_branch_and_upstreams() {
    let (fixture, config) = setup();
    let mut client = McpClient::spawn(&fixture);
    client.handshake();

    let (payload, is_error) = client.tool("inspect_repo", common_args(&fixture, &config));
    assert!(!is_error);
    assert_eq!(payload["clean"], json!(true));
    assert_eq!(payload["current_branch"], json!("main"));
    assert_eq!(payload["fetched"], json!(false));
    let branches = payload["branches"].as_array().unwrap();
    let upstream = |name: &str| {
        branches
            .iter()
            .find(|branch| branch["name"] == json!(name))
            .unwrap_or_else(|| panic!("{name} listed"))["has_upstream"]
            .as_bool()
            .unwrap()
    };
    assert!(upstream("main"));
    assert!(upstream("staging-patch"));
    assert!(!upstream("Patch-v0.1.2"));
    assert!(!upstream("banana"));

    fixture.write("README.md", "dirty\n");
    let (payload, _) = client.tool(
        "inspect_repo",
        with(common_args(&fixture, &config), json!({"fetch": true})),
    );
    assert_eq!(payload["clean"], json!(false));
    assert_eq!(payload["fetched"], json!(true));
}

#[test]
fn plan_workflow_reports_ambiguity_then_resolves_with_selections() {
    let (fixture, config) = setup();
    let mut client = McpClient::spawn(&fixture);
    client.handshake();

    let (payload, is_error) = client.tool(
        "plan_workflow",
        with(
            common_args(&fixture, &config),
            json!({"workflow": "Any Patch to Staging"}),
        ),
    );
    assert!(!is_error, "ambiguity is a report, not an error: {payload}");
    assert_eq!(payload["complete"], json!(false));
    assert_eq!(
        payload["ambiguous"],
        json!([{"pattern": "^Patch-*", "candidates": ["Patch-v0.1.1", "Patch-v0.1.2"]}])
    );
    assert_eq!(payload["no_match"], json!([]));
    assert!(payload.get("steps").is_none());

    let (payload, is_error) = client.tool(
        "plan_workflow",
        with(
            common_args(&fixture, &config),
            json!({
                "workflow": "Any Patch to Staging",
                "selections": {"^Patch-*": "Patch-v0.1.2"},
                "fetch": false
            }),
        ),
    );
    assert!(!is_error);
    assert_eq!(payload["complete"], json!(true));
    assert_eq!(
        payload["branches"],
        json!(["Patch-v0.1.2", "staging-patch"])
    );
    assert_eq!(
        payload["steps"],
        json!([{"source": "Patch-v0.1.2", "target": "staging-patch"}])
    );
    assert_eq!(payload["workflow"], json!("Any Patch to Staging"));

    let (payload, is_error) = client.tool(
        "plan_workflow",
        with(common_args(&fixture, &config), json!({"workflow": "Nope"})),
    );
    assert!(is_error);
    assert!(
        payload["error"]
            .as_str()
            .unwrap()
            .contains("Could not find Workflow named \"Nope\"")
    );
    assert!(
        payload["error"]
            .as_str()
            .unwrap()
            .contains("\"Patch to Staging\"")
    );
}

#[test]
fn run_workflow_requires_confirm_and_a_clean_tree() {
    let (fixture, config) = setup();
    let mut client = McpClient::spawn(&fixture);
    client.handshake();

    let (payload, is_error) = client.tool(
        "run_workflow",
        with(
            common_args(&fixture, &config),
            json!({"workflow": "Patch to Staging"}),
        ),
    );
    assert!(is_error);
    assert!(payload["error"].as_str().unwrap().contains("confirm=true"));

    fixture.write("README.md", "dirty\n");
    let (payload, is_error) = client.tool(
        "run_workflow",
        with(
            common_args(&fixture, &config),
            json!({"workflow": "Patch to Staging", "confirm": true}),
        ),
    );
    assert!(is_error);
    assert_eq!(payload["error"], json!("Git is in a Dirty State"));
}

#[test]
fn run_workflow_merges_and_pushes_when_confirmed() {
    let (fixture, config) = setup();
    fixture.commit_on("Patch-v0.1.1", "patch.txt", "patched\n", "patch change");
    fixture.raw(&fixture.work, &["checkout", "main"]);
    let before = fixture.origin_head("staging-patch");

    let mut client = McpClient::spawn(&fixture);
    client.handshake();
    let (payload, is_error) = client.tool(
        "run_workflow",
        with(
            common_args(&fixture, &config),
            json!({"workflow": "Patch to Staging", "confirm": true, "auto_push": true}),
        ),
    );
    assert!(!is_error, "{payload}");
    assert_eq!(
        payload["branches"],
        json!(["Patch-v0.1.1", "staging-patch"])
    );
    assert_eq!(payload["steps"].as_array().unwrap().len(), 1);
    assert_eq!(payload["steps"][0]["merged"], json!(true));
    assert_eq!(payload["steps"][0]["pushed"], json!(true));
    assert_eq!(payload["starting_branch"], json!("main"));
    assert_eq!(payload["current_branch"], json!("staging-patch"));
    let types: Vec<&str> = payload["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["type"].as_str().unwrap())
        .collect();
    assert!(types.contains(&"merged"));
    assert!(types.contains(&"pushed"));

    let after = fixture.origin_head("staging-patch");
    assert_ne!(before, after, "origin received the merge");
    assert_eq!(after, fixture.local_head("staging-patch"));
    assert!(fixture.read("patch.txt").contains("patched"));
}

#[test]
fn run_workflow_without_auto_push_merges_locally_only() {
    let (fixture, config) = setup();
    fixture.commit_on("Patch-v0.1.1", "patch.txt", "patched\n", "patch change");
    fixture.raw(&fixture.work, &["checkout", "main"]);
    let before = fixture.origin_head("staging-patch");

    let mut client = McpClient::spawn(&fixture);
    client.handshake();
    let (payload, is_error) = client.tool(
        "run_workflow",
        with(
            common_args(&fixture, &config),
            json!({"workflow": "Patch to Staging", "confirm": true}),
        ),
    );
    assert!(!is_error, "{payload}");
    assert_eq!(payload["steps"][0]["merged"], json!(true));
    assert_eq!(payload["steps"][0]["pushed"], json!(false));
    assert_eq!(fixture.origin_head("staging-patch"), before);
    assert_ne!(fixture.local_head("staging-patch"), before);
}

#[test]
fn run_workflow_returns_the_open_question_when_a_pattern_is_ambiguous() {
    let (fixture, config) = setup();
    let mut client = McpClient::spawn(&fixture);
    client.handshake();

    let (payload, is_error) = client.tool(
        "run_workflow",
        with(
            common_args(&fixture, &config),
            json!({"workflow": "Any Patch to Staging", "confirm": true}),
        ),
    );
    assert!(is_error);
    assert_eq!(payload["question"]["key"], json!("branch:^Patch-*"));
    assert_eq!(
        payload["question"]["choices"],
        json!(["Patch-v0.1.1", "Patch-v0.1.2"])
    );
    assert!(payload["error"].as_str().unwrap().contains("selections"));
    assert_eq!(fixture.current_branch(), "main", "nothing was touched");

    let (payload, is_error) = client.tool(
        "run_workflow",
        with(
            common_args(&fixture, &config),
            json!({
                "workflow": "Any Patch to Staging",
                "confirm": true,
                "selections": {"^Patch-*": "Patch-v0.1.2"}
            }),
        ),
    );
    assert!(!is_error, "{payload}");
    assert_eq!(
        payload["branches"],
        json!(["Patch-v0.1.2", "staging-patch"])
    );
}

#[test]
fn run_workflow_reports_conflicts_and_aborts_the_merge_by_default() {
    let (fixture, config) = setup();
    fixture.commit_on(
        "Patch-v0.1.1",
        "README.md",
        "# patch side\n",
        "patch readme",
    );
    fixture.commit_on(
        "staging-patch",
        "README.md",
        "# staging side\n",
        "staging readme",
    );
    fixture.raw(&fixture.work, &["push", "origin", "staging-patch"]);
    fixture.raw(&fixture.work, &["checkout", "main"]);

    let mut client = McpClient::spawn(&fixture);
    client.handshake();
    let (payload, is_error) = client.tool(
        "run_workflow",
        with(
            common_args(&fixture, &config),
            json!({"workflow": "Patch to Staging", "confirm": true}),
        ),
    );
    assert!(is_error);
    assert!(
        payload["error"]
            .as_str()
            .unwrap()
            .starts_with("Merge Error: Patch-v0.1.1 into staging-patch.")
    );
    assert_eq!(payload["conflicted_paths"], json!(["README.md"]));
    assert_eq!(payload["merge_aborted"], json!(true));
    assert_eq!(payload["merge_in_progress"], json!(false));
    assert_eq!(
        payload["step"],
        json!({"source": "Patch-v0.1.1", "target": "staging-patch"})
    );
    let types: Vec<&str> = payload["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["type"].as_str().unwrap())
        .collect();
    assert!(types.contains(&"conflicts_detected"));

    let status = fixture.raw(&fixture.work, &["status", "--porcelain"]);
    assert!(
        status.is_empty(),
        "tree is clean after abort, got {status:?}"
    );
}

#[test]
fn run_workflow_auto_resolves_package_json_versions_with_higher_strategy() {
    let (fixture, config) = setup();
    fixture.commit_on(
        "staging-patch",
        "package.json",
        "{\n  \"name\": \"example\",\n  \"version\": \"1.0.0\"\n}\n",
        "base package",
    );
    fixture.raw(&fixture.work, &["push", "origin", "staging-patch"]);
    fixture.raw(&fixture.work, &["checkout", "Patch-v0.1.1"]);
    fixture.raw(&fixture.work, &["merge", "staging-patch"]);
    fixture.commit_on(
        "Patch-v0.1.1",
        "package.json",
        "{\n  \"name\": \"example\",\n  \"version\": \"1.2.0\"\n}\n",
        "patch bumps",
    );
    fixture.commit_on(
        "staging-patch",
        "package.json",
        "{\n  \"name\": \"example\",\n  \"version\": \"1.1.0\"\n}\n",
        "staging bumps",
    );
    fixture.raw(&fixture.work, &["push", "origin", "staging-patch"]);
    fixture.raw(&fixture.work, &["checkout", "main"]);

    let mut client = McpClient::spawn(&fixture);
    client.handshake();

    let (payload, is_error) = client.tool(
        "run_workflow",
        with(
            common_args(&fixture, &config),
            json!({"workflow": "Patch to Staging", "confirm": true, "version_strategy": "fail"}),
        ),
    );
    assert!(is_error, "fail strategy leaves the conflict: {payload}");
    assert_eq!(payload["conflicted_paths"], json!(["package.json"]));
    assert_eq!(payload["merge_aborted"], json!(true));

    let (payload, is_error) = client.tool(
        "run_workflow",
        with(
            common_args(&fixture, &config),
            json!({"workflow": "Patch to Staging", "confirm": true, "version_strategy": "higher"}),
        ),
    );
    assert!(!is_error, "{payload}");
    assert_eq!(payload["steps"][0]["merged"], json!(true));
    assert_eq!(
        payload["steps"][0]["conflicts_resolved"],
        json!(["package.json"])
    );
    assert!(
        fixture
            .read("package.json")
            .contains("\"version\": \"1.2.0\"")
    );
    assert!(!fixture.read("package.json").contains("<<<<<<<"));
}

#[test]
fn install_wires_project_and_user_configuration_files() {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(
        project.join(".mcp.json"),
        r#"{"mcpServers": {"motte": {"command": "motte", "args": ["mcp"]}}}"#,
    )
    .unwrap();

    let output = Command::new(assert_cmd::cargo::cargo_bin("merge-pipeline"))
        .arg("install")
        .current_dir(&project)
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("added merge-pipeline to"), "{stdout}");
    let written: Value =
        serde_json::from_str(&std::fs::read_to_string(project.join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(written["mcpServers"]["motte"]["command"], json!("motte"));
    assert_eq!(
        written["mcpServers"]["merge-pipeline"],
        json!({"command": "merge-pipeline", "args": ["mcp"]})
    );

    let home = temp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let output = Command::new(assert_cmd::cargo::cargo_bin("merge-pipeline"))
        .args(["install", "--scope", "user"])
        .env("HOME", &home)
        .current_dir(&project)
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("created"), "{stdout}");
    let written: Value =
        serde_json::from_str(&std::fs::read_to_string(home.join(".claude.json")).unwrap()).unwrap();
    assert_eq!(
        written["mcpServers"]["merge-pipeline"]["args"],
        json!(["mcp"])
    );
}
