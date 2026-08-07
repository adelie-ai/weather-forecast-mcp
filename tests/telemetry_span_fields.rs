#![deny(warnings)]

// In-process proof of D10 for weather-forecast-mcp, table-driven over the
// server's whole tool list rather than any one tool (mcp-core#40 lesson 8:
// fileio-mcp's equivalent test covered one tool of twenty-seven, and review
// caught a dropped `skip_all` on the tested one while missing it on two
// others). `tests/support::tool_cases` plants a sentinel location in every
// registered tool's arguments, and `tool_cases_cover_every_registered_tool`
// below fails the moment a tool is added to `WeatherService::tools()` without
// a matching row, so the leak checks cannot silently stop covering a tool.
//
// Each tool has both a success and a failure case (mcp-core#40 lesson 9): a
// success-only table never exercises the error `Display` impls that most
// naturally quote a location back -- `WeatherError::LocationNotFound` and
// `WeatherError::InvalidCoordinates` both do exactly that, and
// `tool_cases_cover_every_registered_tool_on_both_branches` fails if a tool
// ever loses its failure row. `network_fault_does_not_leak_the_upstream_url`
// covers the sharpest instance of that risk directly: `reqwest::Error`'s own
// `Display` embeds the request URL.
//
// Every call here goes through a local mock of both Open-Meteo endpoints
// (`tests/support::start_mock_server`), never a live service.
//
// `tests/telemetry_stdio.rs` proves the same no-leak claim against the real,
// installed subscriber; this drives mcp-core's dispatch directly and reads
// back the spans and events it really emitted. A span field would not
// necessarily show up on an INFO-level *line* of console text (the fmt layer
// only renders a span's fields on a line when some event fires while that
// span is entered), so this checks span fields directly rather than relying
// on the console rendering to surface one.

mod support;

use serde_json::json;
use tracing::Level;

use support::{
    Expect, Recorded, capture_dispatch, registered_tool_names, start_mock_server, tool_cases,
};

/// Drive one `tools/call` per [`support::tool_cases`] row through a fresh
/// session, and return what the dispatch and handler paths emitted.
fn capture_all_tool_calls(mock_base_url: &str) -> Recorded {
    let mut messages =
        vec![json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}})];
    for (i, case) in tool_cases().into_iter().enumerate() {
        messages.push(json!({
            "jsonrpc": "2.0",
            "id": i + 2,
            "method": "tools/call",
            "params": {"name": case.tool, "arguments": case.arguments},
        }));
    }
    capture_dispatch(mock_base_url, &messages)
}

/// The completeness guard mcp-core#40 lesson 8 asks for: `tool_cases` must
/// name exactly the tools `WeatherService::tools()` registers, in either
/// direction. A tool added without a row here fails this test rather than
/// silently going untested by the two leak checks below.
#[test]
fn tool_cases_cover_every_registered_tool() {
    let mut cases: Vec<&str> = tool_cases().iter().map(|c| c.tool).collect();
    cases.sort_unstable();
    cases.dedup();

    let mut registered = registered_tool_names();
    registered.sort_unstable();
    registered.dedup();

    assert_eq!(
        cases, registered,
        "tests/support::tool_cases must list exactly the tools WeatherService::tools() \
         registers -- a mismatch means a tool exists with no leak-check coverage, or a case \
         exists for a tool that no longer does"
    );
}

/// AC (mcp-core#40 lesson 9): every registered tool has both a success and a
/// failure row. Covering a tool is not covering it if only its success path
/// ever runs -- the failure branch is where an error type's `Display` most
/// naturally quotes back the content that made it fail.
#[test]
fn tool_cases_cover_every_registered_tool_on_both_branches() {
    for tool in registered_tool_names() {
        let branches: Vec<Expect> = tool_cases()
            .into_iter()
            .filter(|c| c.tool == tool)
            .map(|c| c.expect)
            .collect();
        assert!(
            branches.contains(&Expect::Success),
            "{tool} has no Expect::Success row in tests/support::tool_cases"
        );
        assert!(
            branches.contains(&Expect::Failure),
            "{tool} has no Expect::Failure row in tests/support::tool_cases"
        );
    }
}

/// Sanity check on the test fixtures themselves: every row marked
/// `Expect::Failure` must actually produce mcp-core's "tool returned an error
/// result" DEBUG event, or the leak checks above would be exercising the
/// success path under a different name and proving nothing about the failure
/// branch. Counts rather than correlates per-tool (the capture layer does not
/// track span/event nesting), which is enough to catch a mock that silently
/// answers with a 200 the code accepts as success.
#[test]
fn failure_cases_actually_fail() {
    let mock = start_mock_server();
    let recorded = capture_all_tool_calls(&mock.base_url());

    let expected_failures = tool_cases()
        .iter()
        .filter(|c| c.expect == Expect::Failure)
        .count();
    let observed_failures = recorded
        .events
        .iter()
        .filter(|event| {
            event.fields.get("message").map(String::as_str) == Some("tool returned an error result")
        })
        .count();

    assert_eq!(
        observed_failures, expected_failures,
        "expected {expected_failures} failure outcomes (one per Expect::Failure row), but \
         mcp-core's dispatch reported {observed_failures}; a mock that answers 200 with content \
         the code accepts as success would under-count here, silently turning a failure case \
         into an untested success case"
    );
}

/// AC (mcp-core#40 lesson 9): a genuine transport-level fault -- here, a
/// non-JSON response body, which makes `.json()` fail -- classifies as
/// `WeatherError::Http`. `reqwest::Error`'s own `Display` embeds the request
/// URL, which is exactly the risk lesson 9 names: an error type written to
/// be helpful quotes back what failed. That URL must not reach an INFO line
/// or a span field either, even though weather-forecast-mcp's own code never
/// chose to write it there.
#[test]
fn network_fault_does_not_leak_the_upstream_url_above_debug() {
    let mock = start_mock_server();
    let fault_lat = 77.889900_f64;
    mock.mock(|when, then| {
        when.method(httpmock::Method::GET)
            .path("/v1/forecast")
            .query_param("latitude", fault_lat.to_string());
        then.status(200).body("not valid json");
    });

    let recorded = capture_dispatch(
        &mock.base_url(),
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {
                    "name": "weather_get_current",
                    "arguments": {"latitude": fault_lat, "longitude": 65.432109},
                },
            }),
        ],
    );

    // The mock's own base URL (host:port) is the substring reqwest's Display
    // would embed -- it is what makes this test able to fail at all.
    let url_sentinel = mock.base_url();

    for span in &recorded.spans {
        for (key, value) in &span.fields {
            assert!(
                !value.contains(&url_sentinel),
                "the upstream URL reached span {:?} field {key:?}: {value:?}; all spans were \
                 {:?}",
                span.name,
                recorded.span_summary()
            );
        }
    }
    for event in &recorded.events {
        if event.level > Level::INFO {
            continue;
        }
        for (key, value) in &event.fields {
            assert!(
                !value.contains(&url_sentinel),
                "the upstream URL reached a {} line, field {key:?}: {value:?}; all events were \
                 {:?}",
                event.level,
                recorded.event_summary()
            );
        }
    }

    let at_debug = recorded.events.iter().any(|event| {
        event.level == Level::DEBUG
            && event
                .fields
                .values()
                .any(|value| value.contains(&url_sentinel))
    });
    assert!(
        at_debug,
        "the upstream URL must still be reachable at DEBUG (inside the CallError::Tool \
         message reqwest built), or this test cannot tell a real fix from a line that was \
         simply deleted; the events were {:?}",
        recorded.event_summary()
    );
}

/// AC (mcp-core#40, D10): no tool-call span field carries a sentinel
/// location, at any level, and no INFO (or higher) event carries one either
/// -- for every registered tool. The same run proves the positive half too:
/// mcp-core's own dispatch layer logs the tool arguments at DEBUG, so this
/// test cannot pass simply because nothing was captured.
#[test]
fn tool_call_leaves_no_sentinel_in_any_span_field_or_info_event() {
    let mock = start_mock_server();
    let recorded = capture_all_tool_calls(&mock.base_url());
    let cases = tool_cases();

    for case in &cases {
        for span in &recorded.spans {
            for (key, value) in &span.fields {
                assert!(
                    !value.contains(&case.sentinel),
                    "{}'s sentinel reached span {:?} field {key:?}: {value:?}; all spans were \
                     {:?}",
                    case.tool,
                    span.name,
                    recorded.span_summary()
                );
            }
        }

        for event in &recorded.events {
            if event.level > Level::INFO {
                continue;
            }
            for (key, value) in &event.fields {
                assert!(
                    !value.contains(&case.sentinel),
                    "{}'s sentinel reached a {} line, field {key:?}: {value:?}; all events were \
                     {:?}",
                    case.tool,
                    event.level,
                    recorded.event_summary()
                );
            }
        }

        let at_debug = recorded.events.iter().any(|event| {
            event.level == Level::DEBUG
                && event
                    .fields
                    .values()
                    .any(|value| value.contains(&case.sentinel))
        });
        assert!(
            at_debug,
            "{}'s sentinel must still be reachable at DEBUG, or this test cannot tell a real \
             fix from a line that was simply deleted; the events were {:?}",
            case.tool,
            recorded.event_summary()
        );
    }
}

/// AC (mcp-core#40): every tool handler is instrumented -- a span opens for
/// each, nested under mcp-core's own `mcp.tools.call` span. Table-driven so a
/// new tool without a matching span name here is visible, not silently
/// unchecked.
#[test]
fn each_tool_handler_opens_its_own_span() {
    let mock = start_mock_server();
    let recorded = capture_all_tool_calls(&mock.base_url());

    let expected_spans = [
        ("weather_geocode", "call_geocode"),
        ("weather_get_current", "call_get_current"),
        ("weather_get_forecast", "call_get_forecast"),
        ("weather_get_alerts", "call_get_alerts"),
    ];
    // A change to `expected_spans` above must track `support::tool_cases`;
    // if it drifts, `tool_cases_cover_every_registered_tool` still catches a
    // tool with no case at all, but this asserts the pairing explicitly too.
    let expected_tools: Vec<&str> = expected_spans.iter().map(|(tool, _)| *tool).collect();
    let mut cases: Vec<&str> = tool_cases().iter().map(|c| c.tool).collect();
    cases.sort_unstable();
    cases.dedup();
    let mut expected_sorted = expected_tools.clone();
    expected_sorted.sort_unstable();
    assert_eq!(
        cases, expected_sorted,
        "expected_spans above must name the same tools as support::tool_cases"
    );

    for (tool, span_name) in expected_spans {
        assert!(
            recorded.spans.iter().any(|span| span.name == span_name),
            "{tool} must open a {span_name:?} span; the spans were {:?}",
            recorded
                .spans
                .iter()
                .map(|span| span.name)
                .collect::<Vec<_>>()
        );
    }
}
