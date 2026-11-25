use std::collections::HashMap;

use tokio::sync::mpsc;

use super::{
    handle_sse_frame, next_sse_frame, CompletionEvent, ModelRoute, RouteRegistry, StreamKind,
};

#[test]
fn decodes_fragmented_sse_and_waits_for_terminal_done_event() {
    let mut bytes = include_bytes!("fixtures/chatkit/chat_completions.sse").to_vec();
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let (frame, consumed) = next_sse_frame(&bytes).unwrap();
    assert!(!handle_sse_frame(&frame, &StreamKind::ChatCompletions, &sender).unwrap());
    bytes.drain(..consumed);
    let (frame, consumed) = next_sse_frame(&bytes).unwrap();
    assert!(!handle_sse_frame(&frame, &StreamKind::ChatCompletions, &sender).unwrap());
    bytes.drain(..consumed);
    let (frame, consumed) = next_sse_frame(&bytes).unwrap();
    assert!(handle_sse_frame(&frame, &StreamKind::ChatCompletions, &sender).unwrap());
    bytes.drain(..consumed);
    assert!(bytes.is_empty());
    assert!(
        matches!(receiver.try_recv(), Ok(CompletionEvent::Delta(ref value)) if value == "Hello")
    );
    assert!(
        matches!(receiver.try_recv(), Ok(CompletionEvent::Delta(ref value)) if value == " there.")
    );
}

#[test]
fn chatgpt_response_completes_only_on_terminal_event() {
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let mut frames = include_bytes!("fixtures/chatkit/responses.sse").split(|byte| *byte == b'\n');
    let mut delta = Vec::new();
    for line in frames.by_ref().take_while(|line| !line.is_empty()) {
        delta.extend_from_slice(line);
        delta.push(b'\n');
    }
    let delta = delta.strip_suffix(b"\n").unwrap_or(&delta);
    assert!(!handle_sse_frame(delta, &StreamKind::Responses, &sender).unwrap());
    let mut completed = Vec::new();
    for line in frames.filter(|line| !line.is_empty()) {
        completed.extend_from_slice(line);
        completed.push(b'\n');
    }
    let completed = completed.strip_suffix(b"\n").unwrap_or(&completed);
    assert!(handle_sse_frame(completed, &StreamKind::Responses, &sender).unwrap());
    assert!(
        matches!(receiver.try_recv(), Ok(CompletionEvent::Delta(ref value)) if value == "Hello")
    );
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
