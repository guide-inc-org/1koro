# 1koro（イチコロ）

A Rust-powered personal AI agent that never forgets.

## What is this?

1koro is a headless AI agent. It runs 24/7 on a tiny VPS, remembers everything you tell it, and acts on your behalf.

It speaks only HTTP.

## The Problem

Every personal AI agent today — OpenClaw, PicoClaw, OxiCrab — embeds communication channels directly into its codebase. Slack SDK, Discord SDK, Telegram SDK, WhatsApp bridges, cron schedulers, webhook managers. OpenClaw reached **400,000 lines of code**. Most of it has nothing to do with intelligence.

## The Idea

What if the agent didn't know what Slack was?

```
┌─────────────────────────────────────────┐
│           Inputs (anything)             │
│                                         │
│  Slack / Discord / CLI / iPhone / Email │
│  GitHub / Calendar / Cron / Voice / MCP │
└──────────────────┬──────────────────────┘
                   │
                   ▼
┌──────────────────────────────────────────┐
│              n8n (wiring)                │
│                                          │
│  Receives events from any channel,       │
│  converts them to POST /message,         │
│  routes responses back.                  │
│  400+ integrations. GUI. Self-hosted.    │
└──────────────────┬───────────────────────┘
                   │ POST /message
                   ▼
┌──────────────────────────────────────────┐
│            1koro (the brain)             │
│                                          │
│  HTTP API + MCP + LLM + Memory + Shell   │
│                                          │
│  Rust single binary. ~1200 lines.        │
│  Slack? Never heard of it.               │
│  Discord? What's that?                   │
│  I only speak HTTP.                      │
└──────────────────────────────────────────┘
```

1koro exposes a single HTTP endpoint and an MCP server. That's it. Channel integration, scheduling, retries, error handling, routing — all handled by [n8n](https://n8n.io), an open-source workflow automation platform with 400+ integrations.

**Adding a new channel = adding a workflow in n8n's GUI. Zero lines of Rust.**

## Why This Works

| What OpenClaw built (400K lines) | What 1koro does |
|---|---|
| 8 channel integrations | n8n handles it (0 lines) |
| 14 LLM providers | OpenRouter handles it (1 provider, 300+ models) |
| Webhook management | n8n handles it |
| Cron scheduling | n8n handles it |
| Retry & error handling | n8n handles it |
| Web UI for settings | n8n's GUI |
| Plugin system | Markdown files + shell execution |

1koro's codebase stays small forever. The 400,000 lines of functionality comes free from the ecosystem.

## What 1koro Actually Does

### The Brain

- **Thinks** — sends messages to an LLM (MiniMax M2.5 via OpenRouter) and returns responses
- **Remembers** — maintains Core Memory (who you are, what matters now) and full conversation logs that are never deleted
- **Acts** — reads Skill files (plain Markdown) and executes shell commands on the server

### Core Memory

```
~/.1koro/core/
├── identity.md    ← agent personality
├── user.md        ← who you are
└── state.md       ← what matters right now (auto-updated)
```

Small enough to fit in every LLM context window. Updated nightly by the LLM itself.

### Skills = Markdown

```markdown
# deploy
## Steps
1. git pull main
2. cargo build --release
3. Rename old binary to ichikoro.bak
4. Place new binary
5. systemctl restart 1koro
6. Wait 30s, health check
7. If failed, rollback to ichikoro.bak
```

Say "deploy" and 1koro reads this file, generates shell commands, and executes them. Adding a new skill = adding a Markdown file. No code.

### External Knowledge (Bookshelves)

1koro doesn't need to store everything inside itself. It can reach out to external knowledge sources via MCP:

```
1koro (MCP client)
├──→ Obsidian MCP Server
├──→ GitHub MCP Server
├──→ Google Drive MCP
└──→ Postgres MCP
```

Core Memory lives inside 1koro (the brain needs its own memory). Everything else is a bookshelf — available when needed, not loaded by default.

## API

### `POST /message`

The only endpoint. The only way in.

```bash
curl -X POST http://100.64.x.x:3000/message \
  -H "Content-Type: application/json" \
  -d '{"text": "What did I work on last week?"}'
```

```json
{
  "text": "Last week you focused on three things: ...",
  "actions": []
}
```

### MCP Server

For Claude Desktop / Claude mobile app. Exposes memory as tools:

- `read_core_memory`
- `update_core_memory`
- `search_logs`
- `read_daily_log`

## Infrastructure

| Component | Spec | Cost |
|---|---|---|
| 1koro | EC2 t4g.micro (ARM, 1GB RAM) | ~$4/mo |
| n8n | Docker on same EC2 | $0 |
| LLM | OpenRouter → MiniMax M2.5 | ~$1-3/mo |
| Network | Headscale VPN (not exposed to internet) | $0 |

**Total: ~$5-7/month** for a 24/7 personal AI agent that never forgets.

## n8n Workflow Examples

### Slack ↔ 1koro

```
[Slack Trigger] → [HTTP Request: POST /message] → [Slack: Send Message]
```

### Heartbeat (every 30 min)

```
[Schedule: */30 * * * *] → [HTTP Request: POST /message {"text":"heartbeat"}]
```

### Daily Summary (3 AM)

```
[Schedule: 0 3 * * *] → [HTTP Request: POST /message {"text":"generate today's summary"}]
```

## Design Principles

1. **The brain should only be a brain.** No channel code. No scheduling. No webhook management. HTTP in, HTTP out.
2. **Memory is the moat.** Any chatbot can think. Only 1koro remembers everything and never forgets.
3. **Grow through content, not code.** New skills = Markdown files. New channels = n8n workflows. New knowledge = MCP connections. The Rust binary stays the same.
4. **Small is a feature.** ~1,200 lines of Rust. Compiles in seconds. Runs on the smallest EC2 instance. Uses less RAM than your text editor.

## Tech Stack

| Purpose | Choice |
|---|---|
| Language | Rust |
| Async runtime | tokio |
| HTTP server | axum |
| HTTP client | reqwest |
| MCP server | rmcp |
| LLM provider | OpenRouter (MiniMax M2.5 default) |
| Config | TOML |
| Wiring | n8n (external, self-hosted) |

## License

MIT
