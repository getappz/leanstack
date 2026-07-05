#!/usr/bin/env node
// Component registry: each entry knows how to check itself and, if needed,
// fix itself. session-start.js and prompt-submit.js just iterate this list —
// neither hook hardcodes per-tool logic, so adding a component means adding
// one entry here, not touching the hooks.
const fs = require('fs');
const path = require('path');
const os = require('os');
const { execSync, spawn } = require('child_process');
const { CLAUDE_DIR } = require('./state.js');

const HOME = os.homedir();
const RULES_DIR = path.join(CLAUDE_DIR, 'rules');
const SETTINGS_PATH = path.join(CLAUDE_DIR, 'settings.json');
const CAVEMAN_CONFIG = path.join(HOME, '.config', 'caveman', 'config.json');
const PONYTAIL_CONFIG = path.join(HOME, '.config', 'ponytail', 'config.json');
const LEANCTX_LOG = path.join(CLAUDE_DIR, 'leanstack', 'leanctx-install.log');

function enabledPlugins() {
  try { return JSON.parse(fs.readFileSync(SETTINGS_PATH, 'utf8')).enabledPlugins || {}; }
  catch (_) { return {}; }
}

function writePinnedMode(configPath) {
  try {
    let current = null;
    try { current = JSON.parse(fs.readFileSync(configPath, 'utf8')).defaultMode; } catch (_) {}
    if (current) return false;
    fs.mkdirSync(path.dirname(configPath), { recursive: true });
    fs.writeFileSync(configPath, '{"defaultMode": "ultra"}\n');
    return true;
  } catch (_) { return false; }
}

const RULE_FILES = {
  'exa.md': 'Use Exa MCP tools (web_search_exa, get_code_context_exa, company_research_exa) for internet search. Skip WebFetch/WebSearch/websearch-agent — Exa covers it for every session and subagent.',
  'git.md': 'Commit messages are the message only: no "Generated with Claude Code", no Co-Authored-By trailer. `git commit -m "..."` format.',
  'lean-ctx.md': 'Prefer lean-ctx over native tools: ctx_read > Read/cat, ctx_shell > Bash, ctx_search > Grep, ctx_glob > Glob. Orient with ctx_compose before exploring unfamiliar code — one call instead of a search-read-search chain. ctx_callgraph answers "who calls X", not grep. Same rule for every subagent.',
};

module.exports = [
  {
    id: 'rules',
    needsConsent: false,
    describe: () => 'usage rules in ~/.claude/rules/',
    check() {
      return Object.keys(RULE_FILES).every(name => fs.existsSync(path.join(RULES_DIR, name)));
    },
    apply() {
      fs.mkdirSync(RULES_DIR, { recursive: true });
      const written = [];
      for (const [name, text] of Object.entries(RULE_FILES)) {
        const p = path.join(RULES_DIR, name);
        if (!fs.existsSync(p)) {
          fs.writeFileSync(p, text + '\n');
          written.push(name);
        }
      }
      return written.length ? `rules written: ${written.join(', ')}` : 'rules already present';
    },
  },
  {
    id: 'leanctx',
    needsConsent: true,
    describe: () => 'lean-ctx (context compression) — npm install -g lean-ctx-bin && lean-ctx onboard',
    check() {
      try { execSync('lean-ctx --version', { stdio: 'pipe' }); return true; } catch (_) { return false; }
    },
    apply() {
      if (fs.existsSync(LEANCTX_LOG)) return 'lean-ctx install already triggered — check ' + LEANCTX_LOG;
      fs.mkdirSync(path.dirname(LEANCTX_LOG), { recursive: true });
      const cmd = 'npm install -g lean-ctx-bin && lean-ctx onboard';
      const fd = fs.openSync(LEANCTX_LOG, 'a');
      const child = process.platform === 'win32'
        ? spawn('cmd', ['/c', cmd], { detached: true, stdio: ['ignore', fd, fd] })
        : spawn('sh', ['-c', cmd], { detached: true, stdio: ['ignore', fd, fd] });
      child.unref();
      return 'lean-ctx install started in background — ready next session (log: ' + LEANCTX_LOG + ')';
    },
  },
  {
    id: 'ponytail-plugin',
    needsConsent: true,
    describe: () => 'Ponytail plugin — claude plugin marketplace add DietrichGebert/ponytail && claude plugin install ponytail@ponytail',
    check() {
      return enabledPlugins()['ponytail@ponytail'] === true;
    },
    apply() {
      execSync('claude plugin marketplace add DietrichGebert/ponytail', { stdio: 'pipe' });
      execSync('claude plugin install ponytail@ponytail', { stdio: 'pipe' });
      return 'Ponytail plugin installed — restart to activate';
    },
  },
  {
    id: 'ponytail-mode',
    needsConsent: false,
    describe: () => 'pin Ponytail to ultra mode',
    check() {
      try { return !!JSON.parse(fs.readFileSync(PONYTAIL_CONFIG, 'utf8')).defaultMode; } catch (_) { return false; }
    },
    apply() {
      return writePinnedMode(PONYTAIL_CONFIG) ? 'Ponytail pinned to ultra' : 'Ponytail mode already set';
    },
  },
  {
    id: 'caveman-mode',
    needsConsent: false,
    describe: () => 'pin Caveman to ultra mode',
    check() {
      // Only relevant once Caveman itself is installed — otherwise there's
      // nothing to pin, so treat as satisfied rather than perpetually pending.
      if (enabledPlugins()['caveman@caveman'] !== true) return true;
      try { return JSON.parse(fs.readFileSync(CAVEMAN_CONFIG, 'utf8')).defaultMode === 'ultra'; } catch (_) { return false; }
    },
    apply() {
      return writePinnedMode(CAVEMAN_CONFIG) ? 'Caveman pinned to ultra' : 'Caveman mode already set';
    },
  },
];
