//! Shared Agent tools and the registry used by each Agent's own loop.

use std::{future::Future, pin::Pin};

use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::source::{pdf, Workspace};

pub mod overview;
pub mod source;

pub use crate::agent::model::ModelToolCall as ToolCall;

const MAX_TOOL_OUTPUT_BYTES: usize = 48_000;

pub type ToolFuture<'a> = Pin<Box<dyn Future<Output = Result<ToolExecution, String>> + Send + 'a>>;
pub type ToolHandler = for<'a> fn(&'a AppHandle, &'a ToolCall, &'a mut usize) -> ToolFuture<'a>;

#[derive(Clone)]
pub struct ToolDefinition {
    pub schema: Value,
    pub handler: ToolHandler,
}

impl ToolDefinition {
    pub fn name(&self) -> Option<&str> {
        self.schema.get("name").and_then(Value::as_str)
    }
}

#[derive(Clone, Default)]
pub struct ToolRegistry(Vec<ToolDefinition>);

impl ToolRegistry {
    pub fn new(tools: Vec<ToolDefinition>) -> Self {
        Self(tools)
    }

    pub fn extend(&mut self, tools: impl IntoIterator<Item = ToolDefinition>) {
        self.0.extend(tools);
    }

    pub fn select(&self, names: &[&str]) -> Result<Self, String> {
        names
            .iter()
            .map(|name| {
                self.find(name)
                    .cloned()
                    .ok_or_else(|| "Agent 工具配置引用了未注册工具。".to_string())
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Self)
    }

    pub fn schemas(&self) -> Vec<Value> {
        self.0.iter().map(|tool| tool.schema.clone()).collect()
    }

    pub fn find(&self, name: &str) -> Option<&ToolDefinition> {
        self.0.iter().find(|tool| tool.name() == Some(name))
    }
}

#[derive(Debug, Default)]
pub struct ToolExecution {
    pub content: String,
    pub citations: Vec<SourceCitation>,
    pub actions: Vec<ToolAction>,
}

#[derive(Debug)]
pub enum ToolAction {
    AppendInstructions(String),
    OfferUserAction(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceCitation {
    pub id: String,
    pub source_id: String,
    pub source_name: String,
    pub page_number: u32,
}

pub fn shared_registry() -> ToolRegistry {
    ToolRegistry::new(vec![
        ToolDefinition {
            schema: json!({"type":"function","name":"list_sources","description":"列出当前已导入的 Source 及页数。每次新会话先调用它确认可用材料。","parameters":{"type":"object","properties":{},"additionalProperties":false}}),
            handler: list_sources,
        },
        ToolDefinition {
            schema: json!({"type":"function","name":"get_overview","description":"读取指定 Source 已生成的 Markdown 概览；未初始化时会明确返回未初始化。","parameters":{"type":"object","properties":{"source_id":{"type":"string"}},"required":["source_id"],"additionalProperties":false}}),
            handler: get_overview,
        },
        ToolDefinition {
            schema: json!({"type":"function","name":"read_source","description":"读取 PDF 指定页范围的原文。每次最多 5 页；结果包含可引用的页码锚点。","parameters":{"type":"object","properties":{"source_id":{"type":"string"},"page_start":{"type":"integer","minimum":1},"page_end":{"type":"integer","minimum":1}},"required":["source_id","page_start","page_end"],"additionalProperties":false}}),
            handler: read_source,
        },
    ])
}

pub async fn dispatch(
    app: &AppHandle,
    registry: &ToolRegistry,
    call: &ToolCall,
    citation_index: &mut usize,
) -> Result<ToolExecution, String> {
    let definition = registry
        .find(&call.name)
        .ok_or_else(|| "请求了未注册或当前请求不可用的 Agent 工具。".to_string())?;
    let _: Value =
        serde_json::from_str(&call.arguments).map_err(|_| "工具参数不是有效 JSON。".to_string())?;
    (definition.handler)(app, call, citation_index).await
}

fn list_sources<'a>(app: &'a AppHandle, call: &'a ToolCall, _: &'a mut usize) -> ToolFuture<'a> {
    Box::pin(async move {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Args {}
        let _: Args = serde_json::from_str(&call.arguments)
            .map_err(|_| "list_sources 参数无效。".to_string())?;
        let workspace = app.state::<Workspace>();
        let bridge = app.state::<pdf::ParserBridge>();
        let mut sources = Vec::new();
        for source in workspace.list_sources()? {
            let page_count = source::pdf::page_count(&bridge, &workspace, &source.id).await;
            sources.push(json!({"id": source.id, "name": source.name, "page_count": page_count, "initialized": source.has_overview}));
        }
        encode_output(json!({"sources": sources}), Vec::new())
    })
}

fn get_overview<'a>(app: &'a AppHandle, call: &'a ToolCall, _: &'a mut usize) -> ToolFuture<'a> {
    Box::pin(async move {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Args {
            source_id: String,
        }
        let args: Args = serde_json::from_str(&call.arguments)
            .map_err(|_| "get_overview 参数无效。".to_string())?;
        let id = Uuid::parse_str(&args.source_id).map_err(|_| "Source ID 无效。".to_string())?;
        let value = overview::get(&app.state::<Workspace>(), id)?;
        encode_output(value, Vec::new())
    })
}

fn read_source<'a>(
    app: &'a AppHandle,
    call: &'a ToolCall,
    citation_index: &'a mut usize,
) -> ToolFuture<'a> {
    Box::pin(async move {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Args {
            source_id: String,
            page_start: u32,
            page_end: u32,
        }
        let args: Args = serde_json::from_str(&call.arguments)
            .map_err(|_| "read_source 参数无效。".to_string())?;
        let source_id =
            Uuid::parse_str(&args.source_id).map_err(|_| "Source ID 无效。".to_string())?;
        let (start, end) = source::pdf::page_range(
            &json!({"page_start":args.page_start,"page_end":args.page_end}),
        )?;
        let workspace = app.state::<Workspace>();
        let bridge = app.state::<pdf::ParserBridge>();
        let (value, citations) =
            source::pdf::read_source(&bridge, &workspace, source_id, start, end, citation_index)
                .await?;
        encode_output(value, citations)
    })
}

fn encode_output(value: Value, citations: Vec<SourceCitation>) -> Result<ToolExecution, String> {
    let content = serde_json::to_string(&value).map_err(|_| "工具结果无法编码。".to_string())?;
    if content.len() > MAX_TOOL_OUTPUT_BYTES {
        return Err("工具结果超过安全长度限制，请缩小查询范围。".into());
    }
    Ok(ToolExecution {
        content,
        citations,
        actions: Vec::new(),
    })
}

#[cfg(test)]
#[path = "../../../../tests/agent_tools.rs"]
mod tests;
