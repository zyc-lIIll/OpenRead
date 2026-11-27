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
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use futures_util::stream;
use serde_json::{json, Value};
use tauri::AppHandle;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use super::tool::explanation::OPEN_EXPLANATION_ACTION;
use super::{
    citation::{Citation, CitationTarget},
    AgentState, CompletionEvent, ReasoningEffort,
};

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
    title: Option<String>,
    items: Vec<Value>,
    latest_user_message_id: Option<String>,
    latest_user_text: Option<String>,
    pending_action: Option<PendingAction>,
}

#[derive(Clone)]
struct PendingAction {
    id: String,
    user_message_id: String,
    issued_at: DateTime<Utc>,
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
                let user_message_id = user_item["id"].as_str().unwrap_or_default().to_string();
                let mut record = ThreadRecord {
                    id: id.clone(),
                    created_at,
                    title: Some(thread_title(&text)),
                    items: Vec::new(),
                    latest_user_message_id: Some(user_message_id),
                    latest_user_text: Some(text.clone()),
                    pending_action: None,
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
                if record.title.is_none() {
                    record.title = Some(thread_title(&text));
                }
                record.latest_user_message_id = user_item["id"].as_str().map(str::to_string);
                record.latest_user_text = Some(text.clone());
                record.pending_action = None;
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
        "threads.custom_action" => handle_custom_action(state, &request, origin),
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

fn handle_custom_action(state: ServerState, request: &Value, origin: &str) -> Response {
    let params = &request["params"];
    let Some(thread_id) = params.get("thread_id").and_then(Value::as_str) else {
        return chatkit_error(StatusCode::BAD_REQUEST, "找不到当前对话。", origin);
    };
    let action = &params["action"];
    if action.get("type").and_then(Value::as_str) != Some(OPEN_EXPLANATION_ACTION) {
        return chatkit_error(StatusCode::BAD_REQUEST, "此对话操作无效。", origin);
    }
    let Some(offer_id) = action.pointer("/payload/offer_id").and_then(Value::as_str) else {
        return chatkit_error(StatusCode::BAD_REQUEST, "新对话建议已失效。", origin);
    };
    let Some(accepted) = action.pointer("/payload/accepted").and_then(Value::as_bool) else {
        return chatkit_error(StatusCode::BAD_REQUEST, "新对话操作无效。", origin);
    };

    let text = {
        let Ok(mut threads) = state.threads.lock() else {
            return chatkit_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "聊天状态暂不可用。",
                origin,
            );
        };
        let Some(thread) = threads.get_mut(thread_id) else {
            return chatkit_error(StatusCode::NOT_FOUND, "当前对话已结束。", origin);
        };
        let pending = thread.pending_action.clone();
        let valid = pending.as_ref().is_some_and(|pending| {
            pending_action_valid(
                pending,
                offer_id,
                thread.latest_user_message_id.as_deref(),
                Utc::now(),
            )
        });
        if !valid {
            return chatkit_error(StatusCode::CONFLICT, "新对话建议已过期或已处理。", origin);
        }
        thread.pending_action = None;
        if accepted {
            thread.latest_user_text.clone()
        } else {
            None
        }
    };

    match (accepted, text) {
        (true, Some(text)) => {
            client_effect_response("openread.explanation.draft", json!({"text": text}), origin)
        }
        (true, None) => chatkit_error(StatusCode::CONFLICT, "找不到最新的用户输入。", origin),
        (false, _) => client_effect_response("openread.explanation.dismissed", json!({}), origin),
    }
}

fn stream_response(
    state: ServerState,
    thread_id: String,
    route_id: String,
    user_item: Value,
    created: Option<Value>,
    history: Vec<Value>,
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
        {
            return;
        }
        let user_message_id = user_item["id"].as_str().unwrap_or_default().to_string();
        let (completion_tx, mut completion_rx) = mpsc::unbounded_channel();
        tauri::async_runtime::spawn(super::stream_completion(
            stream_state.app.clone(),
            stream_state.agent.clone(),
            route_id,
            history,
            reasoning_effort,
            assistant_id.clone(),
            completion_tx,
        ));
        let mut cancelled = cancel_rx;
        let mut current_text = String::new();
        let mut parts = Vec::<Value>::new();
        let mut annotations = Vec::new();
        let mut content_index = 0usize;
        let mut content_open = false;
        let mut offer_action = None::<String>;
        loop {
            tokio::select! {
                _ = &mut cancelled => {
                    save_partial_assistant(
                        &stream_state,
                        &thread_id,
                        &assistant_id,
                        &created_at,
                        &parts,
                        &current_text,
                        content_open,
                    );
                    return;
                }
                event = completion_rx.recv() => match event {
                    Some(CompletionEvent::Delta(delta)) => {
                        if !content_open {
                            let update = content_part_added_event(&assistant_id, content_index);
                            if !send_event(&event_tx, &update).await {
                                save_partial_assistant(&stream_state, &thread_id, &assistant_id, &created_at, &parts, &current_text, content_open);
                                return;
                            }
                            content_open = true;
                        }
                        current_text.push_str(&delta);
                        let update = content_part_delta_event(&assistant_id, content_index, &delta);
                        if !send_event(&event_tx, &update).await {
                            save_partial_assistant(&stream_state, &thread_id, &assistant_id, &created_at, &parts, &current_text, content_open);
                            return;
                        }
                    }
                    Some(CompletionEvent::Citation(citation)) => annotations.push(chatkit_citation(citation)),
                    Some(CompletionEvent::Activity { reply_id, segment_id, kind, text }) => {
                        let effect = activity_effect(&reply_id, segment_id, &kind, &text);
                        if !send_event(&event_tx, &effect).await {
                            save_partial_assistant(&stream_state, &thread_id, &assistant_id, &created_at, &parts, &current_text, content_open);
                            return;
                        }
                    }
                    Some(CompletionEvent::SectionDone) => {
                        if content_open || !annotations.is_empty() {
                            let completed_part = output_text_part(&current_text, &annotations);
                            parts.push(completed_part);
                            save_item(&stream_state, &thread_id, assistant_item_parts(&assistant_id, &thread_id, &created_at, &parts));
                            if !content_open {
                                let update = content_part_added_event(&assistant_id, content_index);
                                if !send_event(&event_tx, &update).await { return; }
                            }
                            let done = content_part_done_event(&assistant_id, content_index, &current_text, &annotations);
                            if !send_event(&event_tx, &done).await { return; }
                            content_index += 1;
                            content_open = false;
                            current_text.clear();
                            annotations.clear();
                        }
                    }
                    Some(CompletionEvent::OfferUserAction(action)) => offer_action = Some(action),
                    Some(CompletionEvent::Complete) => {
                        if content_open || !annotations.is_empty() {
                            let done = content_part_done_event(&assistant_id, content_index, &current_text, &annotations);
                            let _ = send_event(&event_tx, &done).await;
                            parts.push(output_text_part(&current_text, &annotations));
                        }
                        let item = assistant_item_parts(&assistant_id, &thread_id, &created_at, &parts);
                        if !parts.is_empty() {
                            save_item(&stream_state, &thread_id, item.clone());
                        }
                        let _ = send_event(&event_tx, &json!({"type": "thread.item.done", "item": item})).await;
                        if offer_action.as_deref() == Some(OPEN_EXPLANATION_ACTION) {
                            if let Some(offer_id) = create_action_offer(&stream_state, &thread_id, &user_message_id) {
                                let effect = json!({
                                    "type":"client_effect",
                                    "name":"openread.explanation.offer",
                                    "data":{"reply_id":assistant_id,"offer_id":offer_id}
                                });
                                let _ = send_event(&event_tx, &effect).await;
                            }
                        }
                        return;
                    }
                    Some(CompletionEvent::Error(message)) => {
                        let _ = send_activity_clear(&event_tx, &assistant_id, content_index as u32).await;
                        if content_open {
                            let done = content_part_done_event(&assistant_id, content_index, &current_text, &[]);
                            let _ = send_event(&event_tx, &done).await;
                            parts.push(output_text_part(&current_text, &[]));
                        }
                        if !parts.is_empty() {
                            let item = assistant_item_parts(&assistant_id, &thread_id, &created_at, &parts);
                            save_item(&stream_state, &thread_id, item.clone());
                            let _ = send_event(&event_tx, &json!({"type": "thread.item.done", "item": item})).await;
                        }
                        let _ = send_event(&event_tx, &json!({"type": "error", "code": "custom", "message": message, "allow_retry": false})).await;
                        return;
                    }
                    None => {
                        let _ = send_activity_clear(&event_tx, &assistant_id, content_index as u32).await;
                        if content_open {
                            let done = content_part_done_event(&assistant_id, content_index, &current_text, &[]);
                            let _ = send_event(&event_tx, &done).await;
                            parts.push(output_text_part(&current_text, &[]));
                        }
                        if !parts.is_empty() {
                            let item = assistant_item_parts(&assistant_id, &thread_id, &created_at, &parts);
                            save_item(&stream_state, &thread_id, item.clone());
                            let _ = send_event(&event_tx, &json!({"type": "thread.item.done", "item": item})).await;
                        }
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

fn activity_effect(reply_id: &str, segment_id: u32, kind: &str, text: &str) -> Value {
    json!({
        "type":"client_effect",
        "name":"openread.agent.activity",
        "data":{"reply_id":reply_id,"segment_id":segment_id,"kind":kind,"text":text}
    })
}

async fn send_activity_clear(
    sender: &mpsc::Sender<Bytes>,
    reply_id: &str,
    segment_id: u32,
) -> bool {
    send_event(sender, &activity_effect(reply_id, segment_id, "clear", "")).await
}

fn client_effect_response(name: &str, data: Value, origin: &str) -> Response {
    let event = json!({"type":"client_effect","name":name,"data":data});
    let body = serde_json::to_string(&event).unwrap_or_else(|_| "{}".into());
    let body = format!("data: {body}\n\n");
    response(StatusCode::OK, "text/event-stream", &body, Some(origin))
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

fn content_part_added_event(item_id: &str, content_index: usize) -> Value {
    json!({
        "type": "thread.item.updated",
        "item_id": item_id,
        "update": {
            "type": "assistant_message.content_part.added",
            "content_index": content_index,
            "content": {"type": "output_text", "text": "", "annotations": []}
        }
    })
}

fn content_part_delta_event(item_id: &str, content_index: usize, delta: &str) -> Value {
    json!({
        "type": "thread.item.updated",
        "item_id": item_id,
        "update": {
            "type": "assistant_message.content_part.text_delta",
            "content_index": content_index,
            "delta": delta
        }
    })
}

fn content_part_done_event(
    item_id: &str,
    content_index: usize,
    text: &str,
    annotations: &[Value],
) -> Value {
    json!({
        "type": "thread.item.updated",
        "item_id": item_id,
        "update": {
            "type": "assistant_message.content_part.done",
            "content_index": content_index,
            "content": {"type": "output_text", "text": text, "annotations": annotations}
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

fn model_history(items: &[Value]) -> Vec<Value> {
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
            Some(super::model::text_message(role, &content))
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
        "title": thread.title.clone(),
        "status": {"type": "active"},
        "metadata": {},
        "items": page(thread.items.clone())
    })
}

fn page(items: Vec<Value>) -> Value {
    json!({"data": items, "after": null, "has_more": false})
}

fn output_text_part(text: &str, annotations: &[Value]) -> Value {
    json!({"type":"output_text","text":text,"annotations":annotations})
}

fn chatkit_citation(citation: Citation) -> Value {
    let source = match citation.target {
        CitationTarget::Source(source) => json!({
            "type":"entity",
            "id":format!("{}:{}",source.source_id,source.page_number),
            "title":source.source_name,
            "description":format!("PDF 第 {} 页",source.page_number),
            "label":"原文位置",
            "inline_label":format!("p.{}",source.page_number),
            "interactive":true,
            "data":{"sourceId":source.source_id,"pageNumber":source.page_number.to_string()}
        }),
        CitationTarget::Url { title, url } => json!({"type":"url","url":url,"title":title}),
    };
    json!({"type":"annotation","source":source,"index":citation.index})
}

fn assistant_item_parts(id: &str, thread_id: &str, created_at: &str, parts: &[Value]) -> Value {
    json!({
        "type": "assistant_message",
        "id": id,
        "thread_id": thread_id,
        "created_at": created_at,
        "content": parts
    })
}

#[cfg(test)]
fn assistant_item(
    id: &str,
    thread_id: &str,
    created_at: &str,
    text: &str,
    annotations: &[Value],
) -> Value {
    assistant_item_parts(
        id,
        thread_id,
        created_at,
        &[output_text_part(text, annotations)],
    )
}

fn save_item(state: &ServerState, thread_id: &str, item: Value) {
    if let Ok(mut threads) = state.threads.lock() {
        if let Some(thread) = threads.get_mut(thread_id) {
            if let Some(existing) = thread
                .items
                .iter_mut()
                .find(|existing| existing.get("id") == item.get("id"))
            {
                *existing = item;
            } else {
                thread.items.push(item);
            }
        }
    }
}

fn save_partial_assistant(
    state: &ServerState,
    thread_id: &str,
    id: &str,
    created_at: &str,
    completed_parts: &[Value],
    current_text: &str,
    content_open: bool,
) {
    if let Some(item) = partial_assistant_item(
        id,
        thread_id,
        created_at,
        completed_parts,
        current_text,
        content_open,
    ) {
        save_item(state, thread_id, item);
    }
}

fn partial_assistant_item(
    id: &str,
    thread_id: &str,
    created_at: &str,
    completed_parts: &[Value],
    current_text: &str,
    content_open: bool,
) -> Option<Value> {
    let mut parts = completed_parts.to_vec();
    if content_open {
        parts.push(output_text_part(current_text, &[]));
    }
    (!parts.is_empty()).then(|| assistant_item_parts(id, thread_id, created_at, &parts))
}

fn thread_title(input: &str) -> String {
    let title = input.split_whitespace().collect::<Vec<_>>().join(" ");
    let title: String = title.chars().take(32).collect();
    if title.is_empty() {
        "新对话".into()
    } else {
        title
    }
}

fn create_action_offer(
    state: &ServerState,
    thread_id: &str,
    user_message_id: &str,
) -> Option<String> {
    let mut threads = state.threads.lock().ok()?;
    let thread = threads.get_mut(thread_id)?;
    if thread.latest_user_message_id.as_deref() != Some(user_message_id) {
        return None;
    }
    let id = Uuid::new_v4().to_string();
    thread.pending_action = Some(PendingAction {
        id: id.clone(),
        user_message_id: user_message_id.to_string(),
        issued_at: Utc::now(),
    });
    Some(id)
}

fn pending_action_valid(
    pending: &PendingAction,
    offer_id: &str,
    latest_user_message_id: Option<&str>,
    now: DateTime<Utc>,
) -> bool {
    pending.id == offer_id
        && latest_user_message_id == Some(pending.user_message_id.as_str())
        && (ChronoDuration::zero()..=ChronoDuration::minutes(10))
            .contains(&now.signed_duration_since(pending.issued_at))
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
