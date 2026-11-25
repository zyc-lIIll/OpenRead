//! Normalizes provider-specific streaming events for the Agent workflows.

use std::collections::BTreeMap;

use futures_util::StreamExt;
use reqwest::Response;
use serde_json::Value;
use tauri::{AppHandle, Manager};
use tokio::sync::mpsc;

use crate::{
    ai_verification::chatgpt::{self, ChatGptService},
    providers,
};

use super::{AgentState, ModelRoute, ReasoningEffort};

pub mod chat_completions;
pub mod responses;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    ChatCompletions,
    Responses,
}

#[derive(Debug, PartialEq)]
pub enum Event {
    Text(String),
    ToolCallDelta {
        index: u32,
        id: Option<String>,
        name: Option<String>,
        arguments: Option<String>,
    },
    SearchProgress,
    Truncated(Option<Value>),
    Completed(Option<Value>),
    Failed(Option<String>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Default)]
pub struct Turn {
    pub text: String,
    pub tool_calls: Vec<ModelToolCall>,
    pub responses_items: Vec<Value>,
    pub truncated: bool,
}

pub async fn stream_turn(
    app: &AppHandle,
    state: &AgentState,
    route_id: &str,
    messages: &[super::ModelMessage],
    tools: &[Value],
    require_tool: bool,
    reasoning_effort: ReasoningEffort,
    sender: &mpsc::UnboundedSender<super::CompletionEvent>,
    mut on_text: impl FnMut(&str),
) -> Result<Turn, String> {
    let route = state.route(route_id)?;
    let (response, format) = match &route {
        ModelRoute::Api {
            provider_id,
            model_id,
        } => {
            let provider = app
                .state::<providers::Store>()
                .agent_provider_model(provider_id, model_id)?;
            let body = chat_completions::request_body(model_id, messages, tools, require_tool);
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
            (response, Format::ChatCompletions)
        }
        ModelRoute::Chatgpt {
            model_id,
            registration_id,
        } => {
            let service = app.state::<ChatGptService>().inner().clone();
            let expected_registration_id = registration_id.clone();
            let token = tauri::async_runtime::spawn_blocking(move || {
                chatgpt::model_access_token_for(&service, &expected_registration_id)
            })
            .await
            .map_err(|_| "无法读取 ChatGPT 授权。".to_string())??
            .ok_or_else(|| "请先登录并授权使用 ChatGPT 额度。".to_string())?;
            let body = responses::request_body(
                model_id,
                messages,
                tools,
                reasoning_effort.api_value(),
                !require_tool,
                require_tool,
            );
            let mut response = state
                .client
                .post("https://api.openai.com/v1/responses")
                .bearer_auth(&token)
                .json(&body)
                .send()
                .await
                .map_err(|_| "无法连接 ChatGPT。".to_string())?;
            if response.status() == reqwest::StatusCode::BAD_REQUEST {
                let mut fallback_messages = messages.to_vec();
                if let Some(system) = fallback_messages
                    .iter_mut()
                    .find(|message| message.role == "system")
                {
                    system.content = system.content.replace(
                        "本轮可按需联网搜索。",
                        "本轮没有联网搜索工具，不得声称已检索外部资料。",
                    );
                }
                let body = responses::request_body(
                    model_id,
                    &fallback_messages,
                    tools,
                    reasoning_effort.api_value(),
                    false,
                    require_tool,
                );
                response = state
                    .client
                    .post("https://api.openai.com/v1/responses")
                    .bearer_auth(token)
                    .json(&body)
                    .send()
                    .await
                    .map_err(|_| "无法连接 ChatGPT。".to_string())?;
            }
            (response, Format::Responses)
        }
    };
    if !response.status().is_success() {
        if matches!(route, ModelRoute::Chatgpt { .. })
            && reasoning_effort.api_value().is_some()
            && response.status() == reqwest::StatusCode::BAD_REQUEST
        {
            return Err(
                "模型请求失败（HTTP 400）。如果该模型不支持当前思考强度，请改用“自动”后重试。"
                    .into(),
            );
        }
        return Err(match response.status().as_u16() {
            401 | 403 => "模型服务拒绝了请求，请检查授权或 API Key。".into(),
            429 => "模型服务当前请求过多或额度不足。".into(),
            status => format!("模型服务请求失败（HTTP {status}）。"),
        });
    }
    read_stream(response, format, sender, &mut on_text).await
}

async fn read_stream(
    response: Response,
    format: Format,
    sender: &mpsc::UnboundedSender<super::CompletionEvent>,
    on_text: &mut impl FnMut(&str),
) -> Result<Turn, String> {
    const MAX_FRAME_BYTES: usize = 1_048_576;
    let mut body = response.bytes_stream();
    let mut buffer = Vec::new();
    let mut turn = Turn::default();
    let mut pending_calls = BTreeMap::<u32, ModelToolCall>::new();
    let mut completed = false;
    loop {
        let chunk = tokio::select! { _ = sender.closed() => return Err("响应已取消。".into()), chunk = body.next() => chunk };
        let Some(chunk) = chunk else { break };
        buffer.extend_from_slice(&chunk.map_err(|_| "模型响应流中断。".to_string())?);
        while let Some((end, width)) = frame_end(&buffer) {
            if end > MAX_FRAME_BYTES {
                return Err("模型响应帧超过安全长度限制。".into());
            }
            let frame = buffer[..end].to_vec();
            buffer.drain(..end + width);
            for event in decode_frame(format, &frame)? {
                match event {
                    Event::Text(delta) => {
                        turn.text.push_str(&delta);
                        on_text(&delta);
                    }
                    Event::ToolCallDelta {
                        index,
                        id,
                        name,
                        arguments,
                    } => {
                        let call = pending_calls.entry(index).or_insert_with(|| ModelToolCall {
                            id: String::new(),
                            name: String::new(),
                            arguments: String::new(),
                        });
                        if let Some(id) = id {
                            call.id = id;
                        }
                        if let Some(name) = name {
                            call.name.push_str(&name);
                        }
                        if let Some(arguments) = arguments {
                            call.arguments.push_str(&arguments);
                        }
                    }
                    Event::SearchProgress => {}
                    Event::Truncated(output) => {
                        turn.truncated = true;
                        if let Some(items) = output.as_ref().and_then(Value::as_array) {
                            turn.tool_calls.extend(responses::tool_calls(items));
                            turn.responses_items = items.clone();
                        }
                        completed = true;
                        break;
                    }
                    Event::Completed(output) => {
                        if let Some(output) = output {
                            if let Some(items) = output.as_array() {
                                turn.tool_calls.extend(responses::tool_calls(items));
                                turn.responses_items = items.clone();
                            }
                        }
                        completed = true;
                        break;
                    }
                    Event::Failed(Some(code))
                        if code.starts_with("subscription_sharing_usage_") =>
                    {
                        return Err("ChatGPT 额度不足或暂不可用。".into())
                    }
                    Event::Failed(_) => return Err("ChatGPT 未能完成此次响应。".into()),
                }
            }
            if completed {
                break;
            }
        }
        if completed {
            break;
        }
        if buffer.len() > MAX_FRAME_BYTES + 4 {
            return Err("模型响应帧超过安全长度限制。".into());
        }
    }
    if !completed {
        return if sender.is_closed() {
            Err("响应已取消。".into())
        } else {
            Err("模型响应流未正常结束。".into())
        };
    }
    turn.tool_calls.extend(pending_calls.into_values());
    if turn.truncated && !turn.tool_calls.is_empty() {
        return Err("模型在完成工具调用前截断了参数，请重试或缩小问题范围。".into());
    }
    let mut seen = std::collections::HashSet::new();
    turn.tool_calls.retain(|call| seen.insert(call.id.clone()));
    if turn
        .tool_calls
        .iter()
        .any(|call| call.id.is_empty() || call.name.is_empty())
    {
        return Err("模型返回的工具调用不完整。".into());
    }
    Ok(turn)
}

pub fn decode_frame(format: Format, frame: &[u8]) -> Result<Vec<Event>, String> {
    let frame = String::from_utf8_lossy(frame);
    let event_name = frame
        .lines()
        .find_map(|line| line.strip_prefix("event:"))
        .map(str::trim);
    let data = frame
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n");
    if data.is_empty() {
        return Ok(Vec::new());
    }
    if format == Format::ChatCompletions && data == "[DONE]" {
        return Ok(vec![Event::Completed(None)]);
    }
    let value: Value =
        serde_json::from_str(&data).map_err(|_| "模型返回了无效的流式数据。".to_string())?;
    match format {
        Format::ChatCompletions => chat_completions::decode(&value),
        Format::Responses => responses::decode(&value, event_name),
    }
}

pub fn frame_end(buffer: &[u8]) -> Option<(usize, usize)> {
    let lf = buffer
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|i| (i, 2));
    let crlf = buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|i| (i, 4));
    match (lf, crlf) {
        (Some(lf), Some(crlf)) => Some(if lf.0 < crlf.0 { lf } else { crlf }),
        (Some(found), None) | (None, Some(found)) => Some(found),
        (None, None) => None,
    }
}

#[cfg(test)]
#[path = "../../../../tests/agent_model.rs"]
mod tests;
