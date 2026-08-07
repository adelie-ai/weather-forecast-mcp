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
        )));
    }
    if !(-180.0..=180.0).contains(&longitude) {
        return Err(WeatherError::InvalidCoordinates(format!(
            "Longitude {} is out of range [-180, 180]",
            longitude
        )));
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
        ))),
    }
}

/// Validate that the wind speed unit is one of the supported values.
pub fn validate_wind_speed_unit(unit: &str) -> Result<()> {
    match unit {
        "kmh" | "ms" | "mph" | "kn" => Ok(()),
        _ => Err(WeatherError::InvalidParameters(format!(
            "Invalid wind_speed_unit '{}'. Use 'kmh', 'ms', 'mph', or 'kn'.",
            unit
        ))),
    }
}

#[cfg(test)]
pub(crate) mod test_capture {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};
    use tracing::Level;
    use tracing::field::{Field, Visit};
    use tracing_subscriber::Layer;
    use tracing_subscriber::layer::{Context, SubscriberExt};

    /// One event, as the subscriber saw it: its level and its rendered
    /// fields (the message is the `message` field).
    pub(crate) type CapturedEvent = (Level, BTreeMap<String, String>);

    #[derive(Clone, Default)]
    struct Capture(Arc<Mutex<Vec<CapturedEvent>>>);

    struct Collector<'a>(&'a mut BTreeMap<String, String>);

    impl Visit for Collector<'_> {
        fn record_str(&mut self, field: &Field, value: &str) {
            self.0.insert(field.name().to_string(), value.to_string());
        }

        fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
            self.0
                .insert(field.name().to_string(), format!("{value:?}"));
        }
    }

    impl<S: tracing::Subscriber> Layer<S> for Capture {
        fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
            let mut fields = BTreeMap::new();
            event.record(&mut Collector(&mut fields));
            self.0
                .lock()
                .expect("capture lock is only held to push one record")
                .push((*event.metadata().level(), fields));
        }
    }

    /// Run `body` with a capturing subscriber installed on this thread, and
    /// return the events it emitted.
    pub(crate) fn capture_events(body: impl FnOnce()) -> Vec<CapturedEvent> {
        let capture = Capture::default();
        let subscriber = tracing_subscriber::registry().with(capture.clone());
        tracing::subscriber::with_default(subscriber, body);
        capture
            .0
            .lock()
            .expect("capture lock is only held to push one record")
            .clone()
    }
}
