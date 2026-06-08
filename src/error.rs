#![deny(warnings)]

// Domain error types for weather operations

use thiserror::Error;

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

    /// HTTP errors
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}

/// Result type alias for weather operations
pub type Result<T> = std::result::Result<T, WeatherError>;
