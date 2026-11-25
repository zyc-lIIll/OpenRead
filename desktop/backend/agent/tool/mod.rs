//! Agent-callable tools shared by Explanation and future workflows.

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::source::{pdf, Workspace};

pub mod overview;
pub mod source;

pub use crate::agent::model::ModelToolCall as ToolCall;

const MAX_TOOL_OUTPUT_BYTES: usize = 48_000;

#[derive(Debug, Default)]
pub struct ToolExecution {
    pub content: String,
    pub citations: Vec<SourceCitation>,
}

#[derive(Clone, Debug)]
pub struct SourceCitation {
    pub id: String,
    pub source_id: String,
    pub source_name: String,
    pub page_number: u32,
}

pub fn schemas() -> Vec<Value> {
    vec![
        json!({"type":"function","name":"list_sources","description":"列出当前已导入的 Source 及页数。每次新会话先调用它确认可用材料。","parameters":{"type":"object","properties":{},"additionalProperties":false}}),
        json!({"type":"function","name":"get_overview","description":"读取指定 Source 已生成的 Markdown 概览；未初始化时会明确返回未初始化。","parameters":{"type":"object","properties":{"source_id":{"type":"string"}},"required":["source_id"],"additionalProperties":false}}),
        json!({"type":"function","name":"read_source","description":"读取 PDF 指定页范围的原文。每次最多 5 页；结果包含可引用的页码锚点。","parameters":{"type":"object","properties":{"source_id":{"type":"string"},"page_start":{"type":"integer","minimum":1},"page_end":{"type":"integer","minimum":1}},"required":["source_id","page_start","page_end"],"additionalProperties":false}}),
    ]
}

pub async fn dispatch(
    app: &AppHandle,
    call: &ToolCall,
    citation_index: &mut usize,
) -> Result<ToolExecution, String> {
    let args: Value =
        serde_json::from_str(&call.arguments).map_err(|_| "工具参数不是有效 JSON。".to_string())?;
    let workspace = app.state::<Workspace>();
    let mut citations = Vec::new();
    let value = match call.name.as_str() {
        "list_sources" => {
            let bridge = app.state::<pdf::ParserBridge>();
            let mut sources = Vec::new();
            for source in workspace.list_sources()? {
                let page_count = source::pdf::page_count(&bridge, &workspace, &source.id).await;
                sources.push(json!({"id": source.id, "name": source.name, "page_count": page_count, "initialized": source.has_overview}));
            }
            json!({"sources": sources})
        }
        "get_overview" => overview::get(&workspace, required_source_id(&args)?)?,
        "read_source" => {
            let source_id = required_source_id(&args)?;
            let (start, end) = source::pdf::page_range(&args)?;
            let bridge = app.state::<pdf::ParserBridge>();
            let (value, page_citations) = source::pdf::read_source(
                &bridge,
                &workspace,
                source_id,
                start,
                end,
                citation_index,
            )
            .await?;
            citations = page_citations;
            value
        }
        _ => return Err("请求了未注册的 Agent 工具。".into()),
    };
    let result = serde_json::to_string(&value).map_err(|_| "工具结果无法编码。".to_string())?;
    if result.len() > MAX_TOOL_OUTPUT_BYTES {
        Err("工具结果超过安全长度限制，请缩小查询范围。".into())
    } else {
        Ok(ToolExecution {
            content: result,
            citations,
        })
    }
}

fn required_source_id(args: &Value) -> Result<Uuid, String> {
    args.get("source_id")
        .and_then(Value::as_str)
        .and_then(|id| Uuid::parse_str(id).ok())
        .ok_or_else(|| "Source ID 无效。".into())
}

#[cfg(test)]
#[path = "../../../../tests/agent_tools.rs"]
mod tests;
