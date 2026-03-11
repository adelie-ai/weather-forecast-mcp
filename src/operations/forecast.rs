#![deny(warnings)]

// Weather forecast using Open-Meteo API

use crate::error::{Result, WeatherError};
use crate::operations::current::wmo_code_description;
use serde_json::Value;

/// Fetch weather forecast for given coordinates.
///
/// Returns hourly or daily forecasts depending on `forecast_type`.
pub async fn get_forecast(
    client: &reqwest::Client,
    latitude: f64,
    longitude: f64,
    forecast_type: ForecastType,
    days: u32,
    temperature_unit: Option<&str>,
    wind_speed_unit: Option<&str>,
) -> Result<Value> {
    validate_coordinates(latitude, longitude)?;

    let temp_unit = temperature_unit.unwrap_or("celsius");
    let wind_unit = wind_speed_unit.unwrap_or("kmh");

    validate_temperature_unit(temp_unit)?;
    validate_wind_speed_unit(wind_unit)?;

    let days = days.clamp(1, 16);

    let url = match forecast_type {
        ForecastType::Daily => format!(
            "https://api.open-meteo.com/v1/forecast\
            ?latitude={}&longitude={}\
            &daily=weather_code,temperature_2m_max,temperature_2m_min,\
            apparent_temperature_max,apparent_temperature_min,\
            sunrise,sunset,daylight_duration,sunshine_duration,\
            precipitation_sum,rain_sum,snowfall_sum,precipitation_hours,\
            precipitation_probability_max,\
            wind_speed_10m_max,wind_gusts_10m_max,wind_direction_10m_dominant\
            &temperature_unit={}&wind_speed_unit={}&timezone=auto&forecast_days={}",
            latitude, longitude, temp_unit, wind_unit, days
        ),
        ForecastType::Hourly => format!(
            "https://api.open-meteo.com/v1/forecast\
            ?latitude={}&longitude={}\
            &hourly=temperature_2m,relative_humidity_2m,apparent_temperature,\
            precipitation_probability,precipitation,rain,showers,snowfall,\
            weather_code,cloud_cover,visibility,wind_speed_10m,\
            wind_direction_10m,wind_gusts_10m\
            &temperature_unit={}&wind_speed_unit={}&timezone=auto&forecast_days={}",
            latitude, longitude, temp_unit, wind_unit, days
        ),
    };

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
        ))
        .into());
    }

    match forecast_type {
        ForecastType::Daily => build_daily_response(&resp, temp_unit, wind_unit),
        ForecastType::Hourly => build_hourly_response(&resp, temp_unit, wind_unit),
    }
}

/// Forecast resolution type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForecastType {
    Daily,
    Hourly,
}

fn build_daily_response(resp: &Value, temp_unit: &str, wind_unit: &str) -> Result<Value> {
    let daily = resp
        .get("daily")
        .ok_or_else(|| WeatherError::ForecastUnavailable("No daily data in response".into()))?;

    let times = daily
        .get("time")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let codes = daily
        .get("weather_code")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let t_max = daily
        .get("temperature_2m_max")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let t_min = daily
        .get("temperature_2m_min")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let precip_sum = daily
        .get("precipitation_sum")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let precip_prob = daily
        .get("precipitation_probability_max")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let wind_max = daily
        .get("wind_speed_10m_max")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let sunrise = daily
        .get("sunrise")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let sunset = daily
        .get("sunset")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let days: Vec<Value> = (0..times.len())
        .map(|i| {
            let code = codes.get(i).and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            serde_json::json!({
                "date": times.get(i),
                "weather_code": code,
                "weather_description": wmo_code_description(code),
                "temperature_max": t_max.get(i),
                "temperature_min": t_min.get(i),
                "precipitation_sum": precip_sum.get(i),
                "precipitation_probability_max_pct": precip_prob.get(i),
                "wind_speed_max": wind_max.get(i),
                "sunrise": sunrise.get(i),
                "sunset": sunset.get(i),
            })
        })
        .collect();

    Ok(serde_json::json!({
        "latitude": resp.get("latitude"),
        "longitude": resp.get("longitude"),
        "timezone": resp.get("timezone"),
        "timezone_abbreviation": resp.get("timezone_abbreviation"),
        "elevation_m": resp.get("elevation"),
        "forecast_type": "daily",
        "units": {
            "temperature": if temp_unit == "fahrenheit" { "°F" } else { "°C" },
            "wind_speed": wind_unit,
            "precipitation": "mm",
        },
        "days": days,
    }))
}

fn build_hourly_response(resp: &Value, temp_unit: &str, wind_unit: &str) -> Result<Value> {
    let hourly = resp
        .get("hourly")
        .ok_or_else(|| WeatherError::ForecastUnavailable("No hourly data in response".into()))?;

    let times = hourly
        .get("time")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let codes = hourly
        .get("weather_code")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let temps = hourly
        .get("temperature_2m")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let humidity = hourly
        .get("relative_humidity_2m")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let precip_prob = hourly
        .get("precipitation_probability")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let precip = hourly
        .get("precipitation")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let wind_speed = hourly
        .get("wind_speed_10m")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let visibility = hourly
        .get("visibility")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let hours: Vec<Value> = (0..times.len())
        .map(|i| {
            let code = codes.get(i).and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            serde_json::json!({
                "time": times.get(i),
                "weather_code": code,
                "weather_description": wmo_code_description(code),
                "temperature": temps.get(i),
                "relative_humidity_pct": humidity.get(i),
                "precipitation_probability_pct": precip_prob.get(i),
                "precipitation": precip.get(i),
                "wind_speed": wind_speed.get(i),
                "visibility_m": visibility.get(i),
            })
        })
        .collect();

    Ok(serde_json::json!({
        "latitude": resp.get("latitude"),
        "longitude": resp.get("longitude"),
        "timezone": resp.get("timezone"),
        "timezone_abbreviation": resp.get("timezone_abbreviation"),
        "elevation_m": resp.get("elevation"),
        "forecast_type": "hourly",
        "units": {
            "temperature": if temp_unit == "fahrenheit" { "°F" } else { "°C" },
            "wind_speed": wind_unit,
            "precipitation": "mm",
        },
        "hours": hours,
    }))
}

fn validate_coordinates(latitude: f64, longitude: f64) -> Result<()> {
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

fn validate_temperature_unit(unit: &str) -> Result<()> {
    match unit {
        "celsius" | "fahrenheit" => Ok(()),
        _ => Err(WeatherError::InvalidParameters(format!(
            "Invalid temperature_unit '{}'. Use 'celsius' or 'fahrenheit'.",
            unit
        ))
        .into()),
    }
}

fn validate_wind_speed_unit(unit: &str) -> Result<()> {
    match unit {
        "kmh" | "ms" | "mph" | "kn" => Ok(()),
        _ => Err(WeatherError::InvalidParameters(format!(
            "Invalid wind_speed_unit '{}'. Use 'kmh', 'ms', 'mph', or 'kn'.",
            unit
        ))
        .into()),
    }
}
