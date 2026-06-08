#![deny(warnings)]

// McpService implementation for weather-forecast-mcp

use mcp_core::{CallError, McpService, ToolDef, ToolReply, async_trait};
use serde_json::{Value, json};

use crate::operations::{
    alerts, current, forecast, geocode, validate_coordinates, validate_temperature_unit,
    validate_wind_speed_unit,
};

/// The weather forecast MCP service.
pub struct WeatherService {
    client: reqwest::Client,
}

impl WeatherService {
    /// Create a new weather service with a 30-second HTTP timeout.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("reqwest client"),
        }
    }
}

impl Default for WeatherService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl McpService for WeatherService {
    fn tools(&self) -> Vec<ToolDef> {
        vec![
            ToolDef::new(
                "weather_geocode",
                "Resolve a location name to geographic coordinates (latitude and longitude) \
                using the Open-Meteo geocoding API. Returns up to 'count' matching locations \
                with their coordinates, country, region, and elevation. Use the returned \
                latitude/longitude with weather_get_current or weather_get_forecast. \
                IMPORTANT: Use simple city names for best results (e.g. 'Houston' not \
                'Houston, Texas' or 'Houston TX'). The API matches city names, not full \
                addresses. If multiple cities share a name, filter the results by country \
                or region rather than adding qualifiers to the query.",
                json!({
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Location name to search for. Use a simple city or \
                                place name for best results. Examples: 'London', 'New York', \
                                'Tokyo'. Avoid including state abbreviations, country names, \
                                or comma-separated qualifiers."
                        },
                        "count": {
                            "type": "number",
                            "description": "Maximum number of results to return. Range: 1-10 \
                                (default: 5)."
                        },
                        "language": {
                            "type": "string",
                            "description": "Language for result names (ISO 639-1 code). \
                                Default: 'en'. Example: 'de', 'fr', 'es'."
                        }
                    },
                    "required": ["name"]
                }),
            ),
            ToolDef::new(
                "weather_get_current",
                "Get current weather conditions for a specific location by latitude and \
                longitude. Returns temperature, humidity, wind speed, precipitation, cloud \
                cover, pressure, and a human-readable weather description based on WMO \
                weather codes. Use weather_geocode first to resolve a location name to \
                coordinates.",
                json!({
                    "type": "object",
                    "properties": {
                        "latitude": {
                            "type": "number",
                            "description": "Latitude of the location in decimal degrees. \
                                Range: -90 to 90."
                        },
                        "longitude": {
                            "type": "number",
                            "description": "Longitude of the location in decimal degrees. \
                                Range: -180 to 180."
                        },
                        "temperature_unit": {
                            "type": "string",
                            "enum": ["celsius", "fahrenheit"],
                            "description": "Temperature unit. One of: 'celsius' (default), \
                                'fahrenheit'."
                        },
                        "wind_speed_unit": {
                            "type": "string",
                            "enum": ["kmh", "ms", "mph", "kn"],
                            "description": "Wind speed unit. One of: 'kmh' (default), 'ms', \
                                'mph', 'kn'."
                        }
                    },
                    "required": ["latitude", "longitude"]
                }),
            ),
            ToolDef::new(
                "weather_get_forecast",
                "Get weather forecast for a specific location by latitude and longitude. \
                Supports daily forecasts (up to 16 days) and hourly forecasts. Daily \
                forecast includes high/low temperatures, precipitation probability, wind \
                speeds, and sunrise/sunset. Hourly forecast includes temperature, humidity, \
                precipitation probability, wind, and visibility. Use weather_geocode first \
                to resolve a location name to coordinates.",
                json!({
                    "type": "object",
                    "properties": {
                        "latitude": {
                            "type": "number",
                            "description": "Latitude of the location in decimal degrees. \
                                Range: -90 to 90."
                        },
                        "longitude": {
                            "type": "number",
                            "description": "Longitude of the location in decimal degrees. \
                                Range: -180 to 180."
                        },
                        "forecast_type": {
                            "type": "string",
                            "enum": ["daily", "hourly"],
                            "description": "Forecast resolution. One of: 'daily' (default, \
                                days 1-16), 'hourly' (default days=1, max 16)."
                        },
                        "days": {
                            "type": "number",
                            "description": "Number of forecast days. Range: 1-16. Daily \
                                default: 7. Hourly default: 1 (24 rows). Clamped to valid \
                                range automatically."
                        },
                        "temperature_unit": {
                            "type": "string",
                            "enum": ["celsius", "fahrenheit"],
                            "description": "Temperature unit. One of: 'celsius' (default), \
                                'fahrenheit'."
                        },
                        "wind_speed_unit": {
                            "type": "string",
                            "enum": ["kmh", "ms", "mph", "kn"],
                            "description": "Wind speed unit. One of: 'kmh' (default), 'ms', \
                                'mph', 'kn'."
                        }
                    },
                    "required": ["latitude", "longitude"]
                }),
            ),
            ToolDef::new(
                "weather_get_alerts",
                "Get weather alerts for a specific location by latitude and longitude. \
                Returns any active weather warnings or advisories. Note: live alert \
                integration is not yet configured; see the returned note field for how \
                to extend this capability.",
                json!({
                    "type": "object",
                    "properties": {
                        "latitude": {
                            "type": "number",
                            "description": "Latitude of the location in decimal degrees. \
                                Range: -90 to 90."
                        },
                        "longitude": {
                            "type": "number",
                            "description": "Longitude of the location in decimal degrees. \
                                Range: -180 to 180."
                        }
                    },
                    "required": ["latitude", "longitude"]
                }),
            ),
        ]
    }

    async fn call_tool(&self, name: &str, args: &Value) -> Result<ToolReply, CallError> {
        match name {
            "weather_geocode" => call_geocode(&self.client, args).await,
            "weather_get_current" => call_get_current(&self.client, args).await,
            "weather_get_forecast" => call_get_forecast(&self.client, args).await,
            "weather_get_alerts" => call_get_alerts(&self.client, args).await,
            other => Err(CallError::tool(format!("Tool not found: {other}"))),
        }
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────

/// Extract an f64 from a JSON value, accepting both numbers and numeric strings.
fn value_as_f64(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str()?.parse::<f64>().ok())
}

/// Extract a u64 from a JSON value, accepting both numbers and numeric strings.
fn value_as_u64(v: &Value) -> Option<u64> {
    v.as_u64().or_else(|| v.as_str()?.parse::<u64>().ok())
}

async fn call_geocode(client: &reqwest::Client, args: &Value) -> Result<ToolReply, CallError> {
    let name = args
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CallError::tool("Missing required parameter: name"))?;

    let count = args.get("count").and_then(value_as_u64).unwrap_or(5) as u32;
    let language = args.get("language").and_then(|v| v.as_str());

    let result = geocode::geocode_location(client, name, count, language)
        .await
        .map_err(|e| CallError::tool(e.to_string()))?;

    Ok(ToolReply::json(&result)?)
}

async fn call_get_current(client: &reqwest::Client, args: &Value) -> Result<ToolReply, CallError> {
    let latitude = args
        .get("latitude")
        .and_then(value_as_f64)
        .ok_or_else(|| CallError::tool("Missing required parameter: latitude"))?;

    let longitude = args
        .get("longitude")
        .and_then(value_as_f64)
        .ok_or_else(|| CallError::tool("Missing required parameter: longitude"))?;

    // Validate units early so errors name the bad parameter
    if let Some(tu) = args.get("temperature_unit").and_then(|v| v.as_str()) {
        validate_temperature_unit(tu).map_err(|e| CallError::tool(e.to_string()))?;
    }
    if let Some(wu) = args.get("wind_speed_unit").and_then(|v| v.as_str()) {
        validate_wind_speed_unit(wu).map_err(|e| CallError::tool(e.to_string()))?;
    }

    // Coordinate range validated inside the operation
    validate_coordinates(latitude, longitude).map_err(|e| CallError::tool(e.to_string()))?;

    let temperature_unit = args.get("temperature_unit").and_then(|v| v.as_str());
    let wind_speed_unit = args.get("wind_speed_unit").and_then(|v| v.as_str());

    let result = current::get_current_weather(
        client,
        latitude,
        longitude,
        temperature_unit,
        wind_speed_unit,
    )
    .await
    .map_err(|e| CallError::tool(e.to_string()))?;

    Ok(ToolReply::json(&result)?)
}

async fn call_get_forecast(client: &reqwest::Client, args: &Value) -> Result<ToolReply, CallError> {
    let latitude = args
        .get("latitude")
        .and_then(value_as_f64)
        .ok_or_else(|| CallError::tool("Missing required parameter: latitude"))?;

    let longitude = args
        .get("longitude")
        .and_then(value_as_f64)
        .ok_or_else(|| CallError::tool("Missing required parameter: longitude"))?;

    validate_coordinates(latitude, longitude).map_err(|e| CallError::tool(e.to_string()))?;

    let forecast_type_str = args
        .get("forecast_type")
        .and_then(|v| v.as_str())
        .unwrap_or("daily");

    let forecast_type = match forecast_type_str {
        "daily" => forecast::ForecastType::Daily,
        "hourly" => forecast::ForecastType::Hourly,
        other => {
            return Err(CallError::tool(format!(
                "Invalid forecast_type '{other}'. Use 'daily' or 'hourly'."
            )));
        }
    };

    // For hourly forecasts default to 1 day (24 rows); daily defaults to 7.
    let default_days: u64 = match forecast_type {
        forecast::ForecastType::Hourly => 1,
        forecast::ForecastType::Daily => 7,
    };
    let days = args
        .get("days")
        .and_then(value_as_u64)
        .unwrap_or(default_days) as u32;

    let temperature_unit = args.get("temperature_unit").and_then(|v| v.as_str());
    let wind_speed_unit = args.get("wind_speed_unit").and_then(|v| v.as_str());

    // Validate units before hitting the network
    if let Some(tu) = temperature_unit {
        validate_temperature_unit(tu).map_err(|e| CallError::tool(e.to_string()))?;
    }
    if let Some(wu) = wind_speed_unit {
        validate_wind_speed_unit(wu).map_err(|e| CallError::tool(e.to_string()))?;
    }

    let result = forecast::get_forecast(
        client,
        latitude,
        longitude,
        forecast_type,
        days,
        temperature_unit,
        wind_speed_unit,
    )
    .await
    .map_err(|e| CallError::tool(e.to_string()))?;

    Ok(ToolReply::json(&result)?)
}

async fn call_get_alerts(client: &reqwest::Client, args: &Value) -> Result<ToolReply, CallError> {
    let latitude = args
        .get("latitude")
        .and_then(value_as_f64)
        .ok_or_else(|| CallError::tool("Missing required parameter: latitude"))?;

    let longitude = args
        .get("longitude")
        .and_then(value_as_f64)
        .ok_or_else(|| CallError::tool("Missing required parameter: longitude"))?;

    let result = alerts::get_alerts(client, latitude, longitude)
        .await
        .map_err(|e| CallError::tool(e.to_string()))?;

    Ok(ToolReply::json(&result)?)
}
