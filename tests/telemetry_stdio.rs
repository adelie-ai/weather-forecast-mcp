#![deny(warnings)]

// Acceptance tests for the telemetry weather-forecast-mcp inherits from
// mcp-core's `run`: the stdio transport keeps stdout clean at any log level,
// and a sentinel location never reaches an INFO line (D10, the level
// contract). Table-driven over the server's whole tool list, for the same
// reason `tests/telemetry_span_fields.rs` is (mcp-core#40 lesson 8): a
// single-tool console test would have the same blind spot fileio-mcp's did.
// `support::tool_cases` carries both a success and a failure row per tool
// (lesson 9), and `requests_for_all_tools` below drives every row, so this
// exercises both branches automatically.
//
// Each test spawns the real binary, pointed at a local mock of both
// Open-Meteo endpoints via WEATHER_GEOCODING_BASE_URL / WEATHER_FORECAST_BASE_URL
// (`tests/support::start_mock_server`), so no test reaches a live service.
// Only a real process proves what reaches file descriptor 1 and what the
// installed subscriber really writes to stderr; an in-process capturing
// layer (`tests/telemetry_span_fields.rs`) only proves what a test told a
// layer to do.

mod support;

use serde_json::{Value, json};
use std::io::Write;
use std::process::{Child, Command, Output, Stdio};

use support::{start_mock_server, tool_cases};

fn spawn_with_log_level(level: &str, mock_base_url: &str) -> Child {
    let exe = env!("CARGO_BIN_EXE_weather-forecast-mcp");
    Command::new(exe)
        .args(["serve", "--mode", "stdio"])
        .env("RUST_LOG", level)
        .env("WEATHER_GEOCODING_BASE_URL", mock_base_url)
        .env("WEATHER_FORECAST_BASE_URL", mock_base_url)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn weather-forecast-mcp serve --mode stdio")
}

fn run_requests(level: &str, mock_base_url: &str, requests: &[Value]) -> Output {
    let mut child = spawn_with_log_level(level, mock_base_url);
    {
        let stdin = child.stdin.as_mut().expect("child has a piped stdin");
        for request in requests {
            writeln!(stdin, "{request}").expect("write jsonrpc line");
        }
    }
    drop(child.stdin.take());
    child.wait_with_output().expect("child must exit")
}

/// One `tools/call` per [`support::tool_cases`] row, plus the handshake and a
/// clean shutdown.
fn requests_for_all_tools() -> Vec<Value> {
    let mut requests = vec![
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}),
    ];
    for (i, case) in tool_cases().into_iter().enumerate() {
        requests.push(json!({
            "jsonrpc": "2.0",
            "id": i + 2,
            "method": "tools/call",
            "params": {"name": case.tool, "arguments": case.arguments},
        }));
    }
    requests.push(json!({"jsonrpc":"2.0","id":99,"method":"shutdown","params":{}}));
    requests
}

/// The level word `tracing_subscriber`'s default console formatter writes as
/// the second whitespace-separated token, right after the timestamp. Reading
/// it this way (rather than a substring search for "INFO") does not confuse
/// a level word for content that happens to contain the same letters.
fn line_level(line: &str) -> Option<&str> {
    line.split_whitespace()
        .nth(1)
        .filter(|token| matches!(*token, "ERROR" | "WARN" | "INFO" | "DEBUG" | "TRACE"))
}

#[test]
fn stdout_carries_only_jsonrpc_at_trace_level() {
    let mock = start_mock_server();
    let requests = requests_for_all_tools();
    let output = run_requests("trace", &mock.base_url(), &requests);
    assert!(
        output.status.success(),
        "weather-forecast-mcp must exit cleanly, otherwise an empty stdout proves nothing: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
    let mut replies = 0;
    for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
        let value: Value = serde_json::from_str(line).unwrap_or_else(|e| {
            panic!("every stdout line must be JSON-RPC, but {line:?} is not: {e}")
        });
        assert_eq!(
            value.get("jsonrpc").and_then(Value::as_str),
            Some("2.0"),
            "every stdout line must carry the JSON-RPC envelope: {line:?}"
        );
        replies += 1;
    }
    // initialize + one reply per tool_cases row + shutdown. `initialized` is
    // a notification and gets no reply.
    let expected_replies = 1 + tool_cases().len() + 1;
    assert_eq!(
        replies, expected_replies,
        "expected one reply per request that carried an id"
    );

    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    assert!(
        stderr.contains("INFO") || stderr.contains("DEBUG") || stderr.contains("TRACE"),
        "at RUST_LOG=trace the subscriber must be installed and log to stderr; stderr was: \
         {stderr:?}"
    );
}

/// AC (mcp-core#40, D10): no sentinel location reaches an INFO (or higher)
/// line, for any registered tool.
#[test]
fn no_sentinel_reaches_an_info_line_for_any_tool() {
    let mock = start_mock_server();
    let requests = requests_for_all_tools();
    let output = run_requests("trace", &mock.base_url(), &requests);
    assert!(
        output.status.success(),
        "weather-forecast-mcp must exit cleanly: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    let cases = tool_cases();

    for case in &cases {
        let mut saw_sentinel_at_debug = false;
        for line in stderr.lines() {
            if !line.contains(&case.sentinel) {
                continue;
            }
            let level = line_level(line);
            assert!(
                matches!(level, Some("DEBUG") | Some("TRACE")),
                "{}'s sentinel reached a line at level {level:?}, at or above INFO: {line:?}",
                case.tool
            );
            if level == Some("DEBUG") {
                saw_sentinel_at_debug = true;
            }
        }
        assert!(
            saw_sentinel_at_debug,
            "{}'s sentinel must still be reachable at DEBUG, or this test cannot tell a real \
             fix from a line that was simply deleted; stderr was: {stderr:?}",
            case.tool
        );
    }
}
