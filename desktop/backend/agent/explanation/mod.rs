//! Explanation workflow and its Agent-owned loop.

mod agent_loop;

use serde_json::Value;
use tauri::AppHandle;
use tokio::sync::mpsc;

use super::{model, tool, AgentState, CompletionEvent, ReasoningEffort};

const TOOL_AVAILABILITY_RULE: &str =
    "本轮实际可调用的工具及联网搜索是否可用，以当前请求注册的 tools 为准；不要编造或调用未注册的工具。";

fn system_prompt() -> String {
    format!(
        "{}\n\n{}",
        include_str!("system.md"),
        TOOL_AVAILABILITY_RULE
    )
}

pub(crate) async fn stream(
    app: AppHandle,
    state: AgentState,
    route_id: String,
    input: Vec<Value>,
    reasoning_effort: ReasoningEffort,
    reply_id: String,
    sender: mpsc::UnboundedSender<CompletionEvent>,
) -> Result<(), String> {
    let route = model::resolve(&state, &route_id)?;
    let mut registry = tool::shared_registry();
    registry.extend([tool::explanation::select_workflow_tool()]);
    let outcome = agent_loop::execute(
        app,
        state,
        route,
        input,
        reasoning_effort,
        system_prompt(),
        registry,
        reply_id,
        sender.clone(),
    )
    .await?;
    match outcome {
        agent_loop::LoopOutcome::Completed(actions) => {
            for action in actions {
                let _ = sender.send(CompletionEvent::OfferUserAction(action));
            }
            let _ = sender.send(CompletionEvent::Complete);
        }
        agent_loop::LoopOutcome::Cancelled => {}
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../../tests/agent_explanation.rs"]
mod tests;
