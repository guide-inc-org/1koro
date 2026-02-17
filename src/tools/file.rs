use anyhow::Result;
use serde_json::{json, Value};

use super::{Tool, ToolContext, ToolResult};

pub struct ReadFileTool;

#[async_trait::async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str { "read_file" }
    fn description(&self) -> &str { "Read file contents. Use for loading skill details or workspace files." }
    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": { "path": { "type": "string", "description": "File path (absolute or relative to workspace)" } }, "required": ["path"] })
    }
    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let path_str = args["path"].as_str().unwrap_or("");
        let path = if path_str.starts_with('/') {
            std::path::PathBuf::from(path_str)
        } else {
            ctx.base_dir.join(path_str)
        };
        match std::fs::read_to_string(&path) {
            Ok(content) => Ok(ToolResult { for_llm: content }),
            Err(e) => Ok(ToolResult { for_llm: format!("Error reading {}: {e}", path.display()) }),
        }
    }
}
