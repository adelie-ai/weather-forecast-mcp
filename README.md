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

## Testing

```bash
# Run tests locally (skips network-dependent tests)
cargo test

# Run all tests including network integration
RUN_NETWORK_TESTS=1 cargo test

# Run tests in Docker
just test
```

The test suite includes 20+ integration tests covering MCP protocol compliance, parameter validation, and end-to-end weather API calls.

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
