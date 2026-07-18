#![deny(warnings)]

// Binary entry-point for weather-forecast-mcp

use weather_forecast_mcp::server_config;
use weather_forecast_mcp::service::WeatherService;

#[tokio::main]
async fn main() -> mcp_core::Result<()> {
    mcp_core::run_simple(server_config(), || async { Ok(WeatherService::new()) }).await
}
