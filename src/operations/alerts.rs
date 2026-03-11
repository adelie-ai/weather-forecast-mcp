#![deny(warnings)]

// Weather alerts using Open-Meteo API (via flood and marine APIs where available)
// Note: Open-Meteo does not provide a general alert feed; this module provides
// a structured "no alerts available" response and documents where to integrate
// a proper alert source (e.g. NWS CAP feed, Meteoalarm) in the future.

use crate::error::Result;
use serde_json::Value;

/// Return weather alerts for given coordinates.
///
/// Currently returns a structured placeholder indicating the alerts capability
/// is not yet backed by a live alert API. Integrate an alert provider
/// (e.g. NWS CAP for US, Meteoalarm for Europe) here in the future.
pub async fn get_alerts(
    _client: &reqwest::Client,
    latitude: f64,
    longitude: f64,
) -> Result<Value> {
    Ok(serde_json::json!({
        "latitude": latitude,
        "longitude": longitude,
        "alerts": [],
        "note": "Live alert integration not yet configured. To add alert support, integrate an alert provider such as the NWS CAP API (US) or Meteoalarm (Europe) in src/operations/alerts.rs.",
    }))
}
