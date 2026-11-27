use serde::Deserialize;
use serde_json::{json, Value};

use super::{markdown, ToolAction, ToolCall, ToolDefinition, ToolExecution, ToolFuture};

pub(crate) const OPEN_EXPLANATION_ACTION: &str = "open_explanation";
pub(crate) const SELECT_WORKFLOW: &str = "select_workflow";
pub(crate) const ORGANIZE_OUTPUT: &str = "organize_output";
pub(crate) const CONTINUE_OUTPUT: &str = "continue_output";
pub(crate) const CONTINUE_COLLECTION: &str = "continue_collection";

const GENERAL_READING_LENS: &str =
    "使用跨学科的清晰讲解方式，优先定义概念、展示关系和提供必要例子。";

pub(crate) fn select_workflow_tool() -> ToolDefinition {
    ToolDefinition {
        schema: json!({
            "type":"function",
            "name":SELECT_WORKFLOW,
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

fn select_instructions(workflow: &str, reading_lens: &str) -> Result<String, String> {
    let workflow_key = match workflow {
        "source_explanation" => "skill/explanation/source_explanation.md",
        "knowledge_question" => "skill/explanation/knowledge_question.md",
        "reading_discussion" => "skill/explanation/reading_discussion.md",
        "review" => "skill/explanation/review.md",
        "other" => "skill/explanation/other.md",
        _ => return Err("工作流类型无效。".into()),
    };
    let workflow = markdown::load_markdown(workflow_key)?;
    let lens = match reading_lens {
        "general" => GENERAL_READING_LENS,
        "math" => markdown::load_markdown("skill/math.md")?,
        "formal_paper" => markdown::load_markdown("skill/formal_paper.md")?,
        "empirical" => markdown::load_markdown("skill/empirical.md")?,
        "humanities" => markdown::load_markdown("skill/humanities.md")?,
        _ => return Err("学科视角无效。".into()),
    };
    Ok(format!("{}\n\n{}", workflow.trim(), lens.trim()))
}

fn select_workflow<'a>(
    _: &'a tauri::AppHandle,
    call: &'a ToolCall,
    _: &'a mut usize,
) -> ToolFuture<'a> {
    Box::pin(async move {
        let selection: Selection = serde_json::from_str(&call.arguments)
            .map_err(|_| "工作流选择参数无效。".to_string())?;
        let instructions = select_instructions(&selection.workflow, &selection.reading_lens)?;
        let mut actions = vec![ToolAction::AppendInstructions(instructions)];
        if selection.suggest_new_explanation {
            actions.push(ToolAction::OfferUserAction(
                OPEN_EXPLANATION_ACTION.to_string(),
            ));
        }
        let content = serde_json::to_string(&json!({
            "workflow": selection.workflow,
            "reading_lens": selection.reading_lens,
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

pub(crate) fn control_tool(name: &str) -> Value {
    let description = match name {
        ORGANIZE_OUTPUT => "结束当前取材片段并开始面向用户的正式输出。",
        CONTINUE_OUTPUT => "继续当前正式输出，并保持在正式输出阶段。",
        _ => "结束当前正式输出片段，并继续取材准备下一段。",
    };
    json!({
        "type":"function",
        "name":name,
        "description":description,
        "parameters":{"type":"object","properties":{},"additionalProperties":false}
    })
}

#[cfg(test)]
mod tests {
    use super::select_instructions;

    #[test]
    fn workflow_and_lens_selections_load_their_instructions() {
        for workflow in [
            "source_explanation",
            "knowledge_question",
            "reading_discussion",
            "review",
            "other",
        ] {
            for lens in ["general", "math", "formal_paper", "empirical", "humanities"] {
                assert!(!select_instructions(workflow, lens).unwrap().is_empty());
            }
        }

        assert!(select_instructions("unknown", "general").is_err());
        assert!(select_instructions("other", "unknown").is_err());
    }
}
