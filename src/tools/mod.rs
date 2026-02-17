pub mod shell;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use serde_json::{Value, json};

use crate::memory::MemoryManager;

#[derive(Debug)]
pub struct ToolResult {
    pub for_llm: String,
}

pub struct ToolContext {
    pub memory: Arc<MemoryManager>,
    pub base_dir: PathBuf,
}

fn require_str<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args[key]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing required '{key}' parameter"))
}

fn ok(s: impl Into<String>) -> Result<ToolResult> {
    Ok(ToolResult { for_llm: s.into() })
}

// --- Tool definitions as enum (no trait, no async_trait, no Box<dyn>) ---

pub enum ToolKind {
    SearchLogs,
    ReadCoreMemory,
    UpdateCoreMemory,
    ReadDailyLog,
    WriteSummary,
    AppendLog,
    ReadFile,
    Shell(std::time::Duration),
}

impl ToolKind {
    fn name(&self) -> &'static str {
        match self {
            Self::SearchLogs => "search_logs",
            Self::ReadCoreMemory => "read_core_memory",
            Self::UpdateCoreMemory => "update_core_memory",
            Self::ReadDailyLog => "read_daily_log",
            Self::WriteSummary => "write_summary",
            Self::AppendLog => "append_note",
            Self::ReadFile => "read_file",
            Self::Shell(_) => "shell",
        }
    }

    fn spec(&self) -> (&'static str, &'static str, Value) {
        match self {
            Self::SearchLogs => (
                "search_logs",
                "Search past conversation logs and daily notes for a keyword",
                json!({"type":"object","properties":{
                    "query":{"type":"string"},
                    "limit":{"type":"integer","description":"Max results (default 100)"}
                },"required":["query"]}),
            ),
            Self::ReadCoreMemory => (
                "read_core_memory",
                "Read a core memory file (identity.md, user.md, or state.md)",
                json!({"type":"object","properties":{
                    "file":{"type":"string","enum":["identity.md","user.md","state.md"]}
                },"required":["file"]}),
            ),
            Self::UpdateCoreMemory => (
                "update_core_memory",
                "Update user.md or state.md with new content",
                json!({"type":"object","properties":{
                    "file":{"type":"string","enum":["user.md","state.md"]},
                    "content":{"type":"string"}
                },"required":["file","content"]}),
            ),
            Self::ReadDailyLog => (
                "read_daily_log",
                "Read a daily log by date (YYYY-MM-DD)",
                json!({"type":"object","properties":{
                    "date":{"type":"string"}
                },"required":["date"]}),
            ),
            Self::WriteSummary => (
                "write_summary",
                "Write a weekly or monthly summary. period='weekly' id='2026-W08', or period='monthly' id='2026-02'",
                json!({"type":"object","properties":{
                    "period":{"type":"string","enum":["weekly","monthly"]},
                    "id":{"type":"string","description":"e.g. '2026-W08' or '2026-02'"},
                    "content":{"type":"string"}
                },"required":["period","id","content"]}),
            ),
            Self::AppendLog => (
                "append_note",
                "Append a note to today's daily log",
                json!({"type":"object","properties":{
                    "text":{"type":"string"}
                },"required":["text"]}),
            ),
            Self::ReadFile => (
                "read_file",
                "Read file contents within the memory directory (~/.1koro)",
                json!({"type":"object","properties":{
                    "path":{"type":"string","description":"File path relative to memory directory (~/.1koro)"}
                },"required":["path"]}),
            ),
            Self::Shell(_) => (
                "shell",
                "Execute a shell command (runs in memory directory)",
                json!({"type":"object","properties":{
                    "command":{"type":"string","description":"Shell command to execute"}
                },"required":["command"]}),
            ),
        }
    }

    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult> {
        match self {
            Self::SearchLogs => {
                let query = require_str(&args, "query")?;
                if query.is_empty() {
                    return ok("Error: 'query' must not be empty");
                }
                let limit = args["limit"].as_u64().unwrap_or(100) as usize;
                let results = ctx.memory.search_logs(query, limit)?;
                ok(if results.is_empty() {
                    "No results found.".into()
                } else {
                    results.join("\n")
                })
            }
            Self::ReadCoreMemory => ok(ctx.memory.read_core(require_str(&args, "file")?)?),
            Self::UpdateCoreMemory => {
                let file = require_str(&args, "file")?;
                let content = require_str(&args, "content")?;
                if content.is_empty() {
                    return ok("Error: 'content' must not be empty");
                }
                ctx.memory.write_core(file, content)?;
                ok(format!("Updated {file}"))
            }
            Self::ReadDailyLog => {
                let date = require_str(&args, "date")?;
                ok(ctx
                    .memory
                    .read_daily_log(date)?
                    .unwrap_or_else(|| format!("No log for {date}")))
            }
            Self::WriteSummary => {
                let period = require_str(&args, "period")?;
                let id = require_str(&args, "id")?;
                let content = require_str(&args, "content")?;
                if id.is_empty() || content.is_empty() {
                    return ok("Error: 'id' and 'content' must not be empty");
                }
                match period {
                    "weekly" => ctx.memory.write_weekly_summary(id, content)?,
                    "monthly" => ctx.memory.write_monthly_summary(id, content)?,
                    _ => return ok(format!("Unknown period: {period}")),
                }
                ok(format!("Written {period} summary: {id}"))
            }
            Self::AppendLog => {
                let text = require_str(&args, "text")?;
                if text.is_empty() {
                    return ok("Error: 'text' must not be empty");
                }
                ctx.memory.append_log(text)?;
                ok("Note appended.")
            }
            Self::ReadFile => {
                let path_str = require_str(&args, "path")?;
                let path = if path_str.starts_with('/') {
                    PathBuf::from(path_str)
                } else {
                    ctx.base_dir.join(path_str)
                };
                let canonical = match path.canonicalize() {
                    Ok(p) => p,
                    Err(e) => return ok(format!("Error: cannot resolve path: {e}")),
                };
                let base = match ctx.base_dir.canonicalize() {
                    Ok(p) => p,
                    Err(e) => return ok(format!("Error: cannot resolve base dir: {e}")),
                };
                if !canonical.starts_with(&base) {
                    return ok(format!(
                        "Error: path outside memory directory: {}",
                        path.display()
                    ));
                }
                match std::fs::read_to_string(&canonical) {
                    Ok(c) => ok(c),
                    Err(e) => ok(format!("Error reading {}: {e}", path.display())),
                }
            }
            Self::Shell(timeout) => shell::execute(&args, ctx, *timeout).await,
        }
    }
}

// --- Registry ---

pub struct ToolRegistry {
    tools: Vec<ToolKind>,
    ctx: ToolContext,
}

impl ToolRegistry {
    pub fn new(ctx: ToolContext) -> Self {
        Self {
            tools: Vec::new(),
            ctx,
        }
    }

    pub fn add(&mut self, kind: ToolKind) {
        let name = kind.name();
        self.tools.retain(|t| t.name() != name);
        self.tools.push(kind);
    }

    pub fn tool_defs(&self) -> Vec<Value> {
        let mut defs: Vec<Value> = self
            .tools
            .iter()
            .map(|t| {
                let (name, desc, params) = t.spec();
                json!({"type":"function","function":{"name":name,"description":desc,"parameters":params}})
            })
            .collect();
        defs.sort_by(|a, b| {
            let an = a["function"]["name"].as_str().unwrap_or("");
            let bn = b["function"]["name"].as_str().unwrap_or("");
            an.cmp(bn)
        });
        defs
    }

    pub fn tool_defs_mcp(&self) -> Vec<Value> {
        self.tools
            .iter()
            .map(|t| {
                let (name, desc, params) = t.spec();
                json!({"name":name,"description":desc,"inputSchema":params})
            })
            .collect()
    }

    pub async fn execute(&self, name: &str, args_json: &str) -> Result<ToolResult> {
        let tool = self
            .tools
            .iter()
            .find(|t| t.name() == name)
            .ok_or_else(|| anyhow::anyhow!("Unknown tool: {name}"))?;
        let args: Value = serde_json::from_str(args_json)
            .map_err(|e| anyhow::anyhow!("Invalid tool arguments for {name}: {e}"))?;
        tool.execute(args, &self.ctx).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_kind_names_are_unique() {
        let all = [
            ToolKind::SearchLogs,
            ToolKind::ReadCoreMemory,
            ToolKind::UpdateCoreMemory,
            ToolKind::ReadDailyLog,
            ToolKind::WriteSummary,
            ToolKind::AppendLog,
            ToolKind::ReadFile,
            ToolKind::Shell(std::time::Duration::from_secs(30)),
        ];
        let mut names: Vec<&str> = all.iter().map(|t| t.name()).collect();
        let len_before = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), len_before, "ToolKind names must be unique");
    }

    #[test]
    fn test_tool_kind_spec_consistency() {
        // spec().0 must match name() for all variants
        let all = [
            ToolKind::SearchLogs,
            ToolKind::ReadCoreMemory,
            ToolKind::UpdateCoreMemory,
            ToolKind::ReadDailyLog,
            ToolKind::WriteSummary,
            ToolKind::AppendLog,
            ToolKind::ReadFile,
            ToolKind::Shell(std::time::Duration::from_secs(30)),
        ];
        for t in &all {
            assert_eq!(t.name(), t.spec().0, "name() and spec().0 must match");
        }
    }

    #[test]
    fn test_registry_dedup_on_add() {
        let ctx = ToolContext {
            memory: Arc::new(
                crate::memory::MemoryManager::new(&crate::config::MemoryConfig::default()).unwrap(),
            ),
            base_dir: std::env::temp_dir(),
        };
        let mut reg = ToolRegistry::new(ctx);
        reg.add(ToolKind::SearchLogs);
        reg.add(ToolKind::SearchLogs); // duplicate
        assert_eq!(
            reg.tools
                .iter()
                .filter(|t| t.name() == "search_logs")
                .count(),
            1,
            "duplicate add must not create two entries"
        );
    }

    #[test]
    fn test_registry_unknown_tool() {
        let ctx = ToolContext {
            memory: Arc::new(
                crate::memory::MemoryManager::new(&crate::config::MemoryConfig::default()).unwrap(),
            ),
            base_dir: std::env::temp_dir(),
        };
        let reg = ToolRegistry::new(ctx);
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let result = rt.block_on(reg.execute("nonexistent", "{}"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown tool"));
    }
}
