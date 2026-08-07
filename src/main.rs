#![deny(warnings)]

// Binary entry-point for weather-forecast-mcp

use weather_forecast_mcp::{DEFAULT_FORECAST_BASE_URL, DEFAULT_GEOCODING_BASE_URL, WeatherService};

/// Server-specific `serve` flags, flattened alongside mcp-core's own
/// [`mcp_core::CommonServeArgs`].
///
/// Overriding the upstream hosts is not a normal deployment need -- Open-Meteo
/// is the only backend this server speaks. It exists so a test can point a
/// real, compiled binary at a local mock server instead of a live service
/// (rule 1.5), which `tests/telemetry_stdio.rs` needs for every tool.
#[derive(clap::Args)]
struct Local {
    /// Base URL (host only, no path) for the Open-Meteo geocoding API.
    #[arg(long, env = "WEATHER_GEOCODING_BASE_URL")]
    geocoding_base_url: Option<String>,
    /// Base URL (host only, no path) for the Open-Meteo forecast API.
    #[arg(long, env = "WEATHER_FORECAST_BASE_URL")]
    forecast_base_url: Option<String>,
}

#[tokio::main]
async fn main() -> mcp_core::Result<()> {
    mcp_core::run::<Local, WeatherService, _, _>(
        weather_forecast_mcp::server_config(),
        |local| async move {
            Ok(WeatherService::with_base_urls(
                local
                    .geocoding_base_url
                    .unwrap_or_else(|| DEFAULT_GEOCODING_BASE_URL.to_string()),
                local
                    .forecast_base_url
                    .unwrap_or_else(|| DEFAULT_FORECAST_BASE_URL.to_string()),
            ))
        },
    )
    .await
}
