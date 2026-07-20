#![deny(warnings)]
#![recursion_limit = "256"]

// Library crate for weather-forecast-mcp

pub mod error;
pub mod operations;
pub mod service;

pub use service::WeatherService;

use mcp_core::ServerConfig;

/// Build the [`ServerConfig`] this server starts with.
///
/// Kept as a library function (rather than inline in `main`) so the server-level
/// metadata Adele's tool discovery relies on -- notably the `instructions` blurb
/// surfaced in the MCP `initialize` response -- is testable without spawning the
/// binary.
pub fn server_config() -> ServerConfig {
    ServerConfig::new("weather-forecast-mcp", env!("CARGO_PKG_VERSION")).instructions(
        "Weather lookup by place name: current conditions plus daily and hourly \
        forecasts up to 16 days, anywhere in the world, from the free Open-Meteo \
        API (no API key or setup needed). Reach for it whenever someone asks what \
        the weather is like, whether it will rain, how hot or cold it will be, or \
        what to expect for a given day or an upcoming trip. Typical flow: call \
        weather_geocode to turn a place name like 'London' or 'Tokyo' into \
        latitude/longitude coordinates, then pass those to weather_get_current or \
        weather_get_forecast (temperature and wind-speed units are configurable). \
        Note that weather_get_alerts is a placeholder and does not yet return live \
        weather warnings.",
    )
}

/// Construct the weather service with built-in defaults, for in-process (compiled-in) hosting.
pub fn build_service() -> WeatherService {
    WeatherService::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The server must advertise a non-empty `instructions` blurb so the daemon
    /// can use it as the server's searchable description for tool discovery.
    #[test]
    fn test_server_config_has_non_empty_instructions() {
        let config = server_config();
        let instructions = config
            .instructions
            .as_deref()
            .expect("server_config must set instructions");
        assert!(
            !instructions.trim().is_empty(),
            "instructions blurb must not be blank"
        );
    }

    /// The instructions must name the tools and reflect the geocode-first
    /// discovery pattern, so a model can tell what the server offers and how to
    /// drive it from the blurb alone.
    #[test]
    fn test_server_config_instructions_mentions_key_tools_and_pattern() {
        let config = server_config();
        let instructions = config
            .instructions
            .as_deref()
            .expect("server_config must set instructions")
            .to_lowercase();

        for needle in [
            "weather_geocode",
            "weather_get_current",
            "weather_get_forecast",
            "forecast",
            "coordinates",
        ] {
            assert!(
                instructions.contains(needle),
                "instructions should mention '{needle}', got: {instructions}"
            );
        }
    }

    /// Acceptance (da#538): the in-process entry point builds a service that is
    /// wired to the `McpService` trait and advertises the weather tool set, so a
    /// client can compile this server in and get a working, zero-config server.
    #[test]
    fn build_service_exposes_tools() {
        use mcp_core::McpService;
        let svc = build_service();
        assert!(
            !svc.tools().is_empty(),
            "weather build_service() must expose at least one tool"
        );
    }
}
