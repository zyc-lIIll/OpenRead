use serde_json::{json, Value};

use crate::agent::tool::source::pdf::PageImage;
use crate::agent::ModelMessage;

use super::Event;

pub fn request_body(
    model: &str,
    messages: &[ModelMessage],
    tools: &[Value],
    reasoning_effort: Option<&str>,
    use_search: bool,
    require_tool: bool,
) -> Value {
    let mut instructions = String::new();
    let mut input = Vec::new();
    for message in messages {
        if message.role == "system" {
            if !instructions.is_empty() {
                instructions.push('\n');
            }
            instructions.push_str(&message.content);
        } else if !message.responses_items.is_empty() {
            input.extend(message.responses_items.iter().cloned());
        } else if message.role == "tool" {
            input.push(json!({"type":"function_call_output","call_id":message.tool_call_id,"output":message.content}));
        } else {
            input.push(json!({"role":message.role,"content":[{"type":"input_text","text":message.content}]}));
        }
    }
    let mut body = json!({"model":model,"instructions":instructions,"input":input,"store":false,"stream":true});
    if !tools.is_empty() || use_search {
        let functions = tools
            .iter()
            .map(|tool| {
                json!({"type":"function","name":tool["name"],"description":tool["description"],"parameters":tool["parameters"],"strict":false})
            })
            .collect::<Vec<_>>();
        let mut registered = Vec::new();
        if !functions.is_empty() {
            registered.push(json!({"type":"namespace","name":"openread","description":"Read the user's imported sources and select an explanation workflow.","tools":functions}));
        }
        if use_search {
            registered.push(json!({"type":"web_search"}));
        }
        body["tools"] = Value::Array(registered);
        body["tool_choice"] = json!(if require_tool { "required" } else { "auto" });
    }
    if let Some(effort) = reasoning_effort {
        body["reasoning"] = json!({"effort":effort});
    }
    body
}

pub fn initial_request_body(
    model: &str,
    instructions: &str,
    input: &str,
    images: &[PageImage],
    reasoning_effort: Option<&str>,
    use_search: bool,
) -> Value {
    let mut body = json!({"model":model,"instructions":instructions,"input":[{"role":"user","content":input_content(input,images)}],"store":false,"stream":true});
    if use_search {
        body["tools"] = json!([{ "type":"web_search" }]);
        body["tool_choice"] = json!("auto");
    }
    if let Some(effort) = reasoning_effort {
        body["reasoning"] = json!({"effort":effort});
    }
    body
}

pub fn input_content(input: &str, images: &[PageImage]) -> Vec<Value> {
    let mut content = vec![json!({"type":"input_text","text":input})];
    for image in images {
        content.push(
            json!({"type":"input_text","text":format!("【第 {} 页页面图像】",image.page_number)}),
        );
        content.push(json!({"type":"input_image","image_url":image.data_url,"detail":"auto"}));
    }
    content
}

pub fn tool_calls(output: &[Value]) -> Vec<super::ModelToolCall> {
    output
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("function_call"))
        .filter_map(|item| {
            Some(super::ModelToolCall {
                id: item.get("call_id")?.as_str()?.to_string(),
                name: match item.get("namespace").and_then(Value::as_str) {
                    Some("openread") | None => item.get("name")?.as_str()?.to_string(),
                    Some(namespace) => format!("{namespace}.{}", item.get("name")?.as_str()?),
                },
                arguments: item
                    .get("arguments")
                    .and_then(Value::as_str)
                    .unwrap_or("{}")
                    .to_string(),
            })
        })
        .collect()
}

pub(super) fn decode(value: &Value, event_name: Option<&str>) -> Result<Vec<Event>, String> {
    let name = value.get("type").and_then(Value::as_str).or(event_name);
    Ok(match name {
        Some("response.output_text.delta") => value
            .get("delta")
            .and_then(Value::as_str)
            .map(|text| vec![Event::Text(text.to_string())])
            .unwrap_or_default(),
        Some("response.function_call_arguments.delta") => Vec::new(),
        // This event's item_id identifies the output item; function_call_output needs call_id.
        // Wait for response.completed, whose output contains the complete callable item.
        Some("response.function_call_arguments.done") => Vec::new(),
        Some(name) if name.starts_with("response.web_search_call.") => vec![Event::SearchProgress],
        Some("response.completed") => {
            vec![Event::Completed(value.pointer("/response/output").cloned())]
        }
        Some("response.incomplete")
            if value
                .pointer("/response/incomplete_details/reason")
                .and_then(Value::as_str)
                == Some("max_output_tokens") =>
        {
            vec![Event::Truncated(value.pointer("/response/output").cloned())]
        }
        Some("response.failed" | "response.incomplete" | "error") => vec![Event::Failed(
            value
                .pointer("/response/error/code")
                .and_then(Value::as_str)
                .map(str::to_owned),
        )],
        _ => Vec::new(),
    })
}
