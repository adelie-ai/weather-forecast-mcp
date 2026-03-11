#![deny(warnings)]

// Geocoding: resolve location name to latitude/longitude using Open-Meteo geocoding API

use crate::error::{Result, WeatherError};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct GeocodingResponse {
    results: Option<Vec<GeocodingResult>>,
}

#[derive(Debug, Deserialize)]
struct GeocodingResult {
    name: String,
    latitude: f64,
    longitude: f64,
    country: Option<String>,
    admin1: Option<String>,
    elevation: Option<f64>,
}

/// Geocode a location name and return matching results.
pub async fn geocode_location(
    client: &reqwest::Client,
    name: &str,
    count: u32,
    language: Option<&str>,
) -> Result<Value> {
    let count = count.clamp(1, 10);
    let language = language.unwrap_or("en");

    let url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count={}&language={}&format=json",
        urlencoding(name),
        count,
        language
    );

    let resp = client
        .get(&url)
        .send()
        .await?
        .json::<GeocodingResponse>()
        .await?;

    let results = resp.results.unwrap_or_default();
    if results.is_empty() {
        return Err(WeatherError::LocationNotFound(format!(
            "No locations found for: {}",
            name
        ))
        .into());
    }

    let locations: Vec<Value> = results
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "name": r.name,
                "latitude": r.latitude,
                "longitude": r.longitude,
                "country": r.country,
                "region": r.admin1,
                "elevation_m": r.elevation,
            })
        })
        .collect();

    Ok(serde_json::json!(locations))
}

fn urlencoding(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' {
                vec![c]
            } else {
                format!("%{:02X}", c as u32).chars().collect()
            }
        })
        .collect()
}
