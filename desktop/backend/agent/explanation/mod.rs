//! Main reading conversation: workflow selection, tool loop and citation validation.

mod citation;
mod prompt;
mod skill;

use std::collections::HashMap;

use serde_json::{json, Value};
use tauri::AppHandle;
use tokio::sync::mpsc;

use super::{
    model,
    tool::{self, SourceCitation},
    AgentState, CompletionEvent, ModelMessage, ModelRoute, ReasoningEffort,
};

const MAX_MODEL_TURNS: usize = 24;
const MAX_TOOL_CALLS: usize = 40;
const MAX_CONTINUATIONS: usize = 8;

pub(crate) async fn stream(
    app: AppHandle,
    state: AgentState,
    route_id: String,
    history: Vec<ModelMessage>,
    reasoning_effort: ReasoningEffort,
    sender: mpsc::UnboundedSender<CompletionEvent>,
) -> Result<(), String> {
    let can_search = matches!(state.route(&route_id)?, ModelRoute::Chatgpt { .. });
    let mut messages = vec![ModelMessage::system(prompt::system_prompt(can_search))];
    messages.extend(history);
    let schemas = tool::schemas();
    let mut citations = HashMap::<String, SourceCitation>::new();
    let mut citation_index = 0;
    let mut formatter = citation::Formatter::default();
    let mut selected_workflow = false;
    let mut tool_count = 0;
    let mut continuation_count = 0;
    let mut external_citations = Vec::new();

    for _ in 0..MAX_MODEL_TURNS {
        if sender.is_closed() {
            return Ok(());
        }
        let force_workflow = !selected_workflow;
        let tools = if force_workflow {
            vec![workflow_schema()]
        } else {
            schemas.clone()
        };
        let publish_text = selected_workflow;
        let turn_start = formatter.raw_chars();
        let mut on_text = |delta: &str| {
            if !publish_text {
                return;
            }
            let visible = formatter.push(delta, &citations);
            if !visible.is_empty() {
                let _ = sender.send(CompletionEvent::Delta(visible));
            }
        };
        let turn = model::stream_turn(
            &app,
            &state,
            &route_id,
            &messages,
            &tools,
            force_workflow,
            reasoning_effort,
            &sender,
            &mut on_text,
        )
        .await?;
        drop(on_text);
        if publish_text {
            external_citations.extend(citation::external_annotations(
                &turn.responses_items,
                &formatter,
                turn_start,
            ));
        }

        if turn.truncated {
            if !selected_workflow {
                return Err("模型在选择本轮对话工作流前截断了响应，请重试。".into());
            }
            if !turn.tool_calls.is_empty() {
                return Err("模型在完成工具调用前截断了响应，请缩小问题范围后重试。".into());
            }
            continuation_count += 1;
            if continuation_count > MAX_CONTINUATIONS {
                return Err("回答达到连续生成上限，尚未完整结束；请缩小范围后重试。".into());
            }
            messages.push(ModelMessage::assistant_turn(
                turn.text,
                Vec::new(),
                turn.responses_items,
            ));
            messages.push(ModelMessage::user(
                "请从刚才中断处继续完整回答，不要重复已经讲过的内容，也不要询问用户是否继续；需要时继续读取 Source，直到回答自然完整地结束。".into(),
            ));
            continue;
        }

        if turn.tool_calls.is_empty() {
            if !selected_workflow {
                return Err("模型未能先选择本轮对话工作流，请重试。".into());
            }
            let tail = formatter.finish();
            if !tail.is_empty() {
                let _ = sender.send(CompletionEvent::Delta(tail));
            }
            for annotation in formatter.annotations() {
                let _ = sender.send(CompletionEvent::Annotation(annotation));
            }
            for annotation in external_citations {
                let _ = sender.send(CompletionEvent::Annotation(annotation));
            }
            let _ = sender.send(CompletionEvent::Complete);
            return Ok(());
        }

        messages.push(ModelMessage::assistant_turn(
            turn.text,
            turn.tool_calls.clone(),
            turn.responses_items,
        ));
        for call in turn.tool_calls {
            tool_count += 1;
            if tool_count > MAX_TOOL_CALLS {
                return Err("本轮调用资料工具次数过多，请缩小问题范围后重试。".into());
            }
            let result = if call.name == "select_workflow" && !selected_workflow {
                selected_workflow = true;
                let (workflow, title, instructions) = skill::select(&call.arguments)?;
                let _ = sender.send(CompletionEvent::Title(title.clone()));
                Ok(json!({"workflow":workflow,"instructions":instructions}).to_string())
            } else if call.name == "select_workflow" {
                Err("工作流已选择；请继续使用资料工具并回答用户。".into())
            } else {
                tool::dispatch(&app, &call, &mut citation_index)
                    .await
                    .map(|execution| {
                        for citation in execution.citations {
                            citations.insert(citation.id.clone(), citation);
                        }
                        execution.content
                    })
            };
            let content = match result {
                Ok(content) => content,
                Err(error) => json!({"error":error}).to_string(),
            };
            messages.push(ModelMessage::tool_result(call.id, content));
        }
    }
    Err("本轮模型或资料检索步骤过多，尚未完成回答。请缩小问题范围后重试。".into())
}

fn workflow_schema() -> Value {
    json!({"name":"select_workflow","description":"每条新用户消息先调用一次：判断最适合的对话类别、简短命名，并取得该类别与学科的阅读指导；取得指导后再调用 Source 工具并回答。","parameters":{"type":"object","properties":{"workflow":{"type":"string","enum":["source_explanation","knowledge_question","reading_discussion","review","other"]},"reading_lens":{"type":"string","enum":["general","math","formal_paper","empirical","humanities"]},"title":{"type":"string","description":"不超过 32 个汉字的简短对话名称"}},"required":["workflow","reading_lens","title"],"additionalProperties":false}})
}

#[cfg(test)]
#[path = "../../../../tests/agent_explanation.rs"]
mod tests;
