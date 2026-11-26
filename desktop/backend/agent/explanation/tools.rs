use serde::Deserialize;
use serde_json::json;

use crate::agent::tool::{ToolAction, ToolCall, ToolDefinition, ToolExecution, ToolFuture};

pub(crate) const OPEN_EXPLANATION_ACTION: &str = "open_explanation";

pub(crate) fn select_workflow_tool() -> ToolDefinition {
    ToolDefinition {
        schema: json!({
            "type":"function",
            "name":"select_workflow",
            "description":"每条新用户消息先调用一次：选择最适合的工作流和学科视角，并判断是否建议另开 Explanation。",
            "parameters": {
                "type":"object",
                "properties": {
                    "workflow":{"type":"string","enum":["source_explanation","knowledge_question","reading_discussion","review","other"]},
                    "reading_lens":{"type":"string","enum":["general","math","formal_paper","empirical","humanities"]},
                    "suggest_new_explanation":{"type":"boolean"}
                },
                "required":["workflow","reading_lens","suggest_new_explanation"],
                "additionalProperties":false
            }
        }),
        handler: select_workflow,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    workflow: String,
    reading_lens: String,
    suggest_new_explanation: bool,
}

fn select_workflow<'a>(
    _: &'a tauri::AppHandle,
    call: &'a ToolCall,
    _: &'a mut usize,
) -> ToolFuture<'a> {
    Box::pin(async move {
        let selection: Selection = serde_json::from_str(&call.arguments)
            .map_err(|_| "工作流选择参数无效。".to_string())?;
        let workflow = selection.workflow.clone();
        let reading_lens = selection.reading_lens.clone();
        let instructions = super::skill::select(super::skill::Selection {
            workflow: selection.workflow,
            reading_lens: selection.reading_lens,
        })?;
        let mut actions = vec![ToolAction::AppendInstructions(instructions)];
        if selection.suggest_new_explanation {
            actions.push(ToolAction::OfferUserAction(
                OPEN_EXPLANATION_ACTION.to_string(),
            ));
        }
        let content = serde_json::to_string(&json!({
            "workflow": workflow,
            "reading_lens": reading_lens,
            "suggest_new_explanation": selection.suggest_new_explanation,
        }))
        .map_err(|_| "工作流选择结果无法编码。".to_string())?;
        Ok(ToolExecution {
            content,
            citations: Vec::new(),
            actions,
        })
    })
}
