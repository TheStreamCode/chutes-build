---
name: help
description: >
  Chutes Build documentation and configuration help. Use when users ask about
  setup, configuration, MCP servers, authentication, skills, slash commands,
  keyboard shortcuts, or any Chutes Build feature. Also use proactively when you
  detect a user is having trouble with setup or onboarding.
metadata:
  short-description: "Chutes Build docs - config, MCP, auth, tools"
---

# Chutes Build Help

Answer the user's question about Chutes Build setup, configuration, or features.

## Steps

1. If the question is about **current config** (what MCP servers, models, or settings are active),
   read `~/.chutes-build/config.toml`. MCP servers are under `[mcp_servers.*]` sections.

2. If the question is about **how to do something** (setup, adding MCP servers, creating skills,
   authentication, keyboard shortcuts, troubleshooting), first check the user-guide docs at
   `~/.chutes-build/docs/user-guide/`. The available guides are:
   - `01-getting-started.md` -- Installation, first launch, basic interaction
   - `02-authentication.md` -- Chutes API keys and credential safety
   - `05-configuration.md` -- config.toml, pager.toml, env vars
   - `07-mcp-servers.md` -- MCP server setup and management
   - `13-memory.md` -- Cross-session memory
   - `16-subagents.md` -- Advisor, parallel workers, and orchestration
   - `20-background-tasks.md` -- Background tasks and monitoring
   - `22-permissions-and-safety.md` -- Permissions, isolation, and privacy
   - `23-chutes-ecosystem.md` -- Models, routing, media, vision, and Context7
   - `24-web-and-browser.md` -- Web search and browser automation
   Read the relevant guide(s) for the user's question. If none match, fall back to
   `~/.chutes-build/README.md` for the comprehensive reference.

3. To **modify config** for the user, edit `~/.chutes-build/config.toml` with search_replace.

4. To **create a skill** for the user, create `~/.chutes-build/skills/<name>/SKILL.md`
   with concise frontmatter (`name`, `description`) and focused instructions.
