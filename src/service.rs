#![deny(warnings)]

// McpService implementation for weather-forecast-mcp

use mcp_core::telemetry::metrics::{self, Label};
use mcp_core::{CallError, McpService, ToolDef, ToolReply, async_trait};
use serde_json::{Value, json};

use crate::error::WeatherError;
use crate::operations::{
    alerts, current, forecast, geocode, validate_coordinates, validate_temperature_unit,
    validate_wind_speed_unit,
};
use crate::{DEFAULT_FORECAST_BASE_URL, DEFAULT_GEOCODING_BASE_URL};

/// The weather forecast MCP service.
pub struct WeatherService {
    client: reqwest::Client,
    geocoding_base_url: String,
    forecast_base_url: String,
}

impl WeatherService {
    /// Create a new weather service with a 30-second HTTP timeout, pointed at
    /// the production Open-Meteo hosts.
    pub fn new() -> Self {
        Self::with_base_urls(DEFAULT_GEOCODING_BASE_URL, DEFAULT_FORECAST_BASE_URL)
    }

    /// Create a weather service pointed at explicit upstream hosts.
    ///
    /// The production entry point (`main`) uses this with the CLI/env
    /// defaults, which are the real Open-Meteo hosts; a test passes a local
    /// mock server's URL instead, so the outbound-request tests never reach
    /// a live service (rule 1.5).
    pub fn with_base_urls(
        geocoding_base_url: impl Into<String>,
        forecast_base_url: impl Into<String>,
    ) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("reqwest client"),
            geocoding_base_url: geocoding_base_url.into(),
            forecast_base_url: forecast_base_url.into(),
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
            "weather_geocode" => call_geocode(&self.client, &self.geocoding_base_url, args).await,
            "weather_get_current" => {
                call_get_current(&self.client, &self.forecast_base_url, args).await
            }
            "weather_get_forecast" => {
                call_get_forecast(&self.client, &self.forecast_base_url, args).await
            }
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

/// Classify a `WeatherError` as an upstream fault worth counting, or `None`
/// for a caller-input mistake validated locally (never reaches the network),
/// or a normal "no match" outcome. Exhaustive over `WeatherError`, so a new
/// variant forces this classification to be revisited rather than silently
/// landing as "not counted" (rule 8.2: an operational decline is not a
/// failure).
fn upstream_failure_reason(err: &WeatherError) -> Option<&'static str> {
    match err {
        WeatherError::Http(_) => Some("network"),
        WeatherError::ApiError(_) => Some("api_error"),
        WeatherError::ForecastUnavailable(_) => Some("malformed_response"),
        // A "no results" answer from the upstream geocoder is a normal
        // business outcome (rule 8.2), not a fault reaching outward.
        WeatherError::LocationNotFound(_) => None,
        // Caller input, rejected by local validation before any network
        // call is made.
        WeatherError::InvalidCoordinates(_) => None,
        WeatherError::InvalidParameters(_) => None,
    }
}

/// Count an upstream-level failure against `weather.upstream_failure`.
///
/// `tool` is always one of the four `&'static str` literals its call sites
/// pass, so the label is bounded there rather than by anything a caller
/// supplies; `reason` is bounded the same way, by
/// [`upstream_failure_reason`]'s fixed set of return values. Neither label is
/// ever built from a location name or a coordinate.
fn record_upstream_failure(tool: &'static str, outcome: &Result<Value, WeatherError>) {
    if let Err(err) = outcome
        && let Some(reason) = upstream_failure_reason(err)
    {
        metrics::increment(
            "weather.upstream_failure",
            &[Label::new("tool", tool), Label::new("reason", reason)],
        );
    }
}

// `args` carries the location (a name or coordinates) and is skipped: a tool
// argument is content, so it must never become a span field (D10). The span
// still gives this handler's own work its own timing, nested under
// mcp-core's `mcp.tools.call` span.
#[tracing::instrument(skip_all)]
async fn call_geocode(
    client: &reqwest::Client,
    base_url: &str,
    args: &Value,
) -> Result<ToolReply, CallError> {
    let name = args
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CallError::tool("Missing required parameter: name"))?;

    let count = args.get("count").and_then(value_as_u64).unwrap_or(5) as u32;
    let language = args.get("language").and_then(|v| v.as_str());

    let result = geocode::geocode_location(client, base_url, name, count, language).await;
    record_upstream_failure("weather_geocode", &result);
    let result = result.map_err(|e| CallError::tool(e.to_string()))?;

    Ok(ToolReply::json(&result)?)
}

#[tracing::instrument(skip_all)]
async fn call_get_current(
    client: &reqwest::Client,
    base_url: &str,
    args: &Value,
) -> Result<ToolReply, CallError> {
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
        base_url,
        latitude,
        longitude,
        temperature_unit,
        wind_speed_unit,
    )
    .await;
    record_upstream_failure("weather_get_current", &result);
    let result = result.map_err(|e| CallError::tool(e.to_string()))?;

    Ok(ToolReply::json(&result)?)
}

#[tracing::instrument(skip_all)]
async fn call_get_forecast(
    client: &reqwest::Client,
    base_url: &str,
    args: &Value,
) -> Result<ToolReply, CallError> {
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
        base_url,
        latitude,
        longitude,
        forecast_type,
        days,
        temperature_unit,
        wind_speed_unit,
    )
    .await;
    record_upstream_failure("weather_get_forecast", &result);
    let result = result.map_err(|e| CallError::tool(e.to_string()))?;

    Ok(ToolReply::json(&result)?)
}

#[tracing::instrument(skip_all)]
async fn call_get_alerts(client: &reqwest::Client, args: &Value) -> Result<ToolReply, CallError> {
    let latitude = args
        .get("latitude")
        .and_then(value_as_f64)
        .ok_or_else(|| CallError::tool("Missing required parameter: latitude"))?;

    let longitude = args
        .get("longitude")
        .and_then(value_as_f64)
        .ok_or_else(|| CallError::tool("Missing required parameter: longitude"))?;

    let result = alerts::get_alerts(client, latitude, longitude).await;
    record_upstream_failure("weather_get_alerts", &result);
    let result = result.map_err(|e| CallError::tool(e.to_string()))?;

    Ok(ToolReply::json(&result)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The metrics registry [`mcp_core::telemetry::metrics`] records into is
    /// process-global, and `cargo test` runs a file's tests concurrently by
    /// default. Every test below either records into the registry (a writer)
    /// or reads it back (a reader), so two tests running at once can inflate
    /// each other's before/after delta. This guards every test in this
    /// module so they run one at a time relative to each other; it holds no
    /// data of its own.
    static METRICS_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn lock_metrics() -> std::sync::MutexGuard<'static, ()> {
        METRICS_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// AC (mcp-core#40): a fault reaching outward -- an explicit upstream
    /// error, or a response missing the shape this server expects -- is
    /// counted.
    #[test]
    fn upstream_failure_reason_counts_api_and_malformed_response_faults() {
        assert_eq!(
            upstream_failure_reason(&WeatherError::ApiError("rate limited".into())),
            Some("api_error")
        );
        assert_eq!(
            upstream_failure_reason(&WeatherError::ForecastUnavailable(
                "No daily data in response".into()
            )),
            Some("malformed_response")
        );
    }

    /// AC (mcp-core#40): a transport-level fault (here, a malformed URL that
    /// fails to build a request -- offline, no network access) is counted
    /// too.
    #[tokio::test]
    async fn upstream_failure_reason_counts_a_network_fault() {
        let build_err = reqwest::Client::new()
            .get("not a valid url")
            .send()
            .await
            .expect_err("a malformed url must fail before any network access");
        assert_eq!(
            upstream_failure_reason(&WeatherError::Http(build_err)),
            Some("network")
        );
    }

    /// AC (mcp-core#40, rule 8.2): a "no match" answer from the upstream
    /// geocoder is a normal business outcome, and coordinates or units
    /// rejected by local validation never reach the network at all -- neither
    /// is a fault reaching outward, so both are excluded by the exhaustive
    /// match rather than left to fall through silently.
    #[test]
    fn upstream_failure_reason_excludes_business_declines_and_local_validation() {
        assert_eq!(
            upstream_failure_reason(&WeatherError::LocationNotFound(
                "No locations found for: nowhere".into()
            )),
            None
        );
        assert_eq!(
            upstream_failure_reason(&WeatherError::InvalidCoordinates(
                "Latitude 999 is out of range [-90, 90]".into()
            )),
            None
        );
        assert_eq!(
            upstream_failure_reason(&WeatherError::InvalidParameters(
                "Invalid temperature_unit 'kelvin'".into()
            )),
            None
        );
    }

    /// AC (mcp-core#40): `record_upstream_failure` moves the counter only for
    /// a reason [`upstream_failure_reason`] counts, labelled by tool and
    /// reason.
    #[test]
    fn record_upstream_failure_increments_only_for_counted_reasons() {
        let _guard = lock_metrics();
        let labels = [
            Label::new("tool", "weather_get_current"),
            Label::new("reason", "api_error"),
        ];
        let before = counter_total("weather.upstream_failure", &labels);

        let ok: Result<Value, WeatherError> = Ok(json!({}));
        record_upstream_failure("weather_get_current", &ok);
        let decline: Result<Value, WeatherError> = Err(WeatherError::LocationNotFound("x".into()));
        record_upstream_failure("weather_get_current", &decline);
        assert_eq!(
            counter_total("weather.upstream_failure", &labels),
            before,
            "a successful call or a business decline must not move the counter"
        );

        let api_failed: Result<Value, WeatherError> = Err(WeatherError::ApiError("x".into()));
        record_upstream_failure("weather_get_current", &api_failed);
        assert_eq!(
            counter_total("weather.upstream_failure", &labels),
            before + 1,
            "an upstream API fault must increment the counter, labelled by tool and reason"
        );
    }

    fn counter_total(name: &str, labels: &[Label]) -> u64 {
        metrics::global()
            .snapshot()
            .counters
            .iter()
            .find(|counter| counter.name == name && same_labels(&counter.labels, labels))
            .map_or(0, |counter| counter.total)
    }

    fn same_labels(recorded: &[Label], wanted: &[Label]) -> bool {
        recorded.len() == wanted.len()
            && wanted.iter().all(|want| {
                recorded
                    .iter()
                    .any(|have| have.key() == want.key() && have.value() == want.value())
            })
    }
}
