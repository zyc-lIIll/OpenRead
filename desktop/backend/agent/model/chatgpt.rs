use std::collections::{HashMap, HashSet};

use reqwest::{Response, StatusCode};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::ai_verification::chatgpt::{self as auth, ChatGptService};

use super::{AgentState, ModelRoute, Request, ToolChoice, WebCitation};

pub(super) async fn send(
    app: &AppHandle,
    state: &AgentState,
    route: &ModelRoute,
    request: &Request,
) -> Result<(Response, bool), String> {
    let ModelRoute::Chatgpt {
        model_id,
        registration_id,
    } = route
    else {
        return Err("模型路由与 ChatGPT 适配器不匹配。".into());
    };
    if request.tools.iter().any(|tool| {
        tool.get("type").and_then(Value::as_str) != Some("function") && !is_search_tool(tool)
    }) {
        return Err("ChatGPT 适配器不支持当前请求中的工具类型。".into());
    }
    let service = app.state::<ChatGptService>().inner().clone();
    let expected_registration_id = registration_id.clone();
    let token = tauri::async_runtime::spawn_blocking(move || {
        auth::model_access_token_for(&service, &expected_registration_id)
    })
    .await
    .map_err(|_| "无法读取 ChatGPT 授权。".to_string())??
    .ok_or_else(|| "请先登录并授权使用 ChatGPT 额度。".to_string())?;

    let client = &state.client;
    let response = client
        .post("https://api.openai.com/v1/responses")
        .bearer_auth(&token)
        .json(&request_body(model_id, request, true))
        .send()
        .await
        .map_err(|_| "无法连接 ChatGPT。".to_string())?;
    if response.status() != StatusCode::BAD_REQUEST || !has_search_tool(request) {
        return Ok((response, false));
    }
    if request.tool_choice == ToolChoice::Required && !has_function_tool(request) {
        return Err("ChatGPT 路由无法提供当前必需的联网搜索工具。".into());
    }

    client
        .post("https://api.openai.com/v1/responses")
        .bearer_auth(token)
        .json(&request_body(model_id, request, false))
        .send()
        .await
        .map(|response| (response, true))
        .map_err(|_| "无法连接 ChatGPT。".to_string())
}

pub(super) fn request_body(model: &str, request: &Request, use_search: bool) -> Value {
    let mut body = json!({
        "model":model,
        "instructions":instructions(request, use_search),
        "input":responses_input(request),
        "store":request.store,
        "stream":request.stream
    });
    let functions = request
        .tools
        .iter()
        .filter(|tool| tool.get("type").and_then(Value::as_str) == Some("function"))
        .map(|tool| {
            json!({
                "type":"function",
                "name":tool["name"],
                "description":tool["description"],
                "parameters":tool["parameters"],
                "strict":false
            })
        })
        .collect::<Vec<_>>();
    let mut tools = Vec::new();
    if !functions.is_empty() {
        tools.push(json!({
            "type":"namespace",
            "name":"openread",
            "description":"Read the user's imported sources and select an explanation workflow.",
            "tools":functions
        }));
    }
    if use_search && has_search_tool(request) {
        tools.push(json!({"type":"web_search"}));
    }
    if !tools.is_empty() {
        body["tools"] = Value::Array(tools);
        body["tool_choice"] = json!(match request.tool_choice {
            ToolChoice::None => "none",
            ToolChoice::Auto => "auto",
            ToolChoice::Required => "required",
        });
    }
    if let Some(effort) = request.reasoning_effort.api_value() {
        body["reasoning"] = json!({"effort":effort});
    }
    body
}

fn instructions(request: &Request, use_search: bool) -> String {
    if !has_search_tool(request) || use_search {
        request.instructions.clone()
    } else {
        format!(
            "{}\n\n当前 ChatGPT 请求未提供联网搜索工具；本轮未执行搜索，不得声称已检索外部资料。",
            request.instructions
        )
    }
}

fn responses_input(request: &Request) -> Vec<Value> {
    let contexts = &request.context.0;
    let mut skipped_calls = HashSet::new();
    let mut skip_assistant_at = HashSet::new();
    let mut contexts_at = HashMap::<usize, Vec<&Vec<Value>>>::new();
    for context in contexts {
        contexts_at
            .entry(context.input_index)
            .or_default()
            .push(&context.output);
        let has_output_text = context.output.iter().any(|item| {
            item.get("type").and_then(Value::as_str) == Some("message")
                && item
                    .get("content")
                    .and_then(Value::as_array)
                    .is_some_and(|parts| {
                        parts.iter().any(|part| {
                            part.get("type").and_then(Value::as_str) == Some("output_text")
                        })
                    })
        });
        if has_output_text {
            skip_assistant_at.insert(context.input_index);
        }
        skipped_calls.extend(context.output.iter().filter_map(|item| {
            (item.get("type").and_then(Value::as_str) == Some("function_call"))
                .then(|| {
                    item.get("call_id")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .flatten()
        }));
    }

    let mut input = Vec::new();
    for index in 0..=request.input.len() {
        if let Some(outputs) = contexts_at.get(&index) {
            for output in outputs {
                input.extend(output.iter().cloned());
            }
        }
        let Some(item) = request.input.get(index) else {
            continue;
        };
        let role = item.get("role").and_then(Value::as_str);
        let call_id = item.get("call_id").and_then(Value::as_str);
        let is_replayed_function_call = item.get("type").and_then(Value::as_str)
            == Some("function_call")
            && call_id.is_some_and(|id| skipped_calls.contains(id));
        if skip_assistant_at.contains(&index) && role == Some("assistant")
            || is_replayed_function_call
        {
            continue;
        }
        input.push(item.clone());
    }
    input
}

fn has_search_tool(request: &Request) -> bool {
    request
        .tools
        .iter()
        .any(|tool| tool.get("type").and_then(Value::as_str) == Some("web_search"))
}

fn is_search_tool(tool: &Value) -> bool {
    tool.get("type").and_then(Value::as_str) == Some("web_search")
}

fn has_function_tool(request: &Request) -> bool {
    request
        .tools
        .iter()
        .any(|tool| tool.get("type").and_then(Value::as_str) == Some("function"))
}

pub(super) fn decode(value: &Value, event_name: Option<&str>) -> Result<Vec<super::Event>, String> {
    use super::Event;
    let name = value.get("type").and_then(Value::as_str).or(event_name);
    Ok(match name {
        Some("response.output_text.delta") => value
            .get("delta")
            .and_then(Value::as_str)
            .map(|text| vec![Event::Text(text.to_string())])
            .unwrap_or_default(),
        Some(
            "response.function_call_arguments.delta" | "response.function_call_arguments.done",
        ) => Vec::new(),
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

pub(super) fn tool_calls(output: &[Value]) -> Vec<super::ModelToolCall> {
    output
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("function_call"))
        .filter_map(|item| {
            Some(super::ModelToolCall {
                id: item.get("call_id")?.as_str()?.to_owned(),
                name: match item.get("namespace").and_then(Value::as_str) {
                    Some("openread") | None => item.get("name")?.as_str()?.to_owned(),
                    Some(namespace) => format!("{namespace}.{}", item.get("name")?.as_str()?),
                },
                arguments: item
                    .get("arguments")
                    .and_then(Value::as_str)
                    .unwrap_or("{}")
                    .to_owned(),
            })
        })
        .collect()
}

pub(super) fn citations(output: &[Value]) -> Vec<WebCitation> {
    let mut citations = Vec::new();
    let mut message_offset = 0;
    for item in output {
        let Some(parts) = item.get("content").and_then(Value::as_array) else {
            continue;
        };
        let part_text_len = parts
            .iter()
            .filter(|part| part.get("type").and_then(Value::as_str) == Some("output_text"))
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .map(|text| text.chars().count())
            .sum::<usize>();
        let mut part_offset = 0;
        for part in parts {
            let current_offset = part_offset;
            part_offset += part
                .get("text")
                .and_then(Value::as_str)
                .map(|text| text.chars().count())
                .unwrap_or(0);
            let Some(annotations) = part.get("annotations").and_then(Value::as_array) else {
                continue;
            };
            for annotation in annotations {
                if annotation.get("type").and_then(Value::as_str) != Some("url_citation") {
                    continue;
                }
                let Some(raw_url) = annotation.get("url").and_then(Value::as_str) else {
                    continue;
                };
                let Ok(url) = url::Url::parse(raw_url) else {
                    continue;
                };
                if !matches!(url.scheme(), "http" | "https") || url.host().is_none() {
                    continue;
                }
                citations.push(WebCitation {
                    title: annotation
                        .get("title")
                        .and_then(Value::as_str)
                        .unwrap_or(raw_url)
                        .to_owned(),
                    url: url.to_string(),
                    index: message_offset
                        + current_offset
                        + annotation
                            .get("end_index")
                            .and_then(Value::as_u64)
                            .unwrap_or(0) as usize,
                });
            }
        }
        message_offset += part_text_len;
    }
    citations
}

pub(super) fn error_rejects_images(body: &str) -> bool {
    super::error_rejects_images(body)
}

pub(super) fn request_error(status: u16, request: &Request, search_unavailable: bool) -> String {
    if status == StatusCode::BAD_REQUEST.as_u16()
        && request.reasoning_effort.api_value().is_some()
        && (search_unavailable || !has_search_tool(request))
    {
        return "模型请求失败（HTTP 400）。如果该模型不支持当前思考强度，请改用“自动”后重试。"
            .into();
    }
    super::http_error_message(status)
}
