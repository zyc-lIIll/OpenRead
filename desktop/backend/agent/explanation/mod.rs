//! Explanation workflow and its Agent-owned loop.

mod agent_loop;
mod prompt;
mod skill;
pub(crate) mod tools;

use serde_json::Value;
use tauri::AppHandle;
use tokio::sync::mpsc;

use super::{model, tool, AgentState, CompletionEvent, ReasoningEffort};

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
    registry.extend([tools::select_workflow_tool()]);
    let outcome = agent_loop::execute(
        app,
        state,
        route,
        input,
        reasoning_effort,
        prompt::system_prompt(),
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
