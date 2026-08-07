//! Shared support for weather-forecast-mcp's telemetry acceptance tests: a
//! capturing `tracing` layer, a driver that runs the real service under it, a
//! local mock of both Open-Meteo endpoints, and the table of tool cases both
//! content tests iterate.
//!
//! Each test file gets its own copy of this module, so not every item is
//! reached from every file.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::{Arc, Mutex};

use httpmock::MockServer;
use mcp_core::{ServerCore, Session};
use serde_json::{Value, json};
use tracing::Level;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;

use weather_forecast_mcp::WeatherService;

// ── capturing tracing layer ─────────────────────────────────────────────────

/// One span, as the subscriber saw it. A span whose fields are recorded after
/// creation appears a second time, carrying only what was recorded then.
#[derive(Clone, Debug)]
pub struct RecordedSpan {
    /// The span's name.
    pub name: &'static str,
    /// Field name to its rendered value.
    pub fields: BTreeMap<String, String>,
}

/// One event, as the subscriber saw it.
#[derive(Clone, Debug)]
pub struct RecordedEvent {
    /// The level the event was emitted at.
    pub level: Level,
    /// Field name to its rendered value. The message is the `message` field.
    pub fields: BTreeMap<String, String>,
}

/// Everything one captured run produced.
#[derive(Clone, Debug, Default)]
pub struct Recorded {
    /// Spans, in the order they opened.
    pub spans: Vec<RecordedSpan>,
    /// Events, in the order they were emitted.
    pub events: Vec<RecordedEvent>,
}

impl Recorded {
    /// A short rendering for an assertion message.
    pub fn span_summary(&self) -> Vec<String> {
        self.spans
            .iter()
            .map(|span| format!("{}{:?}", span.name, span.fields))
            .collect()
    }

    /// A short rendering for an assertion message.
    pub fn event_summary(&self) -> Vec<String> {
        self.events
            .iter()
            .map(|event| format!("{}{:?}", event.level, event.fields))
            .collect()
    }
}

/// Run `body` with a capturing subscriber installed on this thread, and
/// return what it emitted.
pub fn capture<F, Fut>(body: F) -> Recorded
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = ()>,
{
    let capture = Capture::default();
    let subscriber = tracing_subscriber::registry().with(capture.clone());
    tracing::subscriber::with_default(subscriber, || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("a current-thread runtime");
        runtime.block_on(body());
    });
    capture.take()
}

/// A shared core over weather-forecast-mcp's real service, pointed at
/// `mock_base_url` for both the geocoding and forecast APIs -- the local
/// mock server started by [`start_mock_server`] answers both paths.
pub fn core(mock_base_url: &str) -> Arc<ServerCore> {
    ServerCore::new(
        weather_forecast_mcp::server_config(),
        Arc::new(WeatherService::with_base_urls(mock_base_url, mock_base_url)),
    )
}

/// Drive `messages` through one session over the real service (pointed at
/// `mock_base_url`), capturing what the dispatch and handler paths emitted.
pub fn capture_dispatch(mock_base_url: &str, messages: &[Value]) -> Recorded {
    let mock_base_url = mock_base_url.to_string();
    let messages = messages.to_vec();
    capture(|| async move {
        let mut session = Session::new(core(&mock_base_url));
        for message in messages {
            session.handle_message(message).await;
        }
    })
}

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Recorded>>);

impl Capture {
    fn take(self) -> Recorded {
        self.0
            .lock()
            .expect("the capture lock is only held to push one record")
            .clone()
    }
}

impl<S> Layer<S> for Capture
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, _id: &Id, _ctx: Context<'_, S>) {
        let mut fields = BTreeMap::new();
        attrs.record(&mut Collector(&mut fields));
        self.0
            .lock()
            .expect("the capture lock is only held to push one record")
            .spans
            .push(RecordedSpan {
                name: attrs.metadata().name(),
                fields,
            });
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        let name = ctx.span(id).map_or("<closed>", |span| span.name());
        let mut fields = BTreeMap::new();
        values.record(&mut Collector(&mut fields));
        self.0
            .lock()
            .expect("the capture lock is only held to push one record")
            .spans
            .push(RecordedSpan { name, fields });
    }

    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let mut fields = BTreeMap::new();
        event.record(&mut Collector(&mut fields));
        self.0
            .lock()
            .expect("the capture lock is only held to push one record")
            .events
            .push(RecordedEvent {
                level: *event.metadata().level(),
                fields,
            });
    }
}

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

// ── local mock of both Open-Meteo endpoints ─────────────────────────────────

/// Start a local mock server with routes for both Open-Meteo endpoints this
/// server calls. Keep the returned server alive for the life of the test --
/// dropping it tears the mock down. Its `.base_url()` is what
/// `WeatherService::with_base_urls` (and, for the real binary,
/// `WEATHER_GEOCODING_BASE_URL` / `WEATHER_FORECAST_BASE_URL`) should point
/// at, so no test ever reaches the real Open-Meteo hosts (rule 1.5).
pub fn start_mock_server() -> MockServer {
    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(httpmock::Method::GET)
            .path("/v1/search")
            .query_param("name", SENTINEL_NAME);
        then.status(200)
            .header("content-type", "application/json")
            .json_body(geocoding_success_fixture());
    });
    // weather_geocode failure: no matching location. This is the one whose
    // Display quotes the request back verbatim ("No locations found for:
    // {name}"), so it is the sharpest test of mcp-core#40 lesson 9.
    server.mock(|when, then| {
        when.method(httpmock::Method::GET)
            .path("/v1/search")
            .query_param("name", FAILURE_SENTINEL_NAME);
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({"results": []}));
    });

    server.mock(|when, then| {
        when.method(httpmock::Method::GET)
            .path("/v1/forecast")
            .query_param("latitude", SENTINEL_LAT.to_string());
        then.status(200)
            .header("content-type", "application/json")
            .json_body(forecast_success_fixture());
    });
    // weather_get_current failure: an explicit upstream error (this is how
    // Open-Meteo signals a rate limit -- HTTP 200 with an `error` body, not a
    // 4xx). The reason text echoes the request coordinate, the way a real
    // API's "helpful" error message often does, so this is a direct test of
    // whether that gets to keep the sentinel below INFO.
    server.mock(|when, then| {
        when.method(httpmock::Method::GET)
            .path("/v1/forecast")
            .query_param("latitude", RATE_LIMIT_LAT.to_string());
        then.status(200).header("content-type", "application/json").json_body(json!({
            "error": true,
            "reason": format!("Minutely API request limit exceeded for latitude={RATE_LIMIT_LAT}"),
        }));
    });
    // weather_get_forecast failure: a response missing the `daily` key
    // (the shape an empty/partial upstream answer takes here).
    server.mock(|when, then| {
        when.method(httpmock::Method::GET)
            .path("/v1/forecast")
            .query_param("latitude", MALFORMED_LAT.to_string());
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "latitude": MALFORMED_LAT,
                "longitude": SENTINEL_LON,
                "timezone": "Etc/UTC",
            }));
    });

    server
}

/// A synthetic geocoding response, shaped like Open-Meteo's documented
/// `results: [{name, latitude, longitude, country, admin1, elevation}]`.
/// Hand-built rather than captured: this repo is public, and a captured
/// response would embed a real place, which the non-negotiable rule for this
/// change forbids.
fn geocoding_success_fixture() -> Value {
    json!({
        "results": [{
            "name": "Fictitious Springs",
            "latitude": 12.34,
            "longitude": 56.78,
            "country": "Testland",
            "admin1": "Example Province",
            "elevation": 100.0,
        }]
    })
}

/// A synthetic forecast response carrying both a `current` and a `daily` key,
/// so the same fixture answers both `weather_get_current` and
/// `weather_get_forecast` -- both call the same `/v1/forecast` path; which
/// top-level key each reads depends on which tool called it. Hand-built for
/// the same reason as [`geocoding_success_fixture`].
fn forecast_success_fixture() -> Value {
    json!({
        "latitude": 12.34,
        "longitude": 56.78,
        "timezone": "Etc/UTC",
        "timezone_abbreviation": "UTC",
        "elevation": 100.0,
        "current_units": {
            "temperature_2m": "degC",
            "wind_speed_10m": "km/h",
            "precipitation": "mm",
        },
        "current": {
            "time": "2026-01-01T00:00",
            "temperature_2m": 20.0,
            "apparent_temperature": 19.0,
            "relative_humidity_2m": 50,
            "precipitation": 0.0,
            "rain": 0.0,
            "showers": 0.0,
            "snowfall": 0.0,
            "weather_code": 0,
            "cloud_cover": 10,
            "pressure_msl": 1013.0,
            "surface_pressure": 1010.0,
            "wind_speed_10m": 5.0,
            "wind_direction_10m": 180,
            "wind_gusts_10m": 10.0,
            "is_day": 1,
        },
        "daily": {
            "time": ["2026-01-01"],
            "weather_code": [0],
            "temperature_2m_max": [22.0],
            "temperature_2m_min": [15.0],
            "precipitation_sum": [0.0],
            "precipitation_probability_max": [0],
            "wind_speed_10m_max": [8.0],
            "sunrise": ["2026-01-01T06:00"],
            "sunset": ["2026-01-01T20:00"],
        },
    })
}

// ── the table both content tests iterate ────────────────────────────────────

/// Which branch a [`ToolCase`] drives. A tool is only fully covered once
/// both variants appear for it (mcp-core#40 lesson 9): a success-only table
/// never exercises the error `Display` impls that most naturally quote a
/// location back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expect {
    Success,
    Failure,
}

/// One row: a tool name, which branch it is meant to drive, the arguments to
/// call it with (embedding a sentinel location value), and the sentinel
/// string that value renders as in a log or span field.
///
/// Table-driven over the server's whole tool list (mcp-core#40 lesson 8): a
/// tool added to [`WeatherService::tools`] without a row here fails
/// `tool_cases_cover_every_registered_tool` in `tests/telemetry_span_fields.rs`,
/// so the leak checks cannot silently stop covering a tool the way
/// fileio-mcp's single-tool test did.
pub struct ToolCase {
    pub tool: &'static str,
    pub expect: Expect,
    pub arguments: Value,
    pub sentinel: String,
}

/// The sentinel location name `weather_geocode`'s success case searches for.
pub const SENTINEL_NAME: &str = "MARKER-weather-geocode-9f3d1c2a";
/// The sentinel name `weather_geocode`'s failure case searches for -- the
/// mock answers this one with an empty `results` array.
pub const FAILURE_SENTINEL_NAME: &str = "MARKER-weather-geocode-fail-3c7a1b9e";

/// The sentinel coordinate the coordinate-taking tools' success cases search
/// for. A value ordinary enough to reach a real Open-Meteo request unmolested
/// (in production; here the mock answers it) but distinctive enough that its
/// digits are not going to appear in unrelated output by chance.
pub const SENTINEL_LAT: f64 = 12.345678;
pub const SENTINEL_LON: f64 = 65.432109;

/// `weather_get_current`'s failure-case latitude -- the mock answers this one
/// with an Open-Meteo-shaped rate-limit error whose reason text echoes it.
pub const RATE_LIMIT_LAT: f64 = 44.556677;
/// `weather_get_forecast`'s failure-case latitude -- the mock answers this
/// one with a response missing the `daily` key.
pub const MALFORMED_LAT: f64 = 55.667788;
/// `weather_get_alerts`'s failure-case latitude -- out of range, so
/// `validate_coordinates` rejects it locally; alerts makes no network call at
/// all, so this is its only failure branch.
pub const INVALID_LAT: f64 = 999999.0;

pub fn tool_cases() -> Vec<ToolCase> {
    vec![
        ToolCase {
            tool: "weather_geocode",
            expect: Expect::Success,
            arguments: json!({"name": SENTINEL_NAME, "count": 1}),
            sentinel: SENTINEL_NAME.to_string(),
        },
        ToolCase {
            tool: "weather_geocode",
            expect: Expect::Failure,
            arguments: json!({"name": FAILURE_SENTINEL_NAME, "count": 1}),
            sentinel: FAILURE_SENTINEL_NAME.to_string(),
        },
        ToolCase {
            tool: "weather_get_current",
            expect: Expect::Success,
            arguments: json!({"latitude": SENTINEL_LAT, "longitude": SENTINEL_LON}),
            sentinel: SENTINEL_LAT.to_string(),
        },
        ToolCase {
            tool: "weather_get_current",
            expect: Expect::Failure,
            arguments: json!({"latitude": RATE_LIMIT_LAT, "longitude": SENTINEL_LON}),
            sentinel: RATE_LIMIT_LAT.to_string(),
        },
        ToolCase {
            tool: "weather_get_forecast",
            expect: Expect::Success,
            arguments: json!({
                "latitude": SENTINEL_LAT,
                "longitude": SENTINEL_LON,
                "forecast_type": "daily",
                "days": 1,
            }),
            sentinel: SENTINEL_LAT.to_string(),
        },
        ToolCase {
            tool: "weather_get_forecast",
            expect: Expect::Failure,
            arguments: json!({
                "latitude": MALFORMED_LAT,
                "longitude": SENTINEL_LON,
                "forecast_type": "daily",
                "days": 1,
            }),
            sentinel: MALFORMED_LAT.to_string(),
        },
        ToolCase {
            tool: "weather_get_alerts",
            expect: Expect::Success,
            arguments: json!({"latitude": SENTINEL_LAT, "longitude": SENTINEL_LON}),
            sentinel: SENTINEL_LAT.to_string(),
        },
        ToolCase {
            tool: "weather_get_alerts",
            expect: Expect::Failure,
            arguments: json!({"latitude": INVALID_LAT, "longitude": SENTINEL_LON}),
            sentinel: "999999".to_string(),
        },
    ]
}

/// The tool names [`WeatherService::tools`] actually registers, so a test can
/// assert [`tool_cases`] covers exactly that set.
pub fn registered_tool_names() -> Vec<String> {
    use mcp_core::McpService;
    weather_forecast_mcp::build_service()
        .tools()
        .iter()
        .map(|tool| tool.name.clone())
        .collect()
}
