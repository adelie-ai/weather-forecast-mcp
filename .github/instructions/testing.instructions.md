# Testing (weather-forecast-mcp)

This repo uses Rust MCP integration tests that spawn the server in stdio mode and exercise the MCP tools end-to-end.

## Source of truth

- Test runner: `cargo test`
- Source of truth suite: `tests/mcp_stdio_suite.rs`
- Test workspace: temporary directory created at runtime (or in-memory for pure logic tests)

## MCP stdio integration tests

The integration suite in `tests/mcp_stdio_suite.rs` spawns `weather-forecast-mcp serve --mode stdio`, performs MCP initialization, and exercises tools end-to-end over JSON-RPC.

The harness is intentionally **one tool call per test**:
1) Rust sets up any preconditions
2) Exactly one MCP `tools/call` is performed
3) Rust validates the return shape and/or expected values

## Important: network access

The weather tools hit live Open-Meteo APIs. Integration tests that make network calls:
- Will fail without internet access
- May be flaky depending on API availability

For CI without internet access, mark network tests with `#[ignore]` and run with `cargo test -- --include-ignored` only when connectivity is available.

## Running tests

Preferred (Docker):

```bash
just test
```

Local:

```bash
cargo test
```

Environment toggles:

- `RUN_NETWORK_TESTS=1` — enables tests that make live HTTP calls to Open-Meteo APIs.

## Expected output

- Standard `cargo test` output
- Exits with code `0` on success, `101` if any tests fail

## Troubleshooting

- If you see JSON-RPC "Server not initialized", the server did not receive `initialize`/`initialized`.
- If network tests fail, check connectivity to `api.open-meteo.com` and `geocoding-api.open-meteo.com`.
- If you see tool parameter shape errors, prefer matching the current Rust implementation (schemas may be more permissive than the actual parser).
