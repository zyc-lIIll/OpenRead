use axum::http::{header, HeaderMap, HeaderValue};
use serde_json::Value;

use super::{
    allowed_origin, assistant_item, authorized, content_part_added_event, content_part_delta_event,
    content_part_done_event, preflight_response, user_message, user_text,
};

const MODEL_ID: &str = "f3d27c59-1f17-4124-860a-0e1f706294a8";

#[test]
fn accepts_chatkit_create_and_follow_up_fixtures_with_selected_model() {
    let create: Value =
        serde_json::from_str(include_str!("fixtures/chatkit/threads.create.json")).unwrap();
    let follow_up: Value = serde_json::from_str(include_str!(
        "fixtures/chatkit/threads.add_user_message.json"
    ))
    .unwrap();

    assert_eq!(create["type"], "threads.create");
    assert_eq!(follow_up["type"], "threads.add_user_message");
    for request in [&create, &follow_up] {
        let input = &request["params"]["input"];
        assert_eq!(input["inference_options"]["model"], MODEL_ID);
        assert_eq!(
            user_text(input).as_deref(),
            Some(if request["type"] == "threads.create" {
                "Summarize this topic."
            } else {
                "Continue."
            })
        );
    }
    let stored = user_message("thread_test", "Continue.", MODEL_ID);
    assert_eq!(stored["inference_options"]["model"], MODEL_ID);
}

#[test]
fn requires_allowed_origin_and_per_launch_bearer_token() {
    assert!(allowed_origin("http://tauri.localhost"));
    assert!(allowed_origin("tauri://localhost"));
    assert!(!allowed_origin("http://evil.localhost"));

    let mut headers = HeaderMap::new();
    assert!(!authorized(&headers, "launch-secret"));
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer wrong"),
    );
    assert!(!authorized(&headers, "launch-secret"));
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer launch-secret"),
    );
    assert!(authorized(&headers, "launch-secret"));
}

#[test]
fn preflight_allows_the_headers_requested_by_chatkit() {
    let requested = include_str!("fixtures/chatkit/preflight.headers");
    let response = preflight_response("http://tauri.localhost");
    let allowed = response
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_HEADERS)
        .unwrap()
        .to_str()
        .unwrap()
        .split(',')
        .map(|name| name.trim().to_ascii_lowercase())
        .collect::<Vec<_>>();

    for requested in requested.trim().split(',') {
        assert!(allowed.contains(&requested.trim().to_ascii_lowercase()));
    }
}

#[test]
fn assistant_stream_adds_content_before_delta_and_finishes_in_order() {
    let actual = [
        content_part_added_event("message_fixture"),
        content_part_delta_event("message_fixture", "Hello"),
        content_part_done_event("message_fixture", "Hello"),
        serde_json::json!({
            "type": "thread.item.done",
            "item": assistant_item(
                "message_fixture",
                "thread_fixture",
                "2026-01-01T00:00:00Z",
                "Hello"
            )
        }),
    ];
    let fixture = include_str!("fixtures/chatkit/assistant_stream.sse");
    let expected: Vec<Value> = fixture
        .split("\n\n")
        .filter_map(|frame| frame.strip_prefix("data: "))
        .map(|data| serde_json::from_str(data).unwrap())
        .collect();

    assert_eq!(actual.as_slice(), expected);
    assert_eq!(
        actual
            .iter()
            .map(|event| {
                event
                    .pointer("/update/type")
                    .and_then(Value::as_str)
                    .or_else(|| event.get("type").and_then(Value::as_str))
                    .unwrap()
            })
            .collect::<Vec<_>>(),
        [
            "assistant_message.content_part.added",
            "assistant_message.content_part.text_delta",
            "assistant_message.content_part.done",
            "thread.item.done",
        ]
    );
}
