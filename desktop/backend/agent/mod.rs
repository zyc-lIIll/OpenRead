mod chatkit;
pub mod initial;
pub mod source;

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};

use futures_util::StreamExt;
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

use crate::{
    ai_verification::chatgpt::{self, ChatGptService},
    providers,
};

#[derive(Clone)]
pub struct AgentState {
    address: SocketAddr,
    token: String,
    routes: Arc<Mutex<RouteRegistry>>,
    initializations:
        Arc<Mutex<HashMap<uuid::Uuid, (uuid::Uuid, tokio::sync::watch::Sender<bool>)>>>,
    client: Client,
}

#[derive(Default)]
struct RouteRegistry {
    generation: u64,
    routes: HashMap<String, ModelRoute>,
}

impl RouteRegistry {
    fn begin_refresh(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.routes.clear();
        self.generation
    }

    fn publish(&mut self, generation: u64, routes: HashMap<String, ModelRoute>) -> bool {
        if self.generation != generation {
            return false;
        }
        self.routes = routes;
        true
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
enum ModelSource {
    Api,
    Chatgpt,
}

#[derive(Clone)]
enum ModelRoute {
    Api {
        provider_id: String,
        model_id: String,
    },
    Chatgpt {
        model_id: String,
        registration_id: String,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatModel {
    id: String,
    label: String,
    preference_key: String,
}

#[derive(Serialize)]
pub struct ChatKitConnection {
    url: String,
    token: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ModelMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug)]
pub(crate) enum CompletionEvent {
    Delta(String),
    Complete,
    Error(String),
}

enum StreamKind {
    ChatCompletions,
    Responses,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReasoningEffort {
    Auto,
    Low,
    Medium,
    High,
}

impl ReasoningEffort {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            _ => None,
        }
    }

    fn api_value(self) -> Option<&'static str> {
        match self {
            Self::Auto => None,
            Self::Low => Some("low"),
            Self::Medium => Some("medium"),
            Self::High => Some("high"),
        }
    }
}

pub fn initialize(app: AppHandle) -> Result<AgentState, String> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .map_err(|_| "无法启动本机聊天服务。".to_string())?;
    listener
        .set_nonblocking(true)
        .map_err(|_| "无法启动本机聊天服务。".to_string())?;
    let address = listener
        .local_addr()
        .map_err(|_| "无法启动本机聊天服务。".to_string())?;
    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let state = AgentState {
        address,
        token,
        routes: Arc::new(Mutex::new(RouteRegistry::default())),
        initializations: Arc::default(),
        client: Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|_| "无法配置本机聊天服务。".to_string())?,
    };
    chatkit::start(listener, app, state.clone());
    Ok(state)
}

#[tauri::command]
pub fn get_chatkit_connection(state: State<'_, AgentState>) -> ChatKitConnection {
    ChatKitConnection {
        url: format!("http://{}/chatkit", state.address),
        token: state.token.clone(),
    }
}

#[tauri::command]
pub async fn list_chat_models(
    app: AppHandle,
    state: State<'_, AgentState>,
    source: String,
) -> Result<Vec<ChatModel>, String> {
    let generation = state
        .routes
        .lock()
        .map_err(|_| "模型列表暂不可用。".to_string())?
        .begin_refresh();
    let source: ModelSource =
        serde_json::from_value(Value::String(source)).map_err(|_| "模型来源无效。".to_string())?;
    let (models, routes) = match source {
        ModelSource::Api => api_models(&app)?,
        ModelSource::Chatgpt => chatgpt_models(&app, &state).await?,
    };
    if models.is_empty() {
        return Err(match source {
            ModelSource::Api => "设置中没有可用的 API 模型。".into(),
            ModelSource::Chatgpt => "当前 ChatGPT 账号没有可用模型。".into(),
        });
    }
    let published = state
        .routes
        .lock()
        .map_err(|_| "模型列表暂不可用。".to_string())?
        .publish(generation, routes);
    if !published {
        return Err("模型列表已更新，请使用最新的列表。".into());
    }
    Ok(models)
}

fn api_models(app: &AppHandle) -> Result<(Vec<ChatModel>, HashMap<String, ModelRoute>), String> {
    let providers = app.state::<providers::Store>().agent_providers()?;
    let mut models = Vec::new();
    let mut routes = HashMap::new();
    for provider in providers {
        for model_id in provider.models {
            let id = Uuid::new_v4().to_string();
            models.push(ChatModel {
                id: id.clone(),
                label: format!("{} · {}", provider.name, model_id),
                preference_key: format!("api:{}:{}", provider.id, model_id),
            });
            routes.insert(
                id,
                ModelRoute::Api {
                    provider_id: provider.id.clone(),
                    model_id,
                },
            );
        }
    }
    Ok((models, routes))
}

async fn chatgpt_models(
    app: &AppHandle,
    state: &AgentState,
) -> Result<(Vec<ChatModel>, HashMap<String, ModelRoute>), String> {
    let service = app.state::<ChatGptService>().inner().clone();
    let (registration_id, token) =
        tauri::async_runtime::spawn_blocking(move || chatgpt::model_access_token(&service))
            .await
            .map_err(|_| "无法读取 ChatGPT 授权。".to_string())??
            .ok_or_else(|| "请先登录并授权使用 ChatGPT 额度。".to_string())?;
    let response = state
        .client
        .get("https://api.openai.com/v1/models")
        .bearer_auth(token)
        .send()
        .await
        .map_err(|_| "无法读取 ChatGPT 可用模型。".to_string())?;
    if !response.status().is_success() {
        return Err("无法读取 ChatGPT 可用模型，请检查登录或额度授权。".into());
    }
    let value: Value = response
        .json()
        .await
        .map_err(|_| "ChatGPT 返回了无效的模型列表。".to_string())?;
    let catalog = value
        .get("models")
        .and_then(Value::as_array)
        .ok_or_else(|| "ChatGPT 返回了无效的模型列表。".to_string())?;
    let mut models = Vec::new();
    let mut routes = HashMap::new();
    for item in catalog {
        if item.get("visibility").and_then(Value::as_str) != Some("list") {
            continue;
        }
        let (Some(model_id), Some(name)) = (
            item.get("slug").and_then(Value::as_str),
            item.get("display_name").and_then(Value::as_str),
        ) else {
            continue;
        };
        let id = Uuid::new_v4().to_string();
        models.push(ChatModel {
            id: id.clone(),
            label: format!("ChatGPT · {name}"),
            preference_key: format!("chatgpt:{model_id}"),
        });
        routes.insert(
            id,
            ModelRoute::Chatgpt {
                model_id: model_id.to_string(),
                registration_id: registration_id.clone(),
            },
        );
    }
    Ok((models, routes))
}

impl AgentState {
    fn route(&self, route_id: &str) -> Result<ModelRoute, String> {
        self.routes
            .lock()
            .map_err(|_| "模型列表暂不可用。".to_string())?
            .routes
            .get(route_id)
            .cloned()
            .ok_or_else(|| "所选模型已不可用，请重新选择模型。".into())
    }
}

pub(crate) async fn stream_completion(
    app: AppHandle,
    state: AgentState,
    route_id: String,
    messages: Vec<ModelMessage>,
    reasoning_effort: ReasoningEffort,
    sender: tokio::sync::mpsc::UnboundedSender<CompletionEvent>,
) {
    if let Err(error) = stream_completion_inner(
        &app,
        &state,
        &route_id,
        &messages,
        reasoning_effort,
        &sender,
    )
    .await
    {
        let _ = sender.send(CompletionEvent::Error(error));
    }
}

async fn stream_completion_inner(
    app: &AppHandle,
    state: &AgentState,
    route_id: &str,
    messages: &[ModelMessage],
    reasoning_effort: ReasoningEffort,
    sender: &tokio::sync::mpsc::UnboundedSender<CompletionEvent>,
) -> Result<(), String> {
    let route = state.route(route_id)?;
    let (response, kind) = match &route {
        ModelRoute::Api {
            provider_id,
            model_id,
        } => {
            let provider = app
                .state::<providers::Store>()
                .agent_provider_model(provider_id, model_id)?;
            let messages: Vec<Value> = messages
                .iter()
                .map(|message| json!({"role": message.role, "content": message.content}))
                .collect();
            let response = state
                .client
                .post(format!(
                    "{}/chat/completions",
                    provider.base_url.trim_end_matches('/')
                ))
                .bearer_auth(provider.api_key)
                .json(&json!({"model": model_id, "messages": messages, "stream": true}))
                .send()
                .await
                .map_err(|_| "无法连接所选 API 服务商。".to_string())?;
            (response, StreamKind::ChatCompletions)
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
            let input: Vec<Value> = messages
                .iter()
                .map(|message| json!({"role": message.role, "content": message.content}))
                .collect();
            let mut body =
                json!({"model": model_id, "input": input, "store": false, "stream": true});
            if let Some(effort) = reasoning_effort.api_value() {
                body["reasoning"] = json!({"effort": effort});
            }
            let response = state
                .client
                .post("https://api.openai.com/v1/responses")
                .bearer_auth(token)
                .json(&body)
                .send()
                .await
                .map_err(|_| "无法连接 ChatGPT。".to_string())?;
            (response, StreamKind::Responses)
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
            _ => format!("模型服务请求失败（HTTP {}）。", response.status().as_u16()),
        });
    }
    read_model_stream(response, kind, sender).await
}

async fn read_model_stream(
    response: Response,
    kind: StreamKind,
    sender: &tokio::sync::mpsc::UnboundedSender<CompletionEvent>,
) -> Result<(), String> {
    let mut body = response.bytes_stream();
    let mut buffer = Vec::new();
    let mut complete = false;
    loop {
        let chunk = tokio::select! {
            _ = sender.closed() => return Ok(()),
            chunk = body.next() => chunk,
        };
        let Some(chunk) = chunk else { break };
        let chunk = chunk.map_err(|_| "模型响应流中断。".to_string())?;
        buffer.extend_from_slice(&chunk);
        while let Some((frame, consumed)) = next_sse_frame(&buffer) {
            buffer.drain(..consumed);
            if handle_sse_frame(&frame, &kind, sender)? {
                complete = true;
                break;
            }
        }
        if complete {
            break;
        }
    }
    if complete {
        let _ = sender.send(CompletionEvent::Complete);
        Ok(())
    } else if sender.is_closed() {
        Ok(())
    } else {
        Err("模型响应流未正常结束。".into())
    }
}

fn next_sse_frame(buffer: &[u8]) -> Option<(Vec<u8>, usize)> {
    let (frame_end, consumed) = sse_frame_end(buffer)?;
    Some((buffer[..frame_end].to_vec(), consumed))
}

fn sse_frame_end(buffer: &[u8]) -> Option<(usize, usize)> {
    let lf = buffer
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|i| (i, 2));
    let crlf = buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|i| (i, 4));
    let (start, width) = match (lf, crlf) {
        (Some(lf), Some(crlf)) => {
            if lf.0 < crlf.0 {
                lf
            } else {
                crlf
            }
        }
        (Some(found), None) | (None, Some(found)) => found,
        (None, None) => return None,
    };
    Some((start, start + width))
}

fn handle_sse_frame(
    frame: &[u8],
    kind: &StreamKind,
    sender: &tokio::sync::mpsc::UnboundedSender<CompletionEvent>,
) -> Result<bool, String> {
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
        return Ok(false);
    }
    if matches!(kind, StreamKind::ChatCompletions) && data == "[DONE]" {
        return Ok(true);
    }
    let value: Value =
        serde_json::from_str(&data).map_err(|_| "模型返回了无效的流式数据。".to_string())?;
    match kind {
        StreamKind::ChatCompletions => {
            if let Some(text) = value
                .pointer("/choices/0/delta/content")
                .and_then(Value::as_str)
            {
                let _ = sender.send(CompletionEvent::Delta(text.to_string()));
            }
            Ok(false)
        }
        StreamKind::Responses => {
            let name = value.get("type").and_then(Value::as_str).or(event_name);
            match name {
                Some("response.output_text.delta") => {
                    if let Some(text) = value.get("delta").and_then(Value::as_str) {
                        let _ = sender.send(CompletionEvent::Delta(text.to_string()));
                    }
                    Ok(false)
                }
                Some("response.completed") => Ok(true),
                Some("response.failed" | "response.incomplete" | "error") => {
                    Err("ChatGPT 未能完成此次响应。".into())
                }
                _ => Ok(false),
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/agent_stream.rs"]
mod tests;
