# Design Philosophy — Read This First

1koro is a headless AI agent. It speaks only HTTP. It does not know what Slack is. It does not know what Discord is. It does not know what a cron job is.

The core insight: every personal AI agent (OpenClaw, PicoClaw, OxiCrab) embeds channels inside itself. Slack SDK, Discord SDK, Telegram SDK — they all live inside the binary. This is why OpenClaw reached 400,000 lines. Every channel, every provider, every scheduler adds code that has nothing to do with intelligence.

1koro takes the opposite approach. It exposes a single HTTP endpoint (`POST /message`) and an MCP server. That's it. Everything else — Slack integration, Discord integration, scheduling, retries, error handling, routing — is handled by n8n, an external workflow automation platform.

This means:
- Adding a new channel = adding a workflow in n8n's GUI. Zero lines of Rust.
- Adding a new scheduled task = adding a cron trigger in n8n. Zero lines of Rust.
- 1koro's codebase stays small forever. Under 1,000 lines.
- OpenClaw's 400,000 lines of functionality comes free from n8n (400+ integrations), OpenRouter (300+ LLM models), and MCP (unlimited external tools).

When implementing 1koro, ask yourself: "Does this belong in the brain, or in the wiring?" If it's wiring, it belongs in n8n, not in 1koro.

The brain handles: thinking (LLM), remembering (Core Memory + logs), and acting (shell execution via Skills).
The wiring handles: everything else.

Do NOT add Slack code. Do NOT add Discord code. Do NOT add cron scheduling. Do NOT add webhook management. These are n8n's job.
