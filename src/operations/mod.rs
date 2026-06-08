#![deny(warnings)]

// Weather forecast operation implementations

pub mod alerts;
pub mod current;
pub mod forecast;
pub mod geocode;

use crate::error::{Result, WeatherError};

/// Validate that latitude is in [-90, 90] and longitude is in [-180, 180].
pub fn validate_coordinates(latitude: f64, longitude: f64) -> Result<()> {
    if !(-90.0..=90.0).contains(&latitude) {
        return Err(WeatherError::InvalidCoordinates(format!(
            "Latitude {} is out of range [-90, 90]",
            latitude
        ))
        .into());
    }
    if !(-180.0..=180.0).contains(&longitude) {
        return Err(WeatherError::InvalidCoordinates(format!(
            "Longitude {} is out of range [-180, 180]",
            longitude
        ))
        .into());
    }
    Ok(())
}

/// Validate that the temperature unit is one of the supported values.
pub fn validate_temperature_unit(unit: &str) -> Result<()> {
    match unit {
        "celsius" | "fahrenheit" => Ok(()),
        _ => Err(WeatherError::InvalidParameters(format!(
            "Invalid temperature_unit '{}'. Use 'celsius' or 'fahrenheit'.",
            unit
        ))
        .into()),
    }
}

/// Validate that the wind speed unit is one of the supported values.
pub fn validate_wind_speed_unit(unit: &str) -> Result<()> {
    match unit {
        "kmh" | "ms" | "mph" | "kn" => Ok(()),
        _ => Err(WeatherError::InvalidParameters(format!(
            "Invalid wind_speed_unit '{}'. Use 'kmh', 'ms', 'mph', or 'kn'.",
            unit
        ))
        .into()),
    }
}
