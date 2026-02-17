use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolResult};

pub struct ReadFileTool;

#[async_trait::async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }
    fn description(&self) -> &str {
        "Read file contents within the memory directory (~/.1koro)"
    }
    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": { "path": { "type": "string", "description": "File path relative to memory directory (~/.1koro)" } }, "required": ["path"] })
    }
    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let path_str = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing required 'path' parameter"))?;
        let path = if path_str.starts_with('/') {
            std::path::PathBuf::from(path_str)
        } else {
            ctx.base_dir.join(path_str)
        };

        let canonical = match path.canonicalize() {
            Ok(p) => p,
            Err(e) => {
                return Ok(ToolResult {
                    for_llm: format!("Error: cannot resolve path: {e}"),
                });
            }
        };
        let base_canonical = match ctx.base_dir.canonicalize() {
            Ok(p) => p,
            Err(e) => {
                return Ok(ToolResult {
                    for_llm: format!("Error: cannot resolve base dir: {e}"),
                });
            }
        };
        if !canonical.starts_with(&base_canonical) {
            return Ok(ToolResult {
                for_llm: format!("Error: path outside memory directory: {}", path.display()),
            });
        }

        match std::fs::read_to_string(&canonical) {
            Ok(content) => Ok(ToolResult { for_llm: content }),
            Err(e) => Ok(ToolResult {
                for_llm: format!("Error reading {}: {e}", path.display()),
            }),
        }
    }
}
