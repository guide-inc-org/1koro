pub mod file;
pub mod memory;
pub mod shell;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use serde_json::Value;

use crate::llm::{FunctionDef, ToolDef};
use crate::memory::MemoryManager;

/// Result from tool execution (PicoClaw pattern: separate LLM vs user content).
pub struct ToolResult {
    pub for_llm: String,
    pub for_user: Option<String>,
}

/// Context passed to every tool execution.
pub struct ToolContext {
    pub memory: Arc<MemoryManager>,
    pub base_dir: PathBuf,
}

#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;
    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult>;
}

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
    ctx: ToolContext,
}

impl ToolRegistry {
    pub fn new(ctx: ToolContext) -> Self {
        Self {
            tools: HashMap::new(),
            ctx,
        }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn tool_defs(&self) -> Vec<ToolDef> {
        self.tools
            .values()
            .map(|t| ToolDef {
                type_: "function".to_string(),
                function: FunctionDef {
                    name: t.name().to_string(),
                    description: t.description().to_string(),
                    parameters: t.parameters(),
                },
            })
            .collect()
    }

    pub async fn execute(&self, name: &str, args_json: &str) -> Result<ToolResult> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Unknown tool: {name}"))?;
        let args: Value =
            serde_json::from_str(args_json).unwrap_or(Value::Object(Default::default()));
        tool.execute(args, &self.ctx).await
    }
}
