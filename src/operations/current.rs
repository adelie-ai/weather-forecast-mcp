#![deny(warnings)]

// Current weather conditions using Open-Meteo API

use crate::error::{Result, WeatherError};
use crate::operations::{
    validate_coordinates, validate_temperature_unit, validate_wind_speed_unit,
};
use serde_json::Value;

/// Fetch current weather conditions for given coordinates.
///
/// `base_url` is the forecast API's host (no path); production callers pass
/// [`crate::DEFAULT_FORECAST_BASE_URL`], and a test can point it at a local
/// mock server instead.
pub async fn get_current_weather(
    client: &reqwest::Client,
    base_url: &str,
    latitude: f64,
    longitude: f64,
    temperature_unit: Option<&str>,
    wind_speed_unit: Option<&str>,
) -> Result<Value> {
    validate_coordinates(latitude, longitude)?;

    let temp_unit = temperature_unit.unwrap_or("celsius");
    let wind_unit = wind_speed_unit.unwrap_or("kmh");

    validate_temperature_unit(temp_unit)?;
    validate_wind_speed_unit(wind_unit)?;

    let url = format!(
        "{}/v1/forecast\
        ?latitude={}&longitude={}\
        &current=temperature_2m,relative_humidity_2m,apparent_temperature,\
        is_day,precipitation,rain,showers,snowfall,weather_code,\
        cloud_cover,pressure_msl,surface_pressure,wind_speed_10m,\
        wind_direction_10m,wind_gusts_10m\
        &temperature_unit={}&wind_speed_unit={}&timezone=auto",
        base_url, latitude, longitude, temp_unit, wind_unit
    );

    log_current_request(&url);

    let resp: Value = client.get(&url).send().await?.json().await?;

    if let Some(err) = resp.get("error") {
        let reason = resp
            .get("reason")
            .and_then(|r| r.as_str())
            .unwrap_or("unknown error");
        return Err(WeatherError::ApiError(format!(
            "{}: {}",
            err.as_bool().unwrap_or(true),
            reason
        )));
    }

    let current = resp
        .get("current")
        .ok_or_else(|| WeatherError::ForecastUnavailable("No current data in response".into()))?;

    let units = resp.get("current_units");

    let result = serde_json::json!({
        "latitude": resp.get("latitude"),
        "longitude": resp.get("longitude"),
        "timezone": resp.get("timezone"),
        "timezone_abbreviation": resp.get("timezone_abbreviation"),
        "elevation_m": resp.get("elevation"),
        "time": current.get("time"),
        "temperature": current.get("temperature_2m"),
        "apparent_temperature": current.get("apparent_temperature"),
        "relative_humidity_pct": current.get("relative_humidity_2m"),
        "precipitation": current.get("precipitation"),
        "rain": current.get("rain"),
        "showers": current.get("showers"),
        "snowfall": current.get("snowfall"),
        "weather_code": current.get("weather_code"),
        "weather_description": wmo_code_description(
            current.get("weather_code").and_then(|v| v.as_u64()).unwrap_or(0) as u32
        ),
        "cloud_cover_pct": current.get("cloud_cover"),
        "pressure_msl_hpa": current.get("pressure_msl"),
        "surface_pressure_hpa": current.get("surface_pressure"),
        "wind_speed": current.get("wind_speed_10m"),
        "wind_direction_deg": current.get("wind_direction_10m"),
        "wind_gusts": current.get("wind_gusts_10m"),
        "is_day": current.get("is_day").and_then(|v| v.as_u64()).map(|v| v == 1),
        "units": {
            "temperature": units.and_then(|u| u.get("temperature_2m")).and_then(|v| v.as_str()).unwrap_or(if temp_unit == "fahrenheit" { "°F" } else { "°C" }),
            "wind_speed": units.and_then(|u| u.get("wind_speed_10m")).and_then(|v| v.as_str()).unwrap_or(wind_unit),
            "precipitation": units.and_then(|u| u.get("precipitation")).and_then(|v| v.as_str()).unwrap_or("mm"),
        }
    });

    Ok(result)
}

/// Map WMO weather interpretation code to a human-readable description.
pub fn wmo_code_description(code: u32) -> &'static str {
    match code {
        0 => "Clear sky",
        1 => "Mainly clear",
        2 => "Partly cloudy",
        3 => "Overcast",
        45 => "Foggy",
        48 => "Depositing rime fog",
        51 => "Light drizzle",
        53 => "Moderate drizzle",
        55 => "Dense drizzle",
        56 => "Light freezing drizzle",
        57 => "Heavy freezing drizzle",
        61 => "Slight rain",
        63 => "Moderate rain",
        65 => "Heavy rain",
        66 => "Light freezing rain",
        67 => "Heavy freezing rain",
        71 => "Slight snowfall",
        73 => "Moderate snowfall",
        75 => "Heavy snowfall",
        77 => "Snow grains",
        80 => "Slight rain showers",
        81 => "Moderate rain showers",
        82 => "Violent rain showers",
        85 => "Slight snow showers",
        86 => "Heavy snow showers",
        95 => "Thunderstorm",
        96 => "Thunderstorm with slight hail",
        99 => "Thunderstorm with heavy hail",
        _ => "Unknown",
    }
}

/// Log that a current-weather request is starting: the URL embeds the
/// coordinates, so it stays at DEBUG and is never attached to a span (a span
/// field would leave the process with `otel` on regardless of level). Kept
/// as its own function so a test can drive it directly, without a real HTTP
/// client.
fn log_current_request(url: &str) {
    tracing::debug!(url = %url, "requesting upstream weather API");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operations::test_capture::capture_events;

    /// A URL that is unmistakably a marker, never a real upstream request.
    const SENTINEL_URL: &str = "https://example.com/MARKER-weather-current-9f3d1c2a";

    /// AC (mcp-core#40, D10): the request URL (it embeds the coordinates)
    /// reaches the log at DEBUG only, never at a louder level.
    #[test]
    fn log_current_request_puts_the_url_at_debug_only() {
        let events = capture_events(|| log_current_request(SENTINEL_URL));

        assert_eq!(
            events.len(),
            1,
            "a current-weather request must log exactly one event: {events:?}"
        );
        let (level, fields) = &events[0];
        assert_eq!(
            *level,
            tracing::Level::DEBUG,
            "a current-weather request must log at DEBUG, so it stays off the INFO band"
        );
        assert_eq!(
            fields.get("url").map(String::as_str),
            Some(SENTINEL_URL),
            "the event must carry the url that was requested: {fields:?}"
        );
    }
}
