use std::collections::HashMap;

use super::{
    model::{decode_frame, frame_end, Event, Format},
    ModelRoute, RouteRegistry,
};

#[test]
fn decodes_fragmented_sse_and_waits_for_terminal_done_event() {
    let mut bytes = include_bytes!("fixtures/chatkit/chat_completions.sse").to_vec();
    let (end, width) = frame_end(&bytes).unwrap();
    let first = decode_frame(Format::ChatCompletions, &bytes[..end]).unwrap();
    let consumed = end + width;
    bytes.drain(..consumed);
    let (end, width) = frame_end(&bytes).unwrap();
    let second = decode_frame(Format::ChatCompletions, &bytes[..end]).unwrap();
    let consumed = end + width;
    bytes.drain(..consumed);
    let (end, width) = frame_end(&bytes).unwrap();
    let terminal = decode_frame(Format::ChatCompletions, &bytes[..end]).unwrap();
    let consumed = end + width;
    bytes.drain(..consumed);
    assert!(bytes.is_empty());
    assert_eq!(first, vec![Event::Text("Hello".into())]);
    assert_eq!(second, vec![Event::Text(" there.".into())]);
    assert_eq!(terminal, vec![Event::Completed(None)]);
}

#[test]
fn chatgpt_response_completes_only_on_terminal_event() {
    let mut frames = include_bytes!("fixtures/chatkit/responses.sse").split(|byte| *byte == b'\n');
    let mut delta = Vec::new();
    for line in frames.by_ref().take_while(|line| !line.is_empty()) {
        delta.extend_from_slice(line);
        delta.push(b'\n');
    }
    let delta = delta.strip_suffix(b"\n").unwrap_or(&delta);
    assert!(
        matches!(decode_frame(Format::Responses, delta).unwrap().as_slice(), [Event::Text(text)] if text == "Hello")
    );
    let mut completed = Vec::new();
    for line in frames.filter(|line| !line.is_empty()) {
        completed.extend_from_slice(line);
        completed.push(b'\n');
    }
    let completed = completed.strip_suffix(b"\n").unwrap_or(&completed);
    assert!(matches!(
        decode_frame(Format::Responses, completed)
            .unwrap()
            .as_slice(),
        [Event::Completed(_)]
    ));
}

#[test]
fn stale_model_catalog_cannot_replace_the_newer_source_routes() {
    let mut registry = RouteRegistry::default();
    let slow_request = registry.begin_refresh();
    let fast_request = registry.begin_refresh();
    let latest = HashMap::from([(
        "latest".to_string(),
        ModelRoute::Chatgpt {
            model_id: "account-model".to_string(),
            registration_id: "registration-1".to_string(),
        },
    )]);
    let stale = HashMap::from([(
        "stale".to_string(),
        ModelRoute::Api {
            provider_id: "provider".to_string(),
            model_id: "api-model".to_string(),
        },
    )]);

    assert!(registry.publish(fast_request, latest));
    assert!(!registry.publish(slow_request, stale));
    assert!(
        matches!(registry.routes.get("latest"), Some(ModelRoute::Chatgpt { model_id, registration_id }) if model_id == "account-model" && registration_id == "registration-1")
    );
    assert!(!registry.routes.contains_key("stale"));
}

#[test]
fn refreshing_model_catalog_invalidates_routes_from_the_previous_source() {
    let mut registry = RouteRegistry::default();
    let initial = registry.begin_refresh();
    assert!(registry.publish(
        initial,
        HashMap::from([(
            "api-route".to_string(),
            ModelRoute::Api {
                provider_id: "provider".into(),
                model_id: "model".into(),
            },
        )]),
    ));
    assert!(registry.routes.contains_key("api-route"));

    registry.begin_refresh();

    assert!(registry.routes.is_empty());
}
