use serde_json::json;

use super::{
    append_web_citation_context, control_call, first_turn_tool_allowed,
    has_conflicting_stage_controls, output_route, request_tools, OutputRoute, Stage,
    CONTINUE_COLLECTION, CONTINUE_OUTPUT, ORGANIZE_OUTPUT, SELECT_WORKFLOW,
};
use crate::agent::{
    explanation, model,
    tool::{self, ToolRegistry},
};

fn registry() -> ToolRegistry {
    let mut registry = tool::shared_registry();
    registry.extend([explanation::tools::select_workflow_tool()]);
    registry
}

#[test]
fn explanation_can_cycle_between_collection_and_writing_more_than_once() {
    let mut stage = Stage::Collect;
    for _ in 0..2 {
        assert_eq!(output_route(stage), OutputRoute::Activity);
        stage = control_call(stage, ORGANIZE_OUTPUT, "{}")
            .unwrap()
            .unwrap()
            .next_stage;
        assert_eq!(stage, Stage::Compose);
        assert_eq!(output_route(stage), OutputRoute::Formal);
        assert_eq!(
            control_call(stage, CONTINUE_OUTPUT, "{}")
                .unwrap()
                .unwrap()
                .next_stage,
            Stage::Compose
        );
        stage = control_call(stage, CONTINUE_COLLECTION, "{}")
            .unwrap()
            .unwrap()
            .next_stage;
    }
    assert_eq!(stage, Stage::Collect);
}

#[test]
fn request_output_route_is_fixed_before_a_late_control_call() {
    let requested_route = output_route(Stage::Collect);
    let collected = "I found the relevant pages.";
    assert_eq!(requested_route, OutputRoute::Activity);
    assert!(!collected.is_empty());

    let next = control_call(Stage::Collect, ORGANIZE_OUTPUT, "{}")
        .unwrap()
        .unwrap()
        .next_stage;
    assert_eq!(next, Stage::Compose);
    assert_eq!(output_route(next), OutputRoute::Formal);
}

#[test]
fn control_calls_are_validated_and_only_switch_in_their_stage() {
    assert!(control_call(Stage::Collect, ORGANIZE_OUTPUT, r#"{"extra":true}"#).is_err());
    assert!(control_call(Stage::Compose, CONTINUE_COLLECTION, "[]").is_err());
    assert!(control_call(Stage::Compose, CONTINUE_OUTPUT, r#"{"extra":true}"#).is_err());
    assert!(control_call(Stage::Compose, ORGANIZE_OUTPUT, "{}")
        .unwrap()
        .is_none());
    assert!(control_call(Stage::Collect, CONTINUE_OUTPUT, "{}")
        .unwrap()
        .is_none());
    assert!(control_call(Stage::Collect, CONTINUE_COLLECTION, "{}")
        .unwrap()
        .is_none());
}

#[test]
fn conflicting_compose_controls_are_detected() {
    let calls = [
        model::ModelToolCall {
            id: "continue-output".into(),
            name: CONTINUE_OUTPUT.into(),
            arguments: "{}".into(),
        },
        model::ModelToolCall {
            id: "continue-collection".into(),
            name: CONTINUE_COLLECTION.into(),
            arguments: "{}".into(),
        },
    ];
    assert!(has_conflicting_stage_controls(Stage::Compose, &calls));
    assert!(!has_conflicting_stage_controls(Stage::Collect, &calls));
}

#[test]
fn each_explanation_request_uses_its_own_tool_set() {
    let registry = registry();
    let (first_allowed, first) = request_tools(&registry, Stage::Collect, true, false).unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0]["name"], "select_workflow");
    assert!(first[0]["parameters"]["properties"]["suggest_new_explanation"].is_object());
    assert_eq!(first_allowed.schemas(), first);

    let (collect_allowed, collect) =
        request_tools(&registry, Stage::Collect, false, false).unwrap();
    let names = collect
        .iter()
        .filter_map(|tool| tool.get("name").and_then(serde_json::Value::as_str))
        .collect::<Vec<_>>();
    assert!(names.contains(&"read_source"));
    assert!(names.contains(&ORGANIZE_OUTPUT));
    assert!(collect.iter().any(|tool| tool["type"] == "web_search"));
    assert!(!names.contains(&"select_workflow"));
    assert!(!names.contains(&CONTINUE_COLLECTION));
    assert!(collect_allowed.find("read_source").is_some());
    assert!(collect_allowed.find("select_workflow").is_none());

    let (compose_allowed, compose) =
        request_tools(&registry, Stage::Compose, false, false).unwrap();
    assert_eq!(compose.len(), 2);
    assert_eq!(compose[0]["name"], CONTINUE_OUTPUT);
    assert_eq!(compose[1]["name"], CONTINUE_COLLECTION);
    assert!(compose_allowed.schemas().is_empty());
}

#[test]
fn first_turn_cannot_run_controls_or_repeat_workflow_selection() {
    assert!(first_turn_tool_allowed(SELECT_WORKFLOW, false));
    assert!(!first_turn_tool_allowed(ORGANIZE_OUTPUT, false));
    assert!(!first_turn_tool_allowed(SELECT_WORKFLOW, true));
}

#[test]
fn forced_organize_turn_exposes_no_collection_tools() {
    let registry = registry();
    let (allowed, tools) = request_tools(&registry, Stage::Collect, false, true).unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["name"], ORGANIZE_OUTPUT);
    assert!(allowed.schemas().is_empty());
}

#[test]
fn tool_registry_rejects_an_unregistered_selection() {
    let registry = tool::shared_registry();
    assert!(registry.select(&["read_source"]).is_ok());
    assert!(registry.select(&["not_registered"]).is_err());
}

#[test]
fn web_source_metadata_is_added_as_untrusted_ordered_input() {
    let mut input = vec![model::text_message("user", "Explain the evidence.")];
    append_web_citation_context(
        &mut input,
        &[json!({"id":"w1","title":"source","url":"https://example.com"})],
    );
    assert_eq!(input[1]["role"], "user");
    assert!(input[1]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("https://example.com"));
}

#[test]
fn output_history_uses_complete_function_call_and_result_items() {
    let call = model::ModelToolCall {
        id: "call-1".into(),
        name: "read_source".into(),
        arguments: json!({"page_start":1}).to_string(),
    };
    let mut input = vec![model::text_message("user", "Explain this section.")];
    input.push(model::function_call(&call));
    input.push(model::function_call_output(&call.id, "source text"));
    assert_eq!(input[1]["type"], "function_call");
    assert_eq!(input[2]["type"], "function_call_output");
    assert_eq!(input[2]["call_id"], "call-1");
}
