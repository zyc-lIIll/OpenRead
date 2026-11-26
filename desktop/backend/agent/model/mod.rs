//! Provider-neutral Agent requests and provider-specific streaming adapters.

use std::{
    collections::{BTreeMap, HashSet},
    future::Future,
    pin::Pin,
};

use futures_util::StreamExt;
use reqwest::Response;
use serde_json::{json, Value};
use tauri::AppHandle;

use super::{AgentState, ModelRoute, ReasoningEffort};

pub mod chatgpt;
pub mod other;

const MAX_FRAME_BYTES: usize = 1_048_576;
const MAX_ERROR_BODY_BYTES: usize = 8_192;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolChoice {
    None,
    Auto,
    Required,
}

#[derive(Clone, Default)]
pub(crate) struct Context(Vec<ResponseContext>);

#[derive(Clone)]
struct ResponseContext {
    input_index: usize,
    output: Vec<Value>,
}

pub(crate) struct Request {
    pub model: Route,
    pub instructions: String,
    pub input: Vec<Value>,
    pub tools: Vec<Value>,
    pub tool_choice: ToolChoice,
    pub stream: bool,
    pub store: bool,
    pub reasoning_effort: ReasoningEffort,
    pub context: Context,
}

impl Request {
    pub(crate) fn new(
        model: Route,
        instructions: String,
        input: Vec<Value>,
        tools: Vec<Value>,
        tool_choice: ToolChoice,
        reasoning_effort: ReasoningEffort,
        context: Context,
    ) -> Self {
        Self {
            model,
            instructions,
            input,
            tools,
            tool_choice,
            stream: true,
            store: false,
            reasoning_effort,
            context,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WebCitation {
    pub title: String,
    pub url: String,
    pub index: usize,
}

#[derive(Default)]
pub(crate) struct Turn {
    pub text: String,
    pub tool_calls: Vec<ModelToolCall>,
    pub citations: Vec<WebCitation>,
    pub context: Context,
    pub truncated: bool,
}

#[derive(Debug)]
pub(crate) enum Error {
    Cancelled,
    Request {
        images_rejected: bool,
        search_unavailable: bool,
        message: String,
    },
    Failed(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Adapter {
    Chatgpt,
    Other,
}

#[derive(Clone)]
pub(crate) struct Route(ModelRoute);

impl Route {
    fn adapter(&self) -> Adapter {
        match &self.0 {
            ModelRoute::Api { .. } => Adapter::Other,
            ModelRoute::Chatgpt { .. } => Adapter::Chatgpt,
        }
    }

    pub(crate) fn supports_web_search(&self) -> bool {
        self.adapter() == Adapter::Chatgpt
    }
}

pub(crate) fn resolve(state: &AgentState, route_id: &str) -> Result<Route, String> {
    state.route(route_id).map(Route)
}

#[cfg(test)]
pub(crate) fn test_route() -> Route {
    Route(ModelRoute::Api {
        provider_id: "provider".into(),
        model_id: "model".into(),
    })
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

#[derive(Debug)]
pub(crate) enum StreamEvent {
    Text(String),
    SearchProgress,
}

pub(crate) fn text_message(role: &str, text: &str) -> Value {
    json!({"role":role,"content":[{"type":"input_text","text":text}]})
}

pub(crate) fn user_message(input: &str, images: &[super::tool::source::pdf::PageImage]) -> Value {
    let mut content = vec![json!({"type":"input_text","text":input})];
    for image in images {
        content.push(
            json!({"type":"input_text","text":format!("【第 {} 页页面图像】",image.page_number)}),
        );
        content.push(json!({"type":"input_image","image_url":image.data_url,"detail":"auto"}));
    }
    json!({"role":"user","content":content})
}

pub(crate) fn function_call(call: &ModelToolCall) -> Value {
    json!({"type":"function_call","call_id":call.id,"name":call.name,"arguments":call.arguments})
}

pub(crate) fn function_call_output(call_id: &str, output: &str) -> Value {
    json!({"type":"function_call_output","call_id":call_id,"output":output})
}

pub(crate) fn web_search_tool() -> Value {
    json!({"type":"web_search"})
}

pub(crate) async fn stream_request<F>(
    app: &AppHandle,
    state: &AgentState,
    request: &Request,
    cancel: F,
    mut on_event: impl FnMut(StreamEvent) -> Result<(), String>,
) -> Result<Turn, Error>
where
    F: Future<Output = ()> + Send,
{
    let route = &request.model.0;
    let adapter = request.model.adapter();
    let mut cancel = Box::pin(cancel);
    let (response, search_unavailable) = tokio::select! {
        biased;
        _ = cancel.as_mut() => return Err(Error::Cancelled),
        response = async {
            match adapter {
                Adapter::Chatgpt => chatgpt::send(app, state, route, request).await,
                Adapter::Other => other::send(app, state, route, request).await,
            }
        } => response.map_err(Error::Failed)?,
    };

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let has_images = has_image_input(&request.input);
        let images_rejected = if status == 413 && has_images {
            true
        } else if should_read_image_error_body(status, has_images) {
            let body = read_error_body(response, &mut cancel).await?;
            match adapter {
                Adapter::Chatgpt => chatgpt::error_rejects_images(&body),
                Adapter::Other => other::error_rejects_images(&body),
            }
        } else {
            false
        };
        let message = match adapter {
            Adapter::Chatgpt => chatgpt::request_error(status, request, search_unavailable),
            Adapter::Other => other::request_error(status),
        };
        return Err(Error::Request {
            images_rejected,
            search_unavailable,
            message,
        });
    }

    read_stream(response, adapter, request, &mut cancel, &mut on_event).await
}

async fn read_error_body<F>(response: Response, cancel: &mut Pin<Box<F>>) -> Result<String, Error>
where
    F: Future<Output = ()> + Send,
{
    let mut stream = response.bytes_stream();
    let mut body = Vec::new();
    while body.len() < MAX_ERROR_BODY_BYTES {
        let chunk = tokio::select! {
            biased;
            _ = cancel.as_mut() => return Err(Error::Cancelled),
            chunk = stream.next() => chunk,
        };
        let Some(chunk) = chunk else { break };
        let chunk = chunk.map_err(|_| Error::Failed("模型服务返回了无效错误信息。".into()))?;
        let remaining = MAX_ERROR_BODY_BYTES - body.len();
        body.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
    }
    Ok(String::from_utf8_lossy(&body).into_owned())
}

fn should_read_image_error_body(status: u16, has_images: bool) -> bool {
    has_images && matches!(status, 400 | 415 | 422)
}

async fn read_stream<F>(
    response: Response,
    adapter: Adapter,
    request: &Request,
    cancel: &mut Pin<Box<F>>,
    on_event: &mut impl FnMut(StreamEvent) -> Result<(), String>,
) -> Result<Turn, Error>
where
    F: Future<Output = ()> + Send,
{
    let mut body = response.bytes_stream();
    let mut buffer = Vec::new();
    let mut turn = Turn {
        context: request.context.clone(),
        ..Turn::default()
    };
    let mut pending_calls = BTreeMap::<u32, ModelToolCall>::new();
    let mut completed = false;

    loop {
        let chunk = tokio::select! {
            biased;
            _ = cancel.as_mut() => return Err(Error::Cancelled),
            chunk = body.next() => chunk,
        };
        let Some(chunk) = chunk else { break };
        buffer.extend_from_slice(&chunk.map_err(|_| Error::Failed("模型响应流中断。".into()))?);
        while let Some((end, width)) = frame_end(&buffer) {
            if end > MAX_FRAME_BYTES {
                return Err(Error::Failed("模型响应帧超过安全长度限制。".into()));
            }
            let frame = buffer[..end].to_vec();
            buffer.drain(..end + width);
            for event in decode_frame(adapter, &frame).map_err(Error::Failed)? {
                match event {
                    Event::Text(delta) => {
                        on_event(StreamEvent::Text(delta.clone())).map_err(Error::Failed)?;
                        turn.text.push_str(&delta);
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
                    Event::SearchProgress => {
                        on_event(StreamEvent::SearchProgress).map_err(Error::Failed)?;
                    }
                    Event::Truncated(output) => {
                        turn.truncated = true;
                        collect_response_output(adapter, output, request, &mut turn)
                            .map_err(Error::Failed)?;
                        completed = true;
                        break;
                    }
                    Event::Completed(output) => {
                        collect_response_output(adapter, output, request, &mut turn)
                            .map_err(Error::Failed)?;
                        completed = true;
                        break;
                    }
                    Event::Failed(Some(code))
                        if code.starts_with("subscription_sharing_usage_") =>
                    {
                        return Err(Error::Failed("ChatGPT 额度不足或暂不可用。".into()))
                    }
                    Event::Failed(_) => return Err(Error::Failed("模型未能完成此次响应。".into())),
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
            return Err(Error::Failed("模型响应帧超过安全长度限制。".into()));
        }
    }
    if !completed {
        return Err(Error::Failed("模型响应流未正常结束。".into()));
    }
    turn.tool_calls.extend(pending_calls.into_values());
    if turn.truncated && !turn.tool_calls.is_empty() {
        return Err(Error::Failed(
            "模型在完成工具调用前截断了参数，请重试或缩小问题范围。".into(),
        ));
    }
    let mut seen = HashSet::new();
    turn.tool_calls.retain(|call| seen.insert(call.id.clone()));
    if turn
        .tool_calls
        .iter()
        .any(|call| call.id.is_empty() || call.name.is_empty() || call.arguments.is_empty())
    {
        return Err(Error::Failed("模型返回的工具调用不完整。".into()));
    }
    Ok(turn)
}

fn collect_response_output(
    adapter: Adapter,
    output: Option<Value>,
    request: &Request,
    turn: &mut Turn,
) -> Result<(), String> {
    let Some(items) = output.and_then(|output| output.as_array().cloned()) else {
        return Ok(());
    };
    if adapter == Adapter::Chatgpt {
        turn.tool_calls.extend(chatgpt::tool_calls(&items)?);
        turn.citations.extend(chatgpt::citations(&items));
        turn.context.0.push(ResponseContext {
            input_index: request.input.len(),
            output: items,
        });
    }
    Ok(())
}

fn has_image_input(input: &[Value]) -> bool {
    input.iter().any(|item| {
        item.pointer("/content")
            .and_then(Value::as_array)
            .is_some_and(|content| {
                content
                    .iter()
                    .any(|part| part.get("type").and_then(Value::as_str) == Some("input_image"))
            })
    })
}

fn error_rejects_images(body: &str) -> bool {
    let body = body.to_ascii_lowercase();
    let value = serde_json::from_str::<Value>(&body).unwrap_or(Value::Null);
    let error = value.get("error").unwrap_or(&value);
    let missing_model = error
        .get("message")
        .and_then(Value::as_str)
        .is_some_and(|message| {
            ["model ", "the model ", "requested model ", "no such model"]
                .iter()
                .any(|prefix| message.starts_with(prefix))
                && ["not found", "was not found", "does not exist"]
                    .iter()
                    .any(|term| message.contains(term))
        });
    if error.get("code").and_then(Value::as_str) == Some("model_not_found")
        || error.get("param").and_then(Value::as_str) == Some("model")
        || missing_model
    {
        return false;
    }
    let details = ["message", "param", "code", "type"]
        .iter()
        .filter_map(|field| error.get(field).and_then(Value::as_str))
        .collect::<Vec<_>>();
    let details = if details.is_empty() {
        vec![body.as_str()]
    } else {
        details
    };
    details.iter().any(|detail| {
        [
            "image",
            "multimodal",
            "image_url",
            "input_image",
            "vision input",
            "support vision",
            "vision is not supported",
            "vision not supported",
            "vision unsupported",
            "visual input",
            "payload too large",
            "request too large",
            "request entity too large",
            "request body too large",
            "maximum request size",
        ]
        .iter()
        .any(|term| detail.contains(term))
    })
}

fn http_error_message(status: u16) -> String {
    match status {
        401 | 403 => "模型服务拒绝了请求，请检查授权或 API Key。".into(),
        429 => "模型服务当前请求过多或额度不足。".into(),
        status => format!("模型服务请求失败（HTTP {status}）。"),
    }
}

pub fn decode_frame(adapter: Adapter, frame: &[u8]) -> Result<Vec<Event>, String> {
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
    if adapter == Adapter::Other && data == "[DONE]" {
        return Ok(vec![Event::Completed(None)]);
    }
    let value: Value =
        serde_json::from_str(&data).map_err(|_| "模型返回了无效的流式数据。".to_string())?;
    match adapter {
        Adapter::Chatgpt => chatgpt::decode(&value, event_name),
        Adapter::Other => other::decode(&value),
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
