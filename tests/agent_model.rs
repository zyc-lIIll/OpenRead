use serde_json::json;

use super::{decode_frame, frame_end, responses, Event, Format, ModelToolCall};
use crate::agent::ModelMessage;

#[test]
fn normalizes_chat_completions_text_and_tool_deltas() {
    let frame = br#"data: {"choices":[{"delta":{"content":"Hi","tool_calls":[{"index":0,"id":"call-1","function":{"name":"read_source","arguments":"{\"page\":"}}]}}]}"#;
    let events = decode_frame(Format::ChatCompletions, frame).unwrap();
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
        matches!(decode_frame(Format::Responses, frame).unwrap().as_slice(), [Event::Completed(Some(items))] if items[0]["type"] == "reasoning")
    );
    assert_eq!(frame_end(b"data: x\r\n\r\nrest"), Some((7, 4)));
}

#[test]
fn normalizes_output_length_as_a_continuable_turn() {
    let chat = br#"data: {"choices":[{"delta":{"content":"More"},"finish_reason":"length"}]}"#;
    assert_eq!(
        decode_frame(Format::ChatCompletions, chat).unwrap(),
        vec![Event::Text("More".into()), Event::Truncated(None)]
    );

    let responses = br#"event: response.incomplete
data: {"type":"response.incomplete","response":{"incomplete_details":{"reason":"max_output_tokens"},"output":[{"type":"message"}]}}"#;
    assert!(matches!(
        decode_frame(Format::Responses, responses).unwrap().as_slice(),
        [Event::Truncated(Some(items))] if items[0]["type"] == "message"
    ));

    let filtered = br#"data: {"choices":[{"finish_reason":"content_filter"}]}"#;
    assert!(decode_frame(Format::ChatCompletions, filtered).is_err());
}

#[test]
fn responses_tool_call_uses_the_terminal_output_call_id() {
    let arguments_done = br#"event: response.function_call_arguments.done
data: {"type":"response.function_call_arguments.done","item_id":"fc_item_1","output_index":0,"arguments":"{\"workflow\":\"source_explanation\"}"}"#;
    assert!(decode_frame(Format::Responses, arguments_done)
        .unwrap()
        .is_empty());

    let completed = br#"event: response.completed
data: {"type":"response.completed","response":{"output":[{"type":"function_call","id":"fc_item_1","call_id":"call_1","namespace":"openread","name":"select_workflow","arguments":"{\"workflow\":\"source_explanation\"}"}]}}"#;
    let decoded = decode_frame(Format::Responses, completed).unwrap();
    let [Event::Completed(Some(output))] = decoded.as_slice() else {
        panic!("response.completed must retain the complete tool call")
    };
    let calls = responses::tool_calls(output.as_array().unwrap());
    let [call] = calls.as_slice() else {
        panic!("complete function_call should become one normalized tool call")
    };
    assert_eq!(call.id, "call_1");
    assert_eq!(call.name, "select_workflow");
}

#[test]
fn provider_adapters_keep_tools_and_stateless_responses_context() {
    let system = ModelMessage::system("instructions".into());
    let user = ModelMessage {
        role: "user".into(),
        content: "Explain".into(),
        tool_calls: vec![],
        tool_call_id: None,
        responses_items: vec![],
    };
    let assistant = ModelMessage {
        role: "assistant".into(),
        content: "Reading".into(),
        tool_calls: vec![ModelToolCall {
            id: "call-1".into(),
            name: "read_source".into(),
            arguments: "{}".into(),
        }],
        tool_call_id: None,
        responses_items: vec![
            json!({"type":"reasoning","encrypted_content":"opaque"}),
            json!({"type":"function_call","call_id":"call-1","name":"read_source","arguments":"{}"}),
        ],
    };
    let result = ModelMessage::tool_result("call-1".into(), "page text".into());
    let tools = vec![
        json!({"name":"read_source","description":"Read PDF","parameters":{"type":"object","properties":{},"required":[],"additionalProperties":false}}),
    ];
    let responses = responses::request_body(
        "model",
        &[system.clone(), user.clone(), assistant.clone(), result],
        &tools,
        Some("medium"),
        true,
        false,
    );
    assert_eq!(responses["store"], false);
    assert_eq!(responses["input"][1]["type"], "reasoning");
    assert_eq!(responses["input"][2]["type"], "function_call");
    assert_eq!(responses["input"][3]["type"], "function_call_output");
    assert_eq!(responses["tools"][0]["type"], "namespace");
    assert_eq!(responses["tools"][0]["name"], "openread");
    assert_eq!(responses["tools"][0]["tools"][0]["name"], "read_source");
    assert_eq!(responses["tools"][1]["type"], "web_search");

    let chat =
        super::chat_completions::request_body("model", &[system, user, assistant], &tools, false);
    assert_eq!(
        chat["messages"][2]["tool_calls"][0]["function"]["name"],
        "read_source"
    );
    assert_eq!(chat["tools"][0]["function"]["name"], "read_source");
}

#[test]
fn shared_tool_schemas_are_provider_neutral() {
    let tools = crate::agent::tool::schemas();
    assert_eq!(tools.len(), 3);
    assert!(tools.iter().all(|tool| tool.get("type").is_none()));
    let request = super::chat_completions::request_body("model", &[], &tools, false);
    assert!(request["tools"]
        .as_array()
        .unwrap()
        .iter()
        .all(|tool| tool["function"].get("type").is_none()));
}

#[test]
fn first_turn_forces_workflow_selection_without_source_or_search_tools() {
    let workflow = json!({
        "name":"select_workflow",
        "description":"Classify this message",
        "parameters":{"type":"object","properties":{},"required":[],"additionalProperties":false}
    });
    let responses = responses::request_body("model", &[], &[workflow.clone()], None, false, true);
    assert_eq!(responses["tool_choice"], "required");
    assert_eq!(responses["tools"].as_array().unwrap().len(), 1);
    assert_eq!(responses["tools"][0]["type"], "namespace");
    assert_eq!(responses["tools"][0]["tools"][0]["name"], "select_workflow");

    let chat = super::chat_completions::request_body("model", &[], &[workflow], true);
    assert_eq!(chat["tool_choice"], "required");
    assert_eq!(chat["tools"].as_array().unwrap().len(), 1);
    assert_eq!(chat["tools"][0]["function"]["name"], "select_workflow");
}
