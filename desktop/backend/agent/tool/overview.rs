use serde_json::{json, Value};
use uuid::Uuid;

use crate::source::Workspace;

pub fn get(workspace: &Workspace, id: Uuid) -> Result<Value, String> {
    let source = workspace.source_name(id)?;
    Ok(match workspace.overview(id)? {
        Some(markdown) => {
            json!({"source_id":id,"name":source,"initialized":true,"markdown":markdown})
        }
        None => json!({"source_id":id,"name":source,"initialized":false,"markdown":null}),
    })
}
