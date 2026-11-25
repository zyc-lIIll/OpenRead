use super::Workspace;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Mutex, time::Duration};
use tauri::{
    ipc::{Channel, Response},
    WebviewWindow,
};
use tokio::sync::oneshot;
use uuid::Uuid;

const PARSE_TIMEOUT: Duration = Duration::from_secs(30);
const PAGE_IMAGE_BYTE_LIMIT: usize = 4 * 1024 * 1024;
const PAGE_IMAGE_DATA_URL_PREFIX: &str = "data:image/png;base64,";

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ParseKind {
    Document,
    #[default]
    PageText,
    PageImage,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseRequest {
    request_id: String,
    source_id: String,
    page_number: Option<u32>,
    kind: ParseKind,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentInfo {
    pub source_id: String,
    pub page_count: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    pub source_id: String,
    pub page_number: u32,
    pub text: String,
    pub view: [f64; 4],
    pub rotation: i32,
    pub user_unit: f64,
}

// The discriminator matches the agreed `{ ok: true, data } | { ok: false, error }` shape.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireResult {
    ok: bool,
    data: Option<ParseData>,
    error: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum ParseData {
    Document {
        #[serde(rename = "pageCount")]
        page_count: u32,
    },
    Page {
        text: String,
        view: [f64; 4],
        rotation: i32,
        #[serde(rename = "userUnit")]
        user_unit: f64,
    },
    PageImage {
        #[serde(rename = "dataUrl")]
        data_url: String,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompletionMessage {
    request_id: String,
    source_id: String,
    page_number: Option<u32>,
    #[serde(default)]
    kind: ParseKind,
    result: WireResult,
}

struct Pending {
    source_id: Uuid,
    page_number: Option<u32>,
    kind: ParseKind,
    reply: oneshot::Sender<Result<ParseData, String>>,
}

struct PendingGuard<'a> {
    bridge: &'a ParserBridge,
    request_id: Uuid,
}

impl Drop for PendingGuard<'_> {
    fn drop(&mut self) {
        self.bridge.forget(self.request_id);
    }
}

#[derive(Default)]
struct State {
    parser: Option<Channel<ParseRequest>>,
    pending: HashMap<Uuid, Pending>,
}

/// Owns the frontend parser channel and correlates requests with validated responses.
#[derive(Default)]
pub struct ParserBridge {
    // Serializes source removal against request registration. Never held over await.
    gate: Mutex<()>,
    state: Mutex<State>,
}

impl ParserBridge {
    pub fn remove_source(&self, workspace: &Workspace, id: Uuid) -> Result<(), String> {
        let _gate = self.gate.lock().map_err(|_| "PDF 服务不可用。")?;
        workspace.remove(id)?;
        let mut state = self.state.lock().map_err(|_| "PDF 服务不可用。")?;
        let cancelled: Vec<_> = state
            .pending
            .iter()
            .filter_map(|(request_id, pending)| (pending.source_id == id).then_some(*request_id))
            .collect();
        for request_id in cancelled {
            if let Some(pending) = state.pending.remove(&request_id) {
                let _ = pending.reply.send(Err("Source 已移除。".into()));
            }
        }
        Ok(())
    }

    fn connect(&self, channel: Channel<ParseRequest>) -> Result<(), String> {
        self.state.lock().map_err(|_| "PDF 服务不可用。")?.parser = Some(channel);
        Ok(())
    }

    fn complete(&self, message: CompletionMessage) -> Result<(), String> {
        let request_id =
            Uuid::parse_str(&message.request_id).map_err(|_| "无效的 PDF 解析请求 ID。")?;
        let source_id = Uuid::parse_str(&message.source_id).map_err(|_| "无效的 Source ID。")?;
        let mut state = self.state.lock().map_err(|_| "PDF 服务不可用。")?;
        let Some(pending) = state.pending.get(&request_id) else {
            // Timed-out, removed, or duplicate/stale responses have no effect.
            return Ok(());
        };
        if pending.source_id != source_id
            || pending.page_number != message.page_number
            || pending.kind != message.kind
        {
            return Err("PDF 解析响应与请求不匹配。".into());
        }
        let result = match (message.result.ok, message.result.data, message.result.error) {
            (true, Some(data), None) => {
                validate_parse_data(pending.page_number, pending.kind, &data)?;
                Ok(data)
            }
            (false, None, Some(error)) => Err(error.chars().take(500).collect()),
            _ => return Err("PDF 解析响应格式无效。".into()),
        };
        if let Some(pending) = state.pending.remove(&request_id) {
            let _ = pending.reply.send(result);
        }
        Ok(())
    }

    async fn request(
        &self,
        workspace: &Workspace,
        source_id: Uuid,
        page_number: Option<u32>,
        kind: ParseKind,
    ) -> Result<ParseData, String> {
        self.request_with_timeout(workspace, source_id, page_number, kind, PARSE_TIMEOUT)
            .await
    }

    async fn request_with_timeout(
        &self,
        workspace: &Workspace,
        source_id: Uuid,
        page_number: Option<u32>,
        kind: ParseKind,
        timeout: Duration,
    ) -> Result<ParseData, String> {
        let request_id = Uuid::new_v4();
        let (reply, receiver) = oneshot::channel();
        let channel = {
            let _gate = self.gate.lock().map_err(|_| "PDF 服务不可用。")?;
            // Validate membership before registering work against this source.
            workspace.file_size(source_id)?;
            let mut state = self.state.lock().map_err(|_| "PDF 服务不可用。")?;
            let channel = state.parser.clone().ok_or("PDF 阅读器尚未连接。")?;
            state.pending.insert(
                request_id,
                Pending {
                    source_id,
                    page_number,
                    kind,
                    reply,
                },
            );
            channel
        };
        let request = ParseRequest {
            request_id: request_id.to_string(),
            source_id: source_id.to_string(),
            page_number,
            kind,
        };
        let _pending_guard = PendingGuard {
            bridge: self,
            request_id,
        };
        if channel.send(request).is_err() {
            return Err("无法向 PDF 阅读器发送解析请求。".into());
        }
        match tokio::time::timeout(timeout, receiver).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("PDF 解析请求已取消。".into()),
            Err(_) => Err("PDF 解析超时。".into()),
        }
    }

    fn forget(&self, request_id: Uuid) {
        if let Ok(mut state) = self.state.lock() {
            state.pending.remove(&request_id);
        }
    }
}

fn validate_parse_data(
    page_number: Option<u32>,
    kind: ParseKind,
    data: &ParseData,
) -> Result<(), String> {
    match (kind, page_number, data) {
        (ParseKind::Document, None, ParseData::Document { page_count }) if *page_count > 0 => {
            Ok(())
        }
        (
            ParseKind::PageText,
            Some(page),
            ParseData::Page {
                view,
                rotation,
                user_unit,
                ..
            },
        ) if page > 0
            && view.iter().all(|value| value.is_finite())
            && view[0] < view[2]
            && view[1] < view[3]
            && matches!(*rotation, 0 | 90 | 180 | 270)
            && user_unit.is_finite()
            && *user_unit > 0.0 =>
        {
            Ok(())
        }
        (ParseKind::PageImage, Some(page), ParseData::PageImage { data_url })
            if page > 0 && validate_page_image(data_url).is_ok() =>
        {
            Ok(())
        }
        _ => Err("PDF 解析结果格式无效。".into()),
    }
}

fn validate_page_image(data_url: &str) -> Result<(), String> {
    let encoded = data_url
        .strip_prefix(PAGE_IMAGE_DATA_URL_PREFIX)
        .ok_or("PDF 页面图像格式无效。")?;
    if encoded.len() > PAGE_IMAGE_BYTE_LIMIT.div_ceil(3) * 4 {
        return Err("PDF 页面图像超过大小限制。".into());
    }
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| "PDF 页面图像编码无效。")?;
    if bytes.len() < 8
        || bytes.len() > PAGE_IMAGE_BYTE_LIMIT
        || bytes[..8] != [137, 80, 78, 71, 13, 10, 26, 10]
    {
        return Err("PDF 页面图像不是有效的 PNG 或超过大小限制。".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn pdf_file_size(
    workspace: tauri::State<'_, Workspace>,
    source_id: String,
) -> Result<u64, String> {
    let id = Uuid::parse_str(&source_id).map_err(|_| "无效的 Source ID。")?;
    let workspace = (*workspace).clone();
    tauri::async_runtime::spawn_blocking(move || workspace.file_size(id))
        .await
        .map_err(|_| "无法读取 PDF 文件。")?
}

#[tauri::command]
pub async fn pdf_read_range(
    workspace: tauri::State<'_, Workspace>,
    source_id: String,
    offset: u64,
    length: usize,
) -> Result<Response, String> {
    let id = Uuid::parse_str(&source_id).map_err(|_| "无效的 Source ID。")?;
    let workspace = (*workspace).clone();
    let bytes =
        tauri::async_runtime::spawn_blocking(move || workspace.read_range(id, offset, length))
            .await
            .map_err(|_| "无法读取 PDF 文件。")??;
    Ok(Response::new(bytes))
}

#[tauri::command]
pub fn pdf_connect_parser(
    window: WebviewWindow,
    bridge: tauri::State<'_, ParserBridge>,
    channel: Channel<ParseRequest>,
) -> Result<(), String> {
    ensure_main_window(&window)?;
    bridge.connect(channel)
}

#[tauri::command]
pub fn pdf_resolve_parse(
    window: WebviewWindow,
    bridge: tauri::State<'_, ParserBridge>,
    request_id: String,
    source_id: String,
    page_number: Option<u32>,
    kind: ParseKind,
    result: WireResult,
) -> Result<(), String> {
    ensure_main_window(&window)?;
    bridge.complete(CompletionMessage {
        request_id,
        source_id,
        page_number,
        kind,
        result,
    })
}

fn ensure_main_window(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == "main" {
        Ok(())
    } else {
        Err("仅主窗口可以连接 PDF 解析器。".into())
    }
}

#[tauri::command]
pub async fn pdf_read_document(
    workspace: tauri::State<'_, Workspace>,
    bridge: tauri::State<'_, ParserBridge>,
    source_id: String,
) -> Result<DocumentInfo, String> {
    read_document(&bridge, &workspace, &source_id).await
}

#[tauri::command]
pub async fn pdf_read_page(
    workspace: tauri::State<'_, Workspace>,
    bridge: tauri::State<'_, ParserBridge>,
    source_id: String,
    page_number: u32,
) -> Result<PageInfo, String> {
    read_page(&bridge, &workspace, &source_id, page_number).await
}

pub async fn read_document(
    bridge: &ParserBridge,
    workspace: &Workspace,
    source_id: &str,
) -> Result<DocumentInfo, String> {
    let id = Uuid::parse_str(source_id).map_err(|_| "无效的 Source ID。")?;
    match bridge
        .request(workspace, id, None, ParseKind::Document)
        .await?
    {
        ParseData::Document { page_count } if page_count > 0 => Ok(DocumentInfo {
            source_id: source_id.into(),
            page_count,
        }),
        _ => Err("PDF 阅读器返回的文档信息无效。".into()),
    }
}

pub async fn read_page(
    bridge: &ParserBridge,
    workspace: &Workspace,
    source_id: &str,
    page_number: u32,
) -> Result<PageInfo, String> {
    if page_number == 0 {
        return Err("PDF 页码从 1 开始。".into());
    }
    let id = Uuid::parse_str(source_id).map_err(|_| "无效的 Source ID。")?;
    match bridge
        .request(workspace, id, Some(page_number), ParseKind::PageText)
        .await?
    {
        ParseData::Page {
            text,
            view,
            rotation,
            user_unit,
        } => Ok(PageInfo {
            source_id: source_id.into(),
            page_number,
            text,
            view,
            rotation,
            user_unit,
        }),
        _ => Err("PDF 阅读器返回的页面信息无效。".into()),
    }
}

pub async fn read_page_image(
    bridge: &ParserBridge,
    workspace: &Workspace,
    source_id: &str,
    page_number: u32,
) -> Result<String, String> {
    if page_number == 0 {
        return Err("PDF 页码从 1 开始。".into());
    }
    let id = Uuid::parse_str(source_id).map_err(|_| "无效的 Source ID。")?;
    match bridge
        .request(workspace, id, Some(page_number), ParseKind::PageImage)
        .await?
    {
        ParseData::PageImage { data_url } => Ok(data_url),
        _ => Err("PDF 阅读器返回的页面图像无效。".into()),
    }
}

#[cfg(test)]
#[path = "../../../../tests/pdf_backend.rs"]
mod tests;
