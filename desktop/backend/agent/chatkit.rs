use std::{
    collections::HashMap,
    convert::Infallible,
    net::TcpListener as StdTcpListener,
    sync::{Arc, Mutex},
};

use axum::{
    body::{to_bytes, Body, Bytes},
    extract::State,
    http::{header, HeaderMap, Method, Request, StatusCode},
    response::Response,
    routing::any,
    Router,
};
use chrono::Utc;
use futures_util::stream;
use serde_json::{json, Value};
use tauri::AppHandle;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use super::{AgentState, CompletionEvent, ModelMessage, ReasoningEffort};

const MAX_REQUEST_BYTES: usize = 256 * 1024;

#[derive(Clone)]
struct ServerState {
    app: AppHandle,
    agent: AgentState,
    threads: Arc<Mutex<HashMap<String, ThreadRecord>>>,
}

#[derive(Clone)]
struct ThreadRecord {
    id: String,
    created_at: String,
    items: Vec<Value>,
}

pub(super) fn start(listener: StdTcpListener, app: AppHandle, agent: AgentState) {
    let state = ServerState {
        app,
        agent,
        threads: Arc::default(),
    };
    tauri::async_runtime::spawn(async move {
        let Ok(listener) = tokio::net::TcpListener::from_std(listener) else {
            return;
        };
        let router = Router::new()
            .route("/chatkit", any(handle))
            .with_state(state);
        let _ = axum::serve(listener, router).await;
    });
}

async fn handle(State(state): State<ServerState>, request: Request<Body>) -> Response {
    let origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok());
    let Some(origin) = origin
        .filter(|origin| allowed_origin(origin))
        .map(str::to_owned)
    else {
        return response(StatusCode::FORBIDDEN, "application/json", "{}", None);
    };
    if request.method() == Method::OPTIONS {
        return preflight_response(&origin);
    }
    if request.method() != Method::POST {
        return response(
            StatusCode::METHOD_NOT_ALLOWED,
            "application/json",
            "{}",
            Some(&origin),
        );
    }
    if !authorized(request.headers(), &state.agent.token) {
        return response(
            StatusCode::UNAUTHORIZED,
            "application/json",
            "{}",
            Some(&origin),
        );
    }
    if request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_none_or(|content_type| {
            !content_type.split(';').next().is_some_and(|media_type| {
                media_type.trim().eq_ignore_ascii_case("application/json")
            })
        })
    {
        return response(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "application/json",
            "{}",
            Some(&origin),
        );
    }
    let reasoning_effort = match request.headers().get("x-openread-reasoning-effort") {
        Some(value) => match value.to_str().ok().and_then(ReasoningEffort::parse) {
            Some(effort) => effort,
            None => return chatkit_error(StatusCode::BAD_REQUEST, "思考强度设置无效。", &origin),
        },
        None => ReasoningEffort::Auto,
    };
    let body = match to_bytes(request.into_body(), MAX_REQUEST_BYTES).await {
        Ok(body) => body,
        Err(_) => {
            return response(
                StatusCode::PAYLOAD_TOO_LARGE,
                "application/json",
                "{}",
                Some(&origin),
            )
        }
    };
    let request: Value = match serde_json::from_slice(&body) {
        Ok(request) => request,
        Err(_) => {
            return response(
                StatusCode::BAD_REQUEST,
                "application/json",
                "{}",
                Some(&origin),
            )
        }
    };
    handle_request(state, request, reasoning_effort, &origin).await
}

async fn handle_request(
    state: ServerState,
    request: Value,
    reasoning_effort: ReasoningEffort,
    origin: &str,
) -> Response {
    let Some(operation) = request.get("type").and_then(Value::as_str) else {
        return response(
            StatusCode::BAD_REQUEST,
            "application/json",
            "{}",
            Some(origin),
        );
    };
    match operation {
        "threads.create" | "threads.add_user_message" => {
            let params = &request["params"];
            let input = &params["input"];
            let route_id = input
                .pointer("/inference_options/model")
                .and_then(Value::as_str);
            let Some(route_id) = route_id.filter(|id| !id.is_empty()) else {
                return chatkit_error(StatusCode::BAD_REQUEST, "请选择一个模型。", origin);
            };
            let Some(text) = user_text(input) else {
                return chatkit_error(StatusCode::BAD_REQUEST, "请输入文字消息。", origin);
            };
            if input
                .get("attachments")
                .and_then(Value::as_array)
                .is_some_and(|attachments| !attachments.is_empty())
            {
                return chatkit_error(StatusCode::BAD_REQUEST, "当前聊天暂不支持附件。", origin);
            }
            let (thread_id, created, user_item, history) = if operation == "threads.create" {
                let id = format!("thread_{}", Uuid::new_v4().simple());
                let created_at = Utc::now().to_rfc3339();
                let user_item = user_message(&id, &text, route_id);
                let mut record = ThreadRecord {
                    id: id.clone(),
                    created_at,
                    items: Vec::new(),
                };
                record.items.push(user_item.clone());
                let history = model_history(&record.items);
                let Ok(mut threads) = state.threads.lock() else {
                    return chatkit_error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "聊天状态暂不可用。",
                        origin,
                    );
                };
                threads.insert(id.clone(), record.clone());
                (id, Some(thread_created(&record)), user_item, history)
            } else {
                let Some(thread_id) = params.get("thread_id").and_then(Value::as_str) else {
                    return chatkit_error(StatusCode::BAD_REQUEST, "找不到当前对话。", origin);
                };
                let user_item = user_message(thread_id, &text, route_id);
                let Ok(mut threads) = state.threads.lock() else {
                    return chatkit_error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "聊天状态暂不可用。",
                        origin,
                    );
                };
                let Some(record) = threads.get_mut(thread_id) else {
                    return chatkit_error(
                        StatusCode::NOT_FOUND,
                        "当前对话已结束，请新建对话。",
                        origin,
                    );
                };
                record.items.push(user_item.clone());
                (
                    thread_id.to_string(),
                    None,
                    user_item,
                    model_history(&record.items),
                )
            };
            stream_response(
                state,
                thread_id,
                route_id.to_string(),
                user_item,
                created,
                history,
                reasoning_effort,
                origin,
            )
        }
        "threads.get_by_id" => {
            let Some(id) = request.pointer("/params/thread_id").and_then(Value::as_str) else {
                return response(
                    StatusCode::BAD_REQUEST,
                    "application/json",
                    "{}",
                    Some(origin),
                );
            };
            let Ok(threads) = state.threads.lock() else {
                return response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "application/json",
                    "{}",
                    Some(origin),
                );
            };
            match threads.get(id) {
                Some(thread) => response_json(StatusCode::OK, &thread_value(thread), Some(origin)),
                None => response(
                    StatusCode::NOT_FOUND,
                    "application/json",
                    "{}",
                    Some(origin),
                ),
            }
        }
        "items.list" => {
            let Some(id) = request.pointer("/params/thread_id").and_then(Value::as_str) else {
                return response(
                    StatusCode::BAD_REQUEST,
                    "application/json",
                    "{}",
                    Some(origin),
                );
            };
            let Ok(threads) = state.threads.lock() else {
                return response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "application/json",
                    "{}",
                    Some(origin),
                );
            };
            match threads.get(id) {
                Some(thread) => {
                    response_json(StatusCode::OK, &page(thread.items.clone()), Some(origin))
                }
                None => response(
                    StatusCode::NOT_FOUND,
                    "application/json",
                    "{}",
                    Some(origin),
                ),
            }
        }
        "threads.list" => response_json(
            StatusCode::OK,
            &json!({"data": [], "after": null, "has_more": false}),
            Some(origin),
        ),
        "threads.delete" => {
            if let Some(id) = request.pointer("/params/thread_id").and_then(Value::as_str) {
                if let Ok(mut threads) = state.threads.lock() {
                    threads.remove(id);
                }
            }
            response_json(StatusCode::OK, &json!({}), Some(origin))
        }
        _ => chatkit_error(
            StatusCode::NOT_IMPLEMENTED,
            "当前聊天操作暂不支持。",
            origin,
        ),
    }
}

fn stream_response(
    state: ServerState,
    thread_id: String,
    route_id: String,
    user_item: Value,
    created: Option<Value>,
    history: Vec<ModelMessage>,
    reasoning_effort: ReasoningEffort,
    origin: &str,
) -> Response {
    let (event_tx, event_rx) = mpsc::channel::<Bytes>(16);
    let (cancel_tx, cancel_rx) = oneshot::channel();
    let stream_state = state.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(created) = created {
            if !send_event(&event_tx, &created).await {
                return;
            }
        }
        if !send_event(
            &event_tx,
            &json!({"type": "thread.item.done", "item": user_item}),
        )
        .await
            || !send_event(
                &event_tx,
                &json!({"type": "stream_options", "stream_options": {"allow_cancel": true}}),
            )
            .await
        {
            return;
        }
        let assistant_id = format!("message_{}", Uuid::new_v4().simple());
        let created_at = Utc::now().to_rfc3339();
        let assistant = json!({
            "type": "assistant_message",
            "id": assistant_id,
            "thread_id": thread_id,
            "created_at": created_at,
            "content": []
        });
        if !send_event(
            &event_tx,
            &json!({"type": "thread.item.added", "item": assistant}),
        )
        .await
            || !send_event(&event_tx, &content_part_added_event(&assistant_id)).await
        {
            return;
        }
        let (completion_tx, mut completion_rx) = mpsc::unbounded_channel();
        tauri::async_runtime::spawn(super::stream_completion(
            stream_state.app.clone(),
            stream_state.agent.clone(),
            route_id,
            history,
            reasoning_effort,
            completion_tx,
        ));
        let mut cancelled = cancel_rx;
        let mut text = String::new();
        loop {
            tokio::select! {
                _ = &mut cancelled => {
                    if !text.is_empty() {
                        save_assistant(&stream_state, &thread_id, &assistant_id, &text);
                    }
                    return;
                }
                event = completion_rx.recv() => match event {
                    Some(CompletionEvent::Delta(delta)) => {
                        text.push_str(&delta);
                        let update = content_part_delta_event(&assistant_id, &delta);
                        if !send_event(&event_tx, &update).await { return; }
                    }
                    Some(CompletionEvent::Complete) => {
                        let item = assistant_item(&assistant_id, &thread_id, &created_at, &text);
                        save_item(&stream_state, &thread_id, item.clone());
                        let _ = send_event(
                            &event_tx,
                            &content_part_done_event(&assistant_id, &text),
                        )
                        .await;
                        let _ = send_event(&event_tx, &json!({"type": "thread.item.done", "item": item})).await;
                        return;
                    }
                    Some(CompletionEvent::Error(message)) => {
                        let _ = send_event(&event_tx, &json!({"type": "error", "code": "custom", "message": message, "allow_retry": false})).await;
                        return;
                    }
                    None => {
                        let _ = send_event(&event_tx, &json!({"type": "error", "code": "stream_error", "message": "模型响应流中断。", "allow_retry": false})).await;
                        return;
                    }
                }
            }
        }
    });
    let stream = stream::unfold(
        (event_rx, CancelOnDrop(Some(cancel_tx))),
        |(mut rx, guard)| async move {
            rx.recv()
                .await
                .map(|event| (Ok::<Bytes, Infallible>(event), (rx, guard)))
        },
    );
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache, no-transform")
        .body(Body::from_stream(stream))
        .unwrap();
    cors_headers(response.headers_mut(), origin);
    response
}

struct CancelOnDrop(Option<oneshot::Sender<()>>);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if let Some(cancel) = self.0.take() {
            let _ = cancel.send(());
        }
    }
}

async fn send_event(sender: &mpsc::Sender<Bytes>, event: &Value) -> bool {
    let Ok(data) = serde_json::to_vec(event) else {
        return false;
    };
    let mut bytes = b"data: ".to_vec();
    bytes.extend_from_slice(&data);
    bytes.extend_from_slice(b"\n\n");
    sender.send(Bytes::from(bytes)).await.is_ok()
}

fn content_part_added_event(item_id: &str) -> Value {
    json!({
        "type": "thread.item.updated",
        "item_id": item_id,
        "update": {
            "type": "assistant_message.content_part.added",
            "content_index": 0,
            "content": {"type": "output_text", "text": "", "annotations": []}
        }
    })
}

fn content_part_delta_event(item_id: &str, delta: &str) -> Value {
    json!({
        "type": "thread.item.updated",
        "item_id": item_id,
        "update": {
            "type": "assistant_message.content_part.text_delta",
            "content_index": 0,
            "delta": delta
        }
    })
}

fn content_part_done_event(item_id: &str, text: &str) -> Value {
    json!({
        "type": "thread.item.updated",
        "item_id": item_id,
        "update": {
            "type": "assistant_message.content_part.done",
            "content_index": 0,
            "content": {"type": "output_text", "text": text, "annotations": []}
        }
    })
}

fn user_text(input: &Value) -> Option<String> {
    if input
        .get("attachments")
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
    {
        return None;
    }
    let content = input.get("content")?.as_array()?;
    let text = content
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("input_text"))
        .filter_map(|item| item.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("");
    (!text.trim().is_empty()).then_some(text)
}

fn user_message(thread_id: &str, text: &str, route_id: &str) -> Value {
    json!({
        "type": "user_message",
        "id": format!("message_{}", Uuid::new_v4().simple()),
        "thread_id": thread_id,
        "created_at": Utc::now().to_rfc3339(),
        "content": [{"type": "input_text", "text": text}],
        "attachments": [],
        "quoted_text": null,
        "inference_options": {"model": route_id, "tool_choice": null}
    })
}

fn model_history(items: &[Value]) -> Vec<ModelMessage> {
    items
        .iter()
        .filter_map(|item| {
            let role = match item.get("type")?.as_str()? {
                "user_message" => "user",
                "assistant_message" => "assistant",
                _ => return None,
            };
            let content = item
                .get("content")?
                .as_array()?
                .iter()
                .filter_map(|part| part.get("text").and_then(Value::as_str))
                .collect::<String>();
            Some(ModelMessage {
                role: role.into(),
                content,
            })
        })
        .collect()
}

fn thread_created(thread: &ThreadRecord) -> Value {
    json!({"type": "thread.created", "thread": thread_value(thread)})
}

fn thread_value(thread: &ThreadRecord) -> Value {
    json!({
        "id": thread.id,
        "created_at": thread.created_at,
        "title": null,
        "status": {"type": "active"},
        "metadata": {},
        "items": page(thread.items.clone())
    })
}

fn page(items: Vec<Value>) -> Value {
    json!({"data": items, "after": null, "has_more": false})
}

fn assistant_item(id: &str, thread_id: &str, created_at: &str, text: &str) -> Value {
    json!({
        "type": "assistant_message",
        "id": id,
        "thread_id": thread_id,
        "created_at": created_at,
        "content": [{"type": "output_text", "text": text, "annotations": []}]
    })
}

fn save_assistant(state: &ServerState, thread_id: &str, id: &str, text: &str) {
    if text.is_empty() {
        return;
    }
    let item = assistant_item(id, thread_id, &Utc::now().to_rfc3339(), text);
    save_item(state, thread_id, item);
}

fn save_item(state: &ServerState, thread_id: &str, item: Value) {
    if let Ok(mut threads) = state.threads.lock() {
        if let Some(thread) = threads.get_mut(thread_id) {
            thread.items.push(item);
        }
    }
}

fn allowed_origin(origin: &str) -> bool {
    matches!(
        origin,
        "http://tauri.localhost" | "tauri://localhost" | "http://127.0.0.1:1420"
    )
}

fn authorized(headers: &HeaderMap, token: &str) -> bool {
    let Some(value) = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    let Some(candidate) = value.strip_prefix("Bearer ") else {
        return false;
    };
    let expected = token.as_bytes();
    let candidate = candidate.as_bytes();
    expected.len() == candidate.len()
        && expected
            .iter()
            .zip(candidate)
            .fold(0u8, |difference, (a, b)| difference | (a ^ b))
            == 0
}

fn chatkit_error(status: StatusCode, message: &str, origin: &str) -> Response {
    response_json(
        status,
        &json!({"error": {"message": message}}),
        Some(origin),
    )
}

fn response_json(status: StatusCode, value: &Value, origin: Option<&str>) -> Response {
    let body = serde_json::to_string(value).unwrap_or_else(|_| "{}".into());
    response(status, "application/json", &body, origin)
}

fn response(status: StatusCode, content_type: &str, body: &str, origin: Option<&str>) -> Response {
    let mut response = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from(body.to_string()))
        .unwrap();
    if let Some(origin) = origin {
        cors_headers(response.headers_mut(), origin);
    }
    response
}

fn cors_headers(headers: &mut HeaderMap, origin: &str) {
    if let Ok(value) = origin.parse() {
        headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, value);
    }
    headers.insert(header::VARY, "Origin".parse().unwrap());
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
        "false".parse().unwrap(),
    );
}

fn preflight_response(origin: &str) -> Response {
    let mut response = response(StatusCode::NO_CONTENT, "text/plain", "", Some(origin));
    response.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        "POST, OPTIONS".parse().unwrap(),
    );
    response.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        "authorization, content-type, chatkit-frame-instance-id, x-openread-reasoning-effort"
            .parse()
            .unwrap(),
    );
    response
        .headers_mut()
        .insert(header::ACCESS_CONTROL_MAX_AGE, "60".parse().unwrap());
    response
}

#[cfg(test)]
#[path = "../../../tests/chatkit.rs"]
mod tests;
