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

pub struct ToolResult {
    pub for_llm: String,
}

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
        Self { tools: HashMap::new(), ctx }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn tool_defs(&self) -> Vec<ToolDef> {
        let mut defs: Vec<_> = self.tools.values().map(|t| ToolDef {
            type_: "function".into(),
            function: FunctionDef {
                name: t.name().into(),
                description: t.description().into(),
                parameters: t.parameters(),
            },
        }).collect();
        defs.sort_by(|a, b| a.function.name.cmp(&b.function.name));
        defs
    }

    pub async fn execute(&self, name: &str, args_json: &str) -> Result<ToolResult> {
        let tool = self.tools.get(name).ok_or_else(|| anyhow::anyhow!("Unknown tool: {name}"))?;
        let args: Value = serde_json::from_str(args_json)
            .map_err(|e| anyhow::anyhow!("Invalid tool arguments for {name}: {e}"))?;
        tool.execute(args, &self.ctx).await
    }
}
