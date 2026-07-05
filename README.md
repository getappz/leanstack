<div align="center">

# leanstack

**lean-ctx + engram powered token-saving stack across Claude Code, Codex, Cursor, Windsurf, VS Code, Cline, and Continue.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

</div>

---

## What this is

A cross-tool setup for two non-overlapping layers, plus two companions where the host
supports them:

| Layer | What it compresses | Tool |
|---|---|---|
| **lean-ctx** | tool I/O *within* a session — reads, shell output, search, up to 99% | [yvgude/lean-ctx](https://github.com/yvgude/lean-ctx) |
| **engram** | knowledge *across* sessions — decisions, facts, preferences that survive a session ending | [Gentleman-Programming/engram](https://github.com/Gentleman-Programming/engram) |
| **Caveman** (Claude Code only) | conversation verbosity, ~75% | companion plugin |
| **Ponytail** (Claude Code only) | code-writing over-engineering, 47-77% on code tasks | companion plugin |

lean-ctx and engram aren't substitutes for each other — one saves tokens inside a
session, the other saves the re-explaining tax across sessions. Neither has a
credible replacement for the other's job.

Every host gets one of three tiers, matched to what that host actually supports —
no auto-install machinery built against an unverified surface.

## Tier 1 — live plugin (marketplace-installable, real hooks)

**Claude Code** and **Codex** both have a real `SessionStart`/`UserPromptSubmit`-shaped
hook system and a plugin marketplace. Same hook scripts run on both (Codex's loader
honors `${CLAUDE_PLUGIN_ROOT}`, confirmed via `biefan/anchor`, a plugin that already
does this in production).

```
/plugin marketplace add getappz/leanstack
/plugin install leanstack@leanstack
/reload-plugins
```

Detection-first: a component registry (`src/hooks/components.js`) checks what's already
configured and skips it. **Consent-gated installs**: the first session only *lists* what's
missing and the exact command each would run. Nothing that installs a package or plugin
runs until you type `/leanstack confirm`. Rule files are the one exception — those are
just usage guidance, not installs, so they write on first run.

**engram on Claude Code** installs via its own plugin marketplace (`Gentleman-Programming/engram`)
— same pattern as the Ponytail companion, and engram's own documented recommended path.

**lean-ctx** auto-installs via `npm install -g lean-ctx-bin && lean-ctx onboard`.

**engram elsewhere** (Codex/Cursor/Tier 2) auto-installs via `go install` (if Go is on
`PATH`) or Homebrew (macOS/Linux, if present) — never the prebuilt Windows binary, which
the project's own docs say gets flagged as a false positive by some antivirus engines. If
neither path is available, the exact command is printed instead of silently downloading
something that might trip your AV.

## Tier 1.5 — real hooks, no marketplace (Cursor)

Cursor has the same kind of hook system (`.cursor/hooks.json`, events `sessionStart`/
`beforeSubmitPrompt`) but no plugin marketplace to install from, so the hook scripts
get copied into your project instead of loaded from an installed plugin:

```bash
npx github:getappz/leanstack cursor
```

Writes `.cursor/leanstack/*.js` (the same hook scripts, host-tagged `cursor`),
`.cursor/hooks.json`, `.cursor/rules/leanstack.mdc`, registers lean-ctx in
`~/.cursor/mcp.json`, and runs `engram setup cursor` (engram's own native integration
for Cursor) — both gated on the respective tool being installed already or auto-installed
via the same safe paths as Tier 1.

## Tier 2 — one-shot setup script (no hooks at all)

**Windsurf**, **VS Code/Copilot**, **Cline**, and **Continue** have no programmable
hook/lifecycle mechanism — but their MCP config and rules files are all scriptable.
Running the script *is* the consent; there's no live confirm-gate because there's no
live hook to gate.

```bash
npx github:getappz/leanstack            # auto-detects installed tools
npx github:getappz/leanstack windsurf   # or force a specific one
```

| Tool | lean-ctx | engram | Rules file |
|---|---|---|---|
| Windsurf | `~/.codeium/windsurf/mcp_config.json` | `engram setup windsurf` (native) | `.windsurf/rules/leanstack.md` |
| VS Code/Copilot | via `code --add-mcp` | `engram setup vscode-copilot` (native) | `.github/copilot-instructions.md` |
| Cline | `~/.cline/mcp.json` | `~/.cline/mcp.json` (manual entry — no native `engram setup cline`) | `.clinerules/leanstack.md` |
| Continue | `.continue/mcpServers/leanstack.json` | `.continue/mcpServers/engram.json` (manual — no native subcommand) | — (no dedicated rules convention found) |

All writes are skip-if-exists — never clobbers something already there. If a tool
itself isn't installed yet, MCP registration is skipped with a printed install command
instead of registering a broken server entry.

## Tier 3 — docs only (everyone else, e.g. Aider)

No MCP support, no hooks: copy `AGENTS.md` into your project root.

```bash
curl -sL https://raw.githubusercontent.com/getappz/leanstack/main/AGENTS.md > AGENTS.md
```

---

## Architecture

```
src/
├── rule-text.js         # shared rule copy (Exa, git, lean-ctx, engram usage)
├── engram-install.js    # engram's safe-install logic (go install/brew, never
│                         # the AV-flagged prebuilt Windows binary), shared by
│                         # components.js and bin/setup.js
└── hooks/
    ├── state.js          # single JSON state blob (~/.leanstack/state.json), host-neutral
    ├── components.js      # registry: each entry checks + fixes itself, host-aware
    ├── session-start.js   # SessionStart hook — argv[2] = host ('claude-code'|'codex'|'cursor')
    └── prompt-submit.js   # UserPromptSubmit hook — /leanstack confirm|on|off
bin/
└── setup.js              # one-shot script for Cursor/Windsurf/VS Code/Cline/Continue
```

Adding a new managed component means adding one entry to `components.js` — neither
hook hardcodes per-tool logic. Adding a new hook-less tool means adding one entry to
`bin/setup.js`'s `TOOLS` map.

---

## What Gets Created

**Claude Code**: `~/.claude/rules/{exa,git,lean-ctx,engram}.md` (global), `~/.config/{caveman,ponytail}/config.json`, `~/.leanstack/state.json`.

**Codex**: project-local `AGENTS.md` (only if absent), `~/.leanstack/state.json`.

**Cursor**: project-local `.cursor/rules/leanstack.mdc`, `.cursor/hooks.json`, `.cursor/leanstack/*.js`, `~/.cursor/mcp.json`, `~/.leanstack/state.json`.

Nothing is created if it already exists.

---

## Uninstall

**Claude Code / Codex**: `/uninstall-plugin leanstack`, then:
```bash
rm ~/.claude/rules/exa.md ~/.claude/rules/git.md ~/.claude/rules/lean-ctx.md ~/.claude/rules/engram.md
rm -rf ~/.leanstack
rm ~/.config/ponytail/config.json  # ~/.config/caveman/config.json too if you want that reset
```

**Cursor**: `rm -rf .cursor/leanstack .cursor/hooks.json .cursor/rules/leanstack.mdc`

**Tier 2 tools**: remove the specific files listed in the table above.

Ponytail/Caveman/engram plugins themselves stay installed (uninstall separately if wanted).

---

<div align="center">

MIT License

</div>
