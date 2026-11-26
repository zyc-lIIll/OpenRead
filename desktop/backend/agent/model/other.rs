use reqwest::Response;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::providers;

use super::{AgentState, ModelRoute, Request, ToolChoice};

pub(super) async fn send(
    app: &AppHandle,
    state: &AgentState,
    route: &ModelRoute,
    request: &Request,
) -> Result<(Response, bool), String> {
    let ModelRoute::Api {
        provider_id,
        model_id,
    } = route
    else {
        return Err("模型路由与通用 API 适配器不匹配。".into());
    };
    let provider = app
        .state::<providers::Store>()
        .agent_provider_model(provider_id, model_id)?;
    let body = request_body(model_id, request)?;
    let response = state
        .client
        .post(format!(
            "{}/chat/completions",
            provider.base_url.trim_end_matches('/')
        ))
        .bearer_auth(provider.api_key)
        .json(&body)
        .send()
        .await
        .map_err(|_| "无法连接所选 API 服务商。".to_string())?;
    let search_unavailable = request.tools.iter().any(is_search_tool);
    Ok((response, search_unavailable))
}

pub(super) fn request_body(model: &str, request: &Request) -> Result<Value, String> {
    if request
        .tools
        .iter()
        .any(|tool| !is_function_tool(tool) && !is_search_tool(tool))
    {
        return Err("通用 API 适配器不支持当前请求中的工具类型。".into());
    }
    let search_requested = request.tools.iter().any(is_search_tool);
    let functions = request
        .tools
        .iter()
        .filter(|tool| is_function_tool(tool))
        .collect::<Vec<_>>();
    if request.tool_choice == ToolChoice::Required && functions.is_empty() {
        return Err("所选 API 路由无法提供当前必需的工具。".into());
    }
    let mut messages =
        vec![json!({"role":"system","content":instructions(request, search_requested)})];
    let mut index = 0;
    while index < request.input.len() {
        let item = &request.input[index];
        match item.get("role").and_then(Value::as_str) {
            Some(role) => {
                let mut next = index + 1;
                let calls = if role == "assistant" {
                    let calls = read_function_calls(&request.input, &mut next)?;
                    calls
                } else {
                    Vec::new()
                };
                let mut message = json!({"role":role,"content":chat_content(item)?});
                if !calls.is_empty() {
                    message["tool_calls"] = Value::Array(calls);
                }
                messages.push(message);
                index = next;
            }
            _ if item.get("type").and_then(Value::as_str) == Some("function_call") => {
                let calls = read_function_calls(&request.input, &mut index)?;
                messages.push(json!({"role":"assistant","content":Value::Null,"tool_calls":calls}));
            }
            _ if item.get("type").and_then(Value::as_str) == Some("function_call_output") => {
                messages.push(json!({
                    "role":"tool",
                    "tool_call_id":item.get("call_id").and_then(Value::as_str).ok_or("函数结果缺少调用 ID。")?,
                    "content":item.get("output").and_then(Value::as_str).ok_or("函数结果缺少输出内容。")?
                }));
                index += 1;
            }
            _ => return Err("统一请求包含通用 API 路由不支持的 input 项。".into()),
        }
    }

    let mut body = json!({"model":model,"messages":messages,"stream":request.stream});
    if !functions.is_empty() {
        body["tools"] = Value::Array(
            functions
                .iter()
                .map(|tool| {
                    json!({"type":"function","function":{
                        "name":tool["name"],
                        "description":tool["description"],
                        "parameters":tool["parameters"]
                    }})
                })
                .collect(),
        );
        body["tool_choice"] = json!(match request.tool_choice {
            ToolChoice::None => "none",
            ToolChoice::Auto => "auto",
            ToolChoice::Required => "required",
        });
    }
    Ok(body)
}

fn instructions(request: &Request, search_requested: bool) -> String {
    if search_requested {
        format!(
            "{}\n\n当前 API 路由没有可用的联网搜索实现；本轮未执行搜索，不得声称已检索外部资料。",
            request.instructions
        )
    } else {
        request.instructions.clone()
    }
}

fn chat_content(item: &Value) -> Result<Value, String> {
    let content = item
        .get("content")
        .and_then(Value::as_array)
        .ok_or("统一消息缺少 content 数组。")?;
    let mut converted = Vec::new();
    for part in content {
        match part.get("type").and_then(Value::as_str) {
            Some("input_text") => converted.push(json!({"type":"text","text":part.get("text").and_then(Value::as_str).ok_or("文字内容无效。")?})),
            Some("input_image") => converted.push(json!({
                "type":"image_url",
                "image_url":{
                    "url":part.get("image_url").and_then(Value::as_str).ok_or("图片 URL 无效。")?,
                    "detail":part.get("detail").and_then(Value::as_str).unwrap_or("auto")
                }
            })),
            _ => return Err("统一消息包含通用 API 路由不支持的内容块。".into()),
        }
    }
    if converted.len() == 1 && converted[0].get("type").and_then(Value::as_str) == Some("text") {
        Ok(converted.remove(0)["text"].clone())
    } else {
        Ok(Value::Array(converted))
    }
}

fn read_function_calls(input: &[Value], index: &mut usize) -> Result<Vec<Value>, String> {
    let mut calls = Vec::new();
    while let Some(item) = input.get(*index) {
        if item.get("type").and_then(Value::as_str) != Some("function_call") {
            break;
        }
        calls.push(json!({
            "id":item.get("call_id").and_then(Value::as_str).ok_or("函数调用缺少调用 ID。")?,
            "type":"function",
            "function":{
                "name":item.get("name").and_then(Value::as_str).ok_or("函数调用缺少名称。")?,
                    "arguments":item.get("arguments").and_then(Value::as_str).ok_or("函数调用缺少参数。")?
            }
        }));
        *index += 1;
    }
    Ok(calls)
}

fn is_search_tool(tool: &Value) -> bool {
    tool.get("type").and_then(Value::as_str) == Some("web_search")
}

fn is_function_tool(tool: &Value) -> bool {
    tool.get("type").and_then(Value::as_str) == Some("function")
}

pub(super) fn decode(value: &Value) -> Result<Vec<super::Event>, String> {
    use super::Event;
    let mut events = Vec::new();
    if let Some(text) = value
        .pointer("/choices/0/delta/content")
        .and_then(Value::as_str)
    {
        events.push(Event::Text(text.to_owned()));
    }
    if let Some(calls) = value
        .pointer("/choices/0/delta/tool_calls")
        .and_then(Value::as_array)
    {
        for call in calls {
            let Some(index) = call
                .get("index")
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok())
            else {
                continue;
            };
            events.push(Event::ToolCallDelta {
                index,
                id: call.get("id").and_then(Value::as_str).map(str::to_owned),
                name: call
                    .pointer("/function/name")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                arguments: call
                    .pointer("/function/arguments")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            });
        }
    }
    if let Some(error) = value.get("error") {
        return Err(error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("模型请求失败。")
            .to_owned());
    }
    match value
        .pointer("/choices/0/finish_reason")
        .and_then(Value::as_str)
    {
        Some("length") => events.push(Event::Truncated(None)),
        Some("content_filter") => return Err("模型输出被内容安全策略截断。".into()),
        _ => {}
    }
    Ok(events)
}

pub(super) fn error_rejects_images(body: &str) -> bool {
    super::error_rejects_images(body)
}

pub(super) fn request_error(status: u16) -> String {
    super::http_error_message(status)
}
