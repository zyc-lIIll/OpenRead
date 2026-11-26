mod chatkit;
pub(crate) mod citation;
pub mod explanation;
pub mod initial;
pub mod model;
pub mod tool;

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
pub(crate) enum ModelRoute {
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

#[derive(Debug)]
pub(crate) enum CompletionEvent {
    Activity {
        reply_id: String,
        segment_id: u32,
        kind: String,
        text: String,
    },
    Delta(String),
    Citation(citation::Citation),
    SectionDone,
    OfferUserAction(String),
    Complete,
    Error(String),
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

    pub(crate) fn api_value(self) -> Option<&'static str> {
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
    pub(crate) fn route(&self, route_id: &str) -> Result<ModelRoute, String> {
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
    messages: Vec<Value>,
    reasoning_effort: ReasoningEffort,
    reply_id: String,
    sender: tokio::sync::mpsc::UnboundedSender<CompletionEvent>,
) {
    if let Err(error) = explanation::stream(
        app,
        state,
        route_id,
        messages,
        reasoning_effort,
        reply_id,
        sender.clone(),
    )
    .await
    {
        let _ = sender.send(CompletionEvent::Error(error));
    }
}

#[cfg(test)]
#[path = "../../../tests/agent_stream.rs"]
mod tests;
