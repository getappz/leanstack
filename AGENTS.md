# agentflare rules

Static fallback for agents with no MCP support and no hook mechanism (e.g. Aider).
Everything else (Claude Code, Codex, Cursor, Windsurf, VS Code/Copilot, Cline,
Continue) gets a real integration via the `agentflare` CLI — see
https://github.com/getappz/agentflare. Use this file only if your tool isn't
one of those.

## Context compression — lean-ctx

Prefer [lean-ctx](https://github.com/yvgude/lean-ctx) tools over native equivalents:
read files with its compressed reader, run shell commands through its
compression wrapper, search code with its search/callgraph tools, orient in
unfamiliar code with its composed-context command before exploring.

```bash
npm install -g lean-ctx-bin && lean-ctx onboard
```


## Web search

Use Exa for internet search when available — free-tier, no API key required.

## Git

Never add "Generated with Claude Code" or "Co-Authored-By: Claude" signatures.
Commit messages are the message only.
