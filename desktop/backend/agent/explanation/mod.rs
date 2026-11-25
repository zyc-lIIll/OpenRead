//! Main reading conversation: workflow selection, tool loop and citation validation.

mod citation;
mod prompt;
mod skill;

use std::collections::HashMap;

use serde_json::{json, Value};
use tauri::AppHandle;
use tokio::sync::mpsc;

use super::{
    model::{self, Context, Error as ModelError, StreamEvent, ToolChoice},
    tool::{self, SourceCitation},
    AgentState, CompletionEvent, ReasoningEffort,
};

const MAX_MODEL_TURNS: usize = 24;
const MAX_TOOL_CALLS: usize = 40;
const MAX_CONTINUATIONS: usize = 8;

pub(crate) async fn stream(
    app: AppHandle,
    state: AgentState,
    route_id: String,
    mut input: Vec<Value>,
    reasoning_effort: ReasoningEffort,
    sender: mpsc::UnboundedSender<CompletionEvent>,
) -> Result<(), String> {
    let route = model::resolve(&state, &route_id)?;
    let schemas = tool::schemas();
    let instructions = prompt::system_prompt();
    let mut context = Context::default();
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
            let mut tools = schemas.clone();
            tools.push(model::web_search_tool());
            tools
        };
        let publish_text = selected_workflow;
        let turn_start = formatter.raw_chars();
        let request = model::Request::new(
            route.clone(),
            instructions.clone(),
            input.clone(),
            tools,
            if force_workflow {
                ToolChoice::Required
            } else {
                ToolChoice::Auto
            },
            reasoning_effort,
            context.clone(),
        );
        let mut on_event = |event| {
            match event {
                StreamEvent::Text(delta) if publish_text => {
                    let visible = formatter.push(&delta, &citations);
                    if !visible.is_empty() {
                        let _ = sender.send(CompletionEvent::Delta(visible));
                    }
                }
                StreamEvent::Text(_) | StreamEvent::SearchProgress => {}
            }
            Ok(())
        };
        let turn =
            match model::stream_request(&app, &state, &request, sender.closed(), &mut on_event)
                .await
            {
                Ok(turn) => turn,
                Err(ModelError::Cancelled) => return Ok(()),
                Err(ModelError::Failed(message) | ModelError::Request { message, .. }) => {
                    return Err(message)
                }
            };
        drop(on_event);
        context = turn.context.clone();
        if publish_text {
            external_citations.extend(citation::external_annotations(
                &turn.citations,
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
            append_assistant_message(&mut input, &turn.text);
            input.push(model::text_message(
                "user",
                "请从刚才中断处继续完整回答，不要重复已经讲过的内容，也不要询问用户是否继续；需要时继续读取 Source，直到回答自然完整地结束。",
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

        append_assistant_message(&mut input, &turn.text);
        for call in &turn.tool_calls {
            input.push(model::function_call(call));
        }
        for call in turn.tool_calls {
            tool_count += 1;
            if tool_count > MAX_TOOL_CALLS {
                return Err("本轮调用资料工具次数过多，请缩小问题范围后重试。".into());
            }
            let result = if call.name == "select_workflow" && !selected_workflow {
                selected_workflow = true;
                let (workflow, title, instructions) = skill::select(&call.arguments)?;
                let _ = sender.send(CompletionEvent::Title(title));
                Ok(json!({"workflow":workflow,"instructions":instructions}).to_string())
            } else if call.name == "select_workflow" {
                Err("工作流已选择；请继续使用资料工具并回答用户。".to_string())
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
            input.push(model::function_call_output(&call.id, &content));
        }
    }
    Err("本轮模型或资料检索步骤过多，尚未完成回答。请缩小问题范围后重试。".into())
}

fn append_assistant_message(input: &mut Vec<Value>, text: &str) {
    if !text.is_empty() {
        input.push(model::text_message("assistant", text));
    }
}

fn workflow_schema() -> Value {
    json!({"type":"function","name":"select_workflow","description":"每条新用户消息先调用一次：判断最适合的对话类别、简短命名，并取得该类别与学科的阅读指导；取得指导后再调用 Source 工具并回答。","parameters":{"type":"object","properties":{"workflow":{"type":"string","enum":["source_explanation","knowledge_question","reading_discussion","review","other"]},"reading_lens":{"type":"string","enum":["general","math","formal_paper","empirical","humanities"]},"title":{"type":"string","description":"不超过 32 个汉字的简短对话名称"}},"required":["workflow","reading_lens","title"],"additionalProperties":false}})
}

#[cfg(test)]
#[path = "../../../../tests/agent_explanation.rs"]
mod tests;
