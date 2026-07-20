#![deny(warnings)]

// Binary entry-point for weather-forecast-mcp

use weather_forecast_mcp::{build_service, server_config};

#[tokio::main]
async fn main() -> mcp_core::Result<()> {
    mcp_core::run_simple(server_config(), || async { Ok(build_service()) }).await
}
