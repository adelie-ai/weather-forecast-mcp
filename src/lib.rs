#![deny(warnings)]
#![recursion_limit = "256"]

// Library crate for weather-forecast-mcp

pub mod error;
pub mod operations;
pub mod service;

use mcp_core::ServerConfig;

/// Build the [`ServerConfig`] this server starts with.
///
/// Kept as a library function (rather than inline in `main`) so the server-level
/// metadata Adele's tool discovery relies on -- notably the `instructions` blurb
/// surfaced in the MCP `initialize` response -- is testable without spawning the
/// binary.
pub fn server_config() -> ServerConfig {
    ServerConfig::new("weather-forecast-mcp", env!("CARGO_PKG_VERSION"))
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
}
