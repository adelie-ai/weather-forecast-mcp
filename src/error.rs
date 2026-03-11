#![deny(warnings)]

// Error types for the weather-forecast-mcp crate

use thiserror::Error;

/// Main error type for the weather-forecast-mcp application
#[derive(Error, Debug)]
pub enum WeatherForecastMcpError {
    /// Weather operation errors
    #[error("Weather error: {0}")]
    Weather(#[from] WeatherError),

    /// JSON serialization/deserialization errors
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// MCP protocol errors
    #[error("MCP protocol error: {0}")]
    Mcp(#[from] McpError),

    /// Transport layer errors
    #[error("Transport error: {0}")]
    Transport(#[from] TransportError),

    /// IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// HTTP errors
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}

/// Weather operation errors
#[derive(Error, Debug)]
pub enum WeatherError {
    /// Location not found
    #[error("Location not found: {0}")]
    LocationNotFound(String),

    /// API error
    #[error("API error: {0}")]
    ApiError(String),

    /// Invalid coordinates
    #[error("Invalid coordinates: {0}")]
    InvalidCoordinates(String),

    /// Invalid parameters
    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),

    /// Forecast unavailable
    #[error("Forecast unavailable: {0}")]
    ForecastUnavailable(String),
}

/// MCP protocol errors
#[derive(Error, Debug)]
pub enum McpError {
    /// Invalid protocol version
    #[error("Unsupported protocol version: {0}")]
    InvalidProtocolVersion(String),

    /// Invalid JSON-RPC message
    #[error("Invalid JSON-RPC message: {0}")]
    InvalidJsonRpc(String),

    /// Tool not found
    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    /// Invalid tool parameters
    #[error("Invalid tool parameters: {0}")]
    InvalidToolParameters(String),
}

/// Transport layer errors
#[derive(Error, Debug)]
pub enum TransportError {
    /// WebSocket connection error
    #[error("WebSocket connection error: {0}")]
    WebSocket(String),

    /// Invalid message format
    #[error("Invalid message format: {0}")]
    InvalidMessage(String),

    /// Connection closed
    #[error("Connection closed")]
    ConnectionClosed,

    /// IO error in transport
    #[error("Transport IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, WeatherForecastMcpError>;
