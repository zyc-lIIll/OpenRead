use serde_json::{json, Value};

use super::{
    chatgpt, decode_frame, error_rejects_images, frame_end, other, Adapter, Context, Event,
    ModelToolCall, Request, ResponseContext, ToolChoice,
};
use crate::agent::{model, ReasoningEffort};

#[test]
fn normalizes_chat_completions_text_and_tool_deltas() {
    let frame = br#"data: {"choices":[{"delta":{"content":"Hi","tool_calls":[{"index":0,"id":"call-1","function":{"name":"read_source","arguments":"{\"page\":"}}]}}]}"#;
    let events = decode_frame(Adapter::Other, frame).unwrap();
    assert_eq!(
        events,
        vec![
            Event::Text("Hi".into()),
            Event::ToolCallDelta {
                index: 0,
                id: Some("call-1".into()),
                name: Some("read_source".into()),
                arguments: Some("{\"page\":".into())
            },
        ]
    );
}

#[test]
fn normalizes_responses_completion_and_finds_frame_boundaries() {
    let frame = br#"event: response.completed
data: {"type":"response.completed","response":{"output":[{"type":"reasoning"}]}}"#;
    assert!(
        matches!(decode_frame(Adapter::Chatgpt, frame).unwrap().as_slice(), [Event::Completed(Some(items))] if items[0]["type"] == "reasoning")
    );
    assert_eq!(frame_end(b"data: x\r\n\r\nrest"), Some((7, 4)));
}

#[test]
fn normalizes_output_length_as_a_continuable_turn() {
    let chat = br#"data: {"choices":[{"delta":{"content":"More"},"finish_reason":"length"}]}"#;
    assert_eq!(
        decode_frame(Adapter::Other, chat).unwrap(),
        vec![Event::Text("More".into()), Event::Truncated(None)]
    );

    let responses = br#"event: response.incomplete
data: {"type":"response.incomplete","response":{"incomplete_details":{"reason":"max_output_tokens"},"output":[{"type":"message"}]}}"#;
    assert!(matches!(
        decode_frame(Adapter::Chatgpt, responses).unwrap().as_slice(),
        [Event::Truncated(Some(items))] if items[0]["type"] == "message"
    ));

    let filtered = br#"data: {"choices":[{"finish_reason":"content_filter"}]}"#;
    assert!(decode_frame(Adapter::Other, filtered).is_err());
}

#[test]
fn chatgpt_normalizes_tool_calls_citations_and_opaque_context() {
    let response = json!({"output":[
        {"type":"reasoning","encrypted_content":"opaque"},
        {"type":"message","content":[{"type":"output_text","text":"Reference" ,"annotations":[
            {"type":"url_citation","title":"Source","url":"https://example.com","end_index":9},
            {"type":"url_citation","title":"Local","url":"file:///tmp/no","end_index":9}
        ]}]},
        {"type":"function_call","id":"fc_item_1","call_id":"call_1","namespace":"openread","name":"read_source","arguments":"{}"}
    ]});
    let output = response["output"].as_array().unwrap();
    assert_eq!(
        chatgpt::tool_calls(output).unwrap(),
        vec![ModelToolCall {
            id: "call_1".into(),
            name: "read_source".into(),
            arguments: "{}".into()
        }]
    );
    assert_eq!(
        chatgpt::citations(output),
        vec![model::WebCitation {
            title: "Source".into(),
            url: "https://example.com/".into(),
            index: 9
        }]
    );
}

#[test]
fn chatgpt_rejects_function_calls_without_complete_arguments() {
    let output = json!([{
        "type":"function_call",
        "call_id":"call_1",
        "name":"organize_output"
    }]);
    assert!(chatgpt::tool_calls(output.as_array().unwrap()).is_err());
}

#[test]
fn adapters_convert_the_same_unified_messages_and_tools() {
    let call = ModelToolCall {
        id: "call-1".into(),
        name: "read_source".into(),
        arguments: "{}".into(),
    };
    let request = Request::new(
        super::test_route(),
        "instructions".into(),
        vec![
            model::text_message("user", "Explain"),
            model::text_message("assistant", "Reading"),
            model::function_call(&call),
            model::function_call_output("call-1", "page text"),
        ],
        vec![
            json!({"type":"function","name":"read_source","description":"Read PDF","parameters":{"type":"object","properties":{}}}),
            model::web_search_tool(),
        ],
        ToolChoice::Auto,
        ReasoningEffort::Medium,
        Context::default(),
    );

    let responses = chatgpt::request_body("chatgpt-model", &request, true);
    assert_eq!(responses["store"], false);
    assert_eq!(responses["input"][2]["type"], "function_call");
    assert_eq!(responses["input"][3]["type"], "function_call_output");
    assert_eq!(responses["tools"][0]["type"], "namespace");
    assert_eq!(responses["tools"][0]["tools"][0]["name"], "read_source");
    assert_eq!(responses["tools"][1]["type"], "web_search");
    assert_eq!(responses["reasoning"]["effort"], "medium");

    let chat = other::request_body("api-model", &request).unwrap();
    assert_eq!(
        chat["messages"][2]["tool_calls"][0]["function"]["name"],
        "read_source"
    );
    assert_eq!(chat["messages"][3]["role"], "tool");
    assert_eq!(chat["tools"][0]["function"]["name"], "read_source");
    assert!(chat["messages"][0]["content"]
        .as_str()
        .unwrap()
        .contains("没有可用的联网搜索实现"));
    assert!(chat["tools"]
        .as_array()
        .unwrap()
        .iter()
        .all(|tool| tool["type"] != "web_search"));
    assert!(chat.get("reasoning_effort").is_none());
}

#[test]
fn responses_context_is_inserted_without_leaking_into_unified_input() {
    let request = Request::new(
        super::test_route(),
        "instructions".into(),
        vec![
            model::text_message("user", "Explain"),
            model::text_message("assistant", "Reading"),
            json!({"type":"function_call","call_id":"call-1","name":"read_source","arguments":"{}"}),
            model::function_call_output("call-1", "page text"),
        ],
        vec![],
        ToolChoice::None,
        ReasoningEffort::Auto,
        Context(vec![ResponseContext {
            input_index: 1,
            output: vec![
                json!({"type":"reasoning","encrypted_content":"opaque"}),
                json!({"type":"message","content":[{"type":"output_text","text":"Reading"}]}),
                json!({"type":"function_call","call_id":"call-1","name":"read_source","arguments":"{}"}),
            ],
        }]),
    );
    let body = chatgpt::request_body("model", &request, false);
    assert_eq!(body["input"].as_array().unwrap().len(), 5);
    assert_eq!(body["input"][1]["type"], "reasoning");
    assert_eq!(body["input"][2]["type"], "message");
    assert_eq!(body["input"][3]["type"], "function_call");
    assert_eq!(body["input"][4]["type"], "function_call_output");
    assert_eq!(request.input[1]["role"], "assistant");
    assert_eq!(request.input[2]["type"], "function_call");
}

#[test]
fn other_adapter_rejects_required_unavailable_search_and_maps_images() {
    let only_search = Request::new(
        super::test_route(),
        "instructions".into(),
        vec![],
        vec![model::web_search_tool()],
        ToolChoice::Required,
        ReasoningEffort::Auto,
        Context::default(),
    );
    assert!(other::request_body("model", &only_search).is_err());

    let image = super::super::tool::source::pdf::PageImage {
        page_number: 2,
        data_url: "data:image/png;base64,AA==".into(),
    };
    let request = Request::new(
        super::test_route(),
        "instructions".into(),
        vec![model::user_message("Read", &[image])],
        vec![],
        ToolChoice::None,
        ReasoningEffort::Auto,
        Context::default(),
    );
    assert_eq!(
        other::request_body("model", &request).unwrap()["messages"][1]["content"][2]["image_url"]
            ["url"],
        "data:image/png;base64,AA=="
    );
}

#[test]
fn image_error_detection_does_not_treat_missing_models_as_vision_failures() {
    assert!(error_rejects_images(
        r#"{"error":{"message":"model does not support image inputs"}}"#
    ));
    assert!(error_rejects_images("Request entity too large"));
    assert!(!error_rejects_images(
        r#"{"error":{"message":"model was not found"}}"#
    ));
    assert!(!error_rejects_images(
        r#"{"error":{"code":"model_not_found","message":"vision model was not found"}}"#
    ));
}

#[test]
fn shared_tool_schemas_use_provider_neutral_function_definitions() {
    let tools = crate::agent::tool::shared_registry().schemas();
    assert_eq!(tools.len(), 3);
    assert!(tools.iter().all(|tool| tool["type"] == "function"));
    let request = Request::new(
        super::test_route(),
        String::new(),
        vec![],
        tools,
        ToolChoice::Auto,
        ReasoningEffort::Auto,
        Context::default(),
    );
    let body = other::request_body("model", &request).unwrap();
    assert!(body["tools"]
        .as_array()
        .unwrap()
        .iter()
        .all(|tool| tool["function"].get("type").is_none()));
}

#[test]
fn resolved_route_survives_model_catalog_refresh() {
    let mut registry = crate::agent::RouteRegistry::default();
    let generation = registry.begin_refresh();
    assert!(registry.publish(
        generation,
        std::collections::HashMap::from([(
            "selected".into(),
            crate::agent::ModelRoute::Api {
                provider_id: "provider".into(),
                model_id: "model".into(),
            },
        )]),
    ));
    let route = super::Route(registry.routes.get("selected").unwrap().clone());

    registry.begin_refresh();

    assert!(registry.routes.is_empty());
    assert_eq!(route.adapter(), Adapter::Other);
}

#[test]
fn only_image_requests_with_relevant_client_errors_read_error_bodies() {
    assert!(super::should_read_image_error_body(400, true));
    assert!(super::should_read_image_error_body(415, true));
    assert!(super::should_read_image_error_body(422, true));
    assert!(!super::should_read_image_error_body(413, true));
    assert!(!super::should_read_image_error_body(401, true));
    assert!(!super::should_read_image_error_body(429, true));
    assert!(!super::should_read_image_error_body(400, false));
}

#[test]
fn chatgpt_search_fallback_adds_an_unavailable_instruction() {
    let request = Request::new(
        super::test_route(),
        "instructions".into(),
        vec![],
        vec![model::web_search_tool()],
        ToolChoice::Auto,
        ReasoningEffort::Auto,
        Context::default(),
    );
    let body = chatgpt::request_body("model", &request, false);
    assert!(body["instructions"]
        .as_str()
        .unwrap()
        .contains("本轮未执行搜索"));
    assert!(body.get("tools").is_none());
}

#[test]
fn input_types_match_the_internal_request_contract() {
    let value: Value = model::function_call_output("call-2", "done");
    assert_eq!(value["type"], "function_call_output");
    assert_eq!(value["call_id"], "call-2");
    assert_eq!(value["output"], "done");
}
