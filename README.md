# weather-forecast-mcp

A Model Context Protocol (MCP) server that provides weather forecast tools for LLM applications. Built in Rust, it exposes geocoding, current conditions, forecasts, and alerts as callable MCP tools over stdio or WebSocket transports.

All weather data comes from the free [Open-Meteo](https://open-meteo.com/) API — no API keys required.

## Tools

| Tool | Description |
|------|-------------|
| `weather_geocode` | Convert location names to geographic coordinates |
| `weather_get_current` | Get current weather conditions for a lat/lon |
| `weather_get_forecast` | Get daily or hourly forecasts (up to 16 days) |
| `weather_get_alerts` | Get weather alerts for a location (placeholder for live provider integration) |

All tools support configurable temperature units (celsius/fahrenheit) and wind speed units (km/h, m/s, mph, knots).

## Building

```bash
cargo build --release
```

## Usage

### Stdio transport (recommended for local/IDE use)

```bash
weather-forecast-mcp serve --mode stdio
```

The server reads JSON-RPC messages from stdin and writes responses to stdout. It auto-detects newline-delimited JSON or Content-Length framing.

### WebSocket transport

```bash
weather-forecast-mcp serve --mode websocket --host 0.0.0.0 --port 8080
```

Connects via WebSocket at `ws://<host>:<port>/ws`.

### Claude Desktop configuration

Add to your Claude Desktop MCP config:

```json
{
  "mcpServers": {
    "weather": {
      "command": "/path/to/weather-forecast-mcp",
      "args": ["serve", "--mode", "stdio"]
    }
  }
}
```

### VS Code configuration

Add to your VS Code MCP settings (`.vscode/mcp.json`):

```json
{
  "servers": {
    "weather": {
      "command": "/path/to/weather-forecast-mcp",
      "args": ["serve", "--mode", "stdio"]
    }
  }
}
```

## Logging

`mcp-core`'s `run` installs the process subscriber; this crate calls nothing to get it.
Logs go to stderr, never stdout -- the stdio transport frames JSON-RPC on stdout, and one
log line there would corrupt the protocol stream. `RUST_LOG` sets the level (default
`info`); see `mcp-core`'s own README for the full level contract, the request/tool-call
spans, and the standard `OTEL_*` environment variables.

A location -- a place name or a pair of coordinates -- is what every tool argument here
carries, and it is content: it says where a person is or cares about. It stays at DEBUG
only, never INFO, never a span field, whether it names a real place or is coarsened to a
city or a region.

What this server adds on top of what it inherits:

- A `debug!` line before each outbound Open-Meteo request (`weather_geocode`'s search, and
  the shared `/v1/forecast` call `weather_get_current` and `weather_get_forecast` both
  make), carrying the location. `RUST_LOG=debug` is what it takes to see it.
- `weather.upstream_failure`, a counter labelled `tool` and a bounded `reason`
  (`network`, `api_error`, or `malformed_response`), for a fault reaching outward. A "no
  match" answer from the geocoder and a coordinate or unit rejected by local validation are
  declines, not faults, and are not counted here.
- `mcp-core` already records a tool-call counter and a latency histogram by tool and
  outcome (`mcp.tools.call`, `mcp.tools.call.duration`); this server does not duplicate
  them.

### The `otel` feature

Off by default. A pure passthrough -- `weather-forecast-mcp -> mcp-core ->
adelie-telemetry` -- so this crate takes no direct dependency on `adelie-telemetry` or on
any opentelemetry crate. With the feature off, `cargo tree` resolves no opentelemetry
crate at all.

```bash
cargo build --features otel
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318 ./target/debug/weather-forecast-mcp serve --mode stdio
```

### Pointing at a different upstream

`--geocoding-base-url` / `WEATHER_GEOCODING_BASE_URL` and `--forecast-base-url` /
`WEATHER_FORECAST_BASE_URL` override the two Open-Meteo hosts (default
`https://geocoding-api.open-meteo.com` and `https://api.open-meteo.com`). Open-Meteo is
the only backend this server speaks, so a normal deployment never sets these; they exist
so a test can point a compiled binary at a local mock server instead of a live service.

## Testing

```bash
just check                       # default features: fmt, lint, build, test
just check-otel                  # the same, built with --features otel
just check-all                   # both -- what the pre-push hook runs
just test-integration            # additionally launch the live Open-Meteo API
```

Network-dependent integration tests in `tests/mcp_stdio_suite.rs` are gated behind
`RUN_NETWORK_TESTS=1` so the default suite is deterministic and offline. The
`tests/telemetry_*.rs` files are the telemetry acceptance suite: that a default build
resolves no opentelemetry crate, that stdout carries only JSON-RPC at `RUST_LOG=trace`,
and that no location -- from any registered tool, on both its success and its failure
path -- reaches an INFO line or a span field. Those tests drive a local mock of both
Open-Meteo endpoints (`tests/support::start_mock_server`), never the real API.

## Project structure

```
src/
├── main.rs            Entry point, CLI, JSON-RPC routing, WebSocket transport
├── server.rs          MCP server state and protocol handling
├── tools.rs           Tool registry and execution dispatcher
├── transport.rs       Stdio transport with auto-detected framing
├── error.rs           Structured error types
└── operations/
    ├── current.rs     Current weather conditions
    ├── forecast.rs    Daily/hourly forecasts
    ├── geocode.rs     Location geocoding
    └── alerts.rs      Weather alerts (placeholder)
```

## License

Apache-2.0
