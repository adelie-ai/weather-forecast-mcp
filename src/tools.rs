#![deny(warnings)]

// Tool registry and MCP tool definitions

use crate::error::{McpError, Result};
use crate::operations::{alerts, current, forecast, geocode};
use serde_json::Value;

/// Tool registry that manages all available tools
pub struct ToolRegistry {
    client: reqwest::Client,
}

impl ToolRegistry {
    /// Create a new tool registry
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Get all tools in MCP format
    pub fn list_tools(&self) -> Value {
        serde_json::json!([
            {
                "name": "weather_get_current",
                "description": "Get current weather conditions for a specific location by latitude and longitude. Returns temperature, humidity, wind speed, precipitation, cloud cover, pressure, and a human-readable weather description based on WMO weather codes. Use weather_geocode first to resolve a location name to coordinates.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "latitude": {
                            "type": "number",
                            "description": "Latitude of the location in decimal degrees. Range: -90 to 90."
                        },
                        "longitude": {
                            "type": "number",
                            "description": "Longitude of the location in decimal degrees. Range: -180 to 180."
                        },
                        "temperature_unit": {
                            "type": "string",
                            "description": "Temperature unit. One of: 'celsius' (default), 'fahrenheit'."
                        },
                        "wind_speed_unit": {
                            "type": "string",
                            "description": "Wind speed unit. One of: 'kmh' (default), 'ms', 'mph', 'kn'."
                        }
                    },
                    "required": ["latitude", "longitude"]
                }
            },
            {
                "name": "weather_get_forecast",
                "description": "Get weather forecast for a specific location by latitude and longitude. Supports daily forecasts (up to 16 days) and hourly forecasts. Daily forecast includes high/low temperatures, precipitation probability, wind speeds, and sunrise/sunset. Hourly forecast includes temperature, humidity, precipitation probability, wind, and visibility. Use weather_geocode first to resolve a location name to coordinates.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "latitude": {
                            "type": "number",
                            "description": "Latitude of the location in decimal degrees. Range: -90 to 90."
                        },
                        "longitude": {
                            "type": "number",
                            "description": "Longitude of the location in decimal degrees. Range: -180 to 180."
                        },
                        "forecast_type": {
                            "type": "string",
                            "description": "Forecast resolution. One of: 'daily' (default), 'hourly'."
                        },
                        "days": {
                            "type": "number",
                            "description": "Number of forecast days. Range: 1-16 (default: 7). Clamped to valid range automatically."
                        },
                        "temperature_unit": {
                            "type": "string",
                            "description": "Temperature unit. One of: 'celsius' (default), 'fahrenheit'."
                        },
                        "wind_speed_unit": {
                            "type": "string",
                            "description": "Wind speed unit. One of: 'kmh' (default), 'ms', 'mph', 'kn'."
                        }
                    },
                    "required": ["latitude", "longitude"]
                }
            },
            {
                "name": "weather_geocode",
                "description": "Resolve a location name (city, region, or place) to geographic coordinates (latitude and longitude) using the Open-Meteo geocoding API. Returns up to 'count' matching locations with their coordinates, country, region, and elevation. Use the returned latitude/longitude with weather_get_current or weather_get_forecast.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Location name to search for. Can be a city, town, region, or place name. Example: 'London', 'New York', 'Tokyo'."
                        },
                        "count": {
                            "type": "number",
                            "description": "Maximum number of results to return. Range: 1-10 (default: 5)."
                        },
                        "language": {
                            "type": "string",
                            "description": "Language for result names (ISO 639-1 code). Default: 'en'. Example: 'de', 'fr', 'es'."
                        }
                    },
                    "required": ["name"]
                }
            },
            {
                "name": "weather_get_alerts",
                "description": "Get weather alerts for a specific location by latitude and longitude. Returns any active weather warnings or advisories. Note: live alert integration is not yet configured; see the returned note field for how to extend this capability.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "latitude": {
                            "type": "number",
                            "description": "Latitude of the location in decimal degrees. Range: -90 to 90."
                        },
                        "longitude": {
                            "type": "number",
                            "description": "Longitude of the location in decimal degrees. Range: -180 to 180."
                        }
                    },
                    "required": ["latitude", "longitude"]
                }
            }
        ])
    }

    /// Execute a tool call by name with given arguments
    pub async fn execute_tool(&self, tool_name: &str, arguments: &Value) -> Result<Value> {
        match tool_name {
            "weather_get_current" => self.execute_get_current(arguments).await,
            "weather_get_forecast" => self.execute_get_forecast(arguments).await,
            "weather_geocode" => self.execute_geocode(arguments).await,
            "weather_get_alerts" => self.execute_get_alerts(arguments).await,
            _ => Err(McpError::ToolNotFound(tool_name.to_string()).into()),
        }
    }

    async fn execute_get_current(&self, arguments: &Value) -> Result<Value> {
        let latitude = arguments
            .get("latitude")
            .and_then(value_as_f64)
            .ok_or_else(|| McpError::InvalidToolParameters("Missing required parameter: latitude".to_string()))?;

        let longitude = arguments
            .get("longitude")
            .and_then(value_as_f64)
            .ok_or_else(|| McpError::InvalidToolParameters("Missing required parameter: longitude".to_string()))?;

        let temperature_unit = arguments
            .get("temperature_unit")
            .and_then(|v| v.as_str());

        let wind_speed_unit = arguments
            .get("wind_speed_unit")
            .and_then(|v| v.as_str());

        let result = current::get_current_weather(
            &self.client,
            latitude,
            longitude,
            temperature_unit,
            wind_speed_unit,
        )
        .await?;

        Ok(mcp_tool_result_json(result))
    }

    async fn execute_get_forecast(&self, arguments: &Value) -> Result<Value> {
        let latitude = arguments
            .get("latitude")
            .and_then(value_as_f64)
            .ok_or_else(|| McpError::InvalidToolParameters("Missing required parameter: latitude".to_string()))?;

        let longitude = arguments
            .get("longitude")
            .and_then(value_as_f64)
            .ok_or_else(|| McpError::InvalidToolParameters("Missing required parameter: longitude".to_string()))?;

        let forecast_type_str = arguments
            .get("forecast_type")
            .and_then(|v| v.as_str())
            .unwrap_or("daily");

        let forecast_type = match forecast_type_str {
            "daily" => forecast::ForecastType::Daily,
            "hourly" => forecast::ForecastType::Hourly,
            other => {
                return Err(McpError::InvalidToolParameters(format!(
                    "Invalid forecast_type '{}'. Use 'daily' or 'hourly'.",
                    other
                ))
                .into())
            }
        };

        let days = arguments
            .get("days")
            .and_then(value_as_u64)
            .unwrap_or(7) as u32;

        let temperature_unit = arguments
            .get("temperature_unit")
            .and_then(|v| v.as_str());

        let wind_speed_unit = arguments
            .get("wind_speed_unit")
            .and_then(|v| v.as_str());

        let result = forecast::get_forecast(
            &self.client,
            latitude,
            longitude,
            forecast_type,
            days,
            temperature_unit,
            wind_speed_unit,
        )
        .await?;

        Ok(mcp_tool_result_json(result))
    }

    async fn execute_geocode(&self, arguments: &Value) -> Result<Value> {
        let name = arguments
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::InvalidToolParameters("Missing required parameter: name".to_string()))?;

        let count = arguments
            .get("count")
            .and_then(value_as_u64)
            .unwrap_or(5) as u32;

        let language = arguments
            .get("language")
            .and_then(|v| v.as_str());

        let result = geocode::geocode_location(&self.client, name, count, language).await?;

        Ok(mcp_tool_result_json(result))
    }

    async fn execute_get_alerts(&self, arguments: &Value) -> Result<Value> {
        let latitude = arguments
            .get("latitude")
            .and_then(value_as_f64)
            .ok_or_else(|| McpError::InvalidToolParameters("Missing required parameter: latitude".to_string()))?;

        let longitude = arguments
            .get("longitude")
            .and_then(value_as_f64)
            .ok_or_else(|| McpError::InvalidToolParameters("Missing required parameter: longitude".to_string()))?;

        let result = alerts::get_alerts(&self.client, latitude, longitude).await?;

        Ok(mcp_tool_result_json(result))
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract an f64 from a JSON value, accepting both numbers and numeric strings.
fn value_as_f64(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str()?.parse::<f64>().ok())
}

/// Extract a u64 from a JSON value, accepting both numbers and numeric strings.
fn value_as_u64(v: &Value) -> Option<u64> {
    v.as_u64().or_else(|| v.as_str()?.parse::<u64>().ok())
}

/// Wrap a JSON value in the MCP tool result content format.
fn mcp_tool_result_json(value: Value) -> Value {
    serde_json::json!({
        "content": [
            {
                "type": "json",
                "value": value,
            }
        ]
    })
}
