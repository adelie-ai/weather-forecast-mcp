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
///
/// If the full query returns no results and contains a comma or whitespace-separated
/// qualifier (e.g. "Houston, Texas" or "Houston TX"), retries with just the first
/// part as a fallback, since Open-Meteo works best with simple city names.
pub async fn geocode_location(
    client: &reqwest::Client,
    name: &str,
    count: u32,
    language: Option<&str>,
) -> Result<Value> {
    let count = count.clamp(1, 10);
    let language = language.unwrap_or("en");

    match geocode_query(client, name, count, language).await {
        Ok(locations) => Ok(locations),
        Err(_) => {
            // Try simplified name: strip everything after a comma, or take the first
            // multi-word token group before a state/country qualifier.
            if let Some(simplified) = simplify_location_name(name) {
                geocode_query(client, &simplified, count, language).await
            } else {
                Err(WeatherError::LocationNotFound(format!(
                    "No locations found for: {}",
                    name
                ))
                .into())
            }
        }
    }
}

/// Execute a single geocoding query against the Open-Meteo API.
async fn geocode_query(
    client: &reqwest::Client,
    name: &str,
    count: u32,
    language: &str,
) -> Result<Value> {
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

/// Try to extract a simpler location name from a qualified string.
/// Returns `None` if the name is already simple (no simplification possible).
fn simplify_location_name(name: &str) -> Option<String> {
    // "Houston, Texas" or "London, UK" -> "Houston" or "London"
    if let Some(before_comma) = name.split(',').next() {
        let trimmed = before_comma.trim();
        if !trimmed.is_empty() && trimmed != name.trim() {
            return Some(trimmed.to_string());
        }
    }

    // "Houston TX" or "Paris FR" -> "Houston" or "Paris"
    // Take everything except the last token if it looks like a 2-3 letter qualifier
    let parts: Vec<&str> = name.split_whitespace().collect();
    if parts.len() >= 2 {
        let last = parts.last().unwrap();
        if last.len() <= 3 && last.chars().all(|c| c.is_ascii_alphabetic()) {
            let simplified = parts[..parts.len() - 1].join(" ");
            if !simplified.is_empty() {
                return Some(simplified);
            }
        }
    }

    None
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
