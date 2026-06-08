#![deny(warnings)]

// Binary entry-point for weather-forecast-mcp

use mcp_core::ServerConfig;
use weather_forecast_mcp::service::WeatherService;

#[tokio::main]
async fn main() -> mcp_core::Result<()> {
    let config = ServerConfig::new("weather-forecast-mcp", env!("CARGO_PKG_VERSION"));
    mcp_core::run_simple(config, || async { Ok(WeatherService::new()) }).await
}
