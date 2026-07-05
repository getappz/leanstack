#!/usr/bin/env node
// One-shot setup for tools with no hook mechanism (Windsurf, VS Code/Copilot,
// Cline, Continue) plus Cursor (which has real hooks but no marketplace to
// install a plugin from, so its hook scripts get copied in here too).
// Running this script IS the consent — no confirm-gate needed, unlike the
// live Claude Code/Codex plugin hooks which install without being asked to run.
const fs = require('fs');
const path = require('path');
const os = require('os');
const { execSync } = require('child_process');
const RULE_TEXT = require('../src/rule-text.js');
const engramInstall = require('../src/engram-install.js');

const HOME = os.homedir();
const CWD = process.cwd();
const LEANCTX_MCP_ENTRY = { command: 'lean-ctx', args: ['serve'] };
const ENGRAM_MCP_ENTRY = { command: 'engram', args: ['mcp', '--tools=agent'] };
const RULES_BLOCK = Object.values(RULE_TEXT).join('\n\n') + '\n';
const ENGRAM_LOG = path.join(HOME, '.leanstack', 'engram-install.log');

function which(cmd) {
  try {
    execSync(process.platform === 'win32' ? `where ${cmd}` : `which ${cmd}`, { stdio: 'pipe' });
    return true;
  } catch (_) { return false; }
}

function leanctxInstalled() {
  try { execSync('lean-ctx --version', { stdio: 'pipe' }); return true; } catch (_) { return false; }
}

function mergeJson(filePath, patch) {
  let existing = {};
  try { existing = JSON.parse(fs.readFileSync(filePath, 'utf8')); } catch (_) {}
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, JSON.stringify({ ...existing, ...patch }, null, 2) + '\n');
}

function writeIfAbsent(filePath, content) {
  if (fs.existsSync(filePath)) return false;
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, content);
  return true;
}

function leanctxNote() {
  return leanctxInstalled()
    ? null
    : 'lean-ctx not installed — skipped MCP registration. Run: npm install -g lean-ctx-bin && lean-ctx onboard';
}

// engram has a native `engram setup <name>` for some tools; for the rest we
// register its MCP command manually (`engram mcp`, stdio, same as the tools
// engram's own docs mark as auto-launched). Auto-installs engram itself
// (go install/brew) if missing, same as engram-install.js does for the hook path.
function engramSetup(nativeName, mcpConfigPath) {
  if (engramInstall.engramInstalled()) {
    if (nativeName) {
      try {
        execSync(`engram setup ${nativeName}`, { stdio: 'pipe' });
        return `engram setup ${nativeName} done`;
      } catch (e) {
        return `engram setup ${nativeName} failed: ${e.message.split('\n')[0]}`;
      }
    }
    mergeJson(mcpConfigPath, { mcpServers: { engram: ENGRAM_MCP_ENTRY } });
    return path.relative(HOME, mcpConfigPath) + ' (engram registered)';
  }
  return engramInstall.startInstall(nativeName || 'codex', ENGRAM_LOG).message;
}

const TOOLS = {
  cursor: {
    detect: () => fs.existsSync(path.join(HOME, '.cursor')) || which('cursor'),
    setup() {
      const out = [];
      const hooksDest = path.join(CWD, '.cursor', 'leanstack');
      fs.mkdirSync(hooksDest, { recursive: true });
      for (const f of ['state.js', 'components.js', 'session-start.js', 'prompt-submit.js']) {
        fs.copyFileSync(path.join(__dirname, '..', 'src', 'hooks', f), path.join(hooksDest, f));
      }
      // components.js does require('../rule-text.js') and require('../engram-install.js')
      // relative to .cursor/leanstack/, so both copies land one level up.
      fs.copyFileSync(path.join(__dirname, '..', 'src', 'rule-text.js'), path.join(CWD, '.cursor', 'rule-text.js'));
      fs.copyFileSync(path.join(__dirname, '..', 'src', 'engram-install.js'), path.join(CWD, '.cursor', 'engram-install.js'));
      out.push('.cursor/leanstack/*.js (hook scripts copied in)');

      const hooksJsonPath = path.join(CWD, '.cursor', 'hooks.json');
      const wrote = writeIfAbsent(hooksJsonPath, JSON.stringify({
        version: 1,
        hooks: {
          sessionStart: [{ command: 'node ./.cursor/leanstack/session-start.js cursor', type: 'command', timeout: 30 }],
          beforeSubmitPrompt: [{ command: 'node ./.cursor/leanstack/prompt-submit.js cursor', type: 'command', timeout: 10 }],
        },
      }, null, 2) + '\n');
      out.push(wrote ? '.cursor/hooks.json' : '.cursor/hooks.json (exists, skipped)');

      const note = leanctxNote();
      if (note) { out.push(note); }
      else {
        mergeJson(path.join(HOME, '.cursor', 'mcp.json'), { mcpServers: { 'lean-ctx': LEANCTX_MCP_ENTRY } });
        out.push('~/.cursor/mcp.json (lean-ctx registered)');
      }
      out.push(engramSetup('cursor', path.join(HOME, '.cursor', 'mcp.json')));
      return out;
    },
  },
  windsurf: {
    detect: () => fs.existsSync(path.join(HOME, '.codeium', 'windsurf')),
    setup() {
      const out = [];
      const note = leanctxNote();
      if (note) { out.push(note); }
      else {
        mergeJson(path.join(HOME, '.codeium', 'windsurf', 'mcp_config.json'), { mcpServers: { 'lean-ctx': LEANCTX_MCP_ENTRY } });
        out.push('windsurf mcp_config.json (lean-ctx registered)');
      }
      out.push(engramSetup('windsurf'));
      const wrote = writeIfAbsent(path.join(CWD, '.windsurf', 'rules', 'leanstack.md'), RULES_BLOCK);
      out.push(wrote ? '.windsurf/rules/leanstack.md' : '.windsurf/rules/leanstack.md (exists, skipped)');
      return out;
    },
  },
  vscode: {
    detect: () => which('code'),
    setup() {
      const out = [];
      const note = leanctxNote();
      if (note) { out.push(note); }
      else {
        const payload = JSON.stringify({ name: 'lean-ctx', command: 'lean-ctx', args: ['serve'] });
        try {
          execSync(`code --add-mcp ${JSON.stringify(payload)}`, { stdio: 'pipe', shell: true });
          out.push('lean-ctx registered via code --add-mcp');
        } catch (e) {
          out.push('code --add-mcp failed (' + e.message.split('\n')[0] + ') — add manually to .vscode/mcp.json: ' + payload);
        }
      }
      out.push(engramSetup('vscode-copilot'));
      const wrote = writeIfAbsent(path.join(CWD, '.github', 'copilot-instructions.md'), RULES_BLOCK);
      out.push(wrote ? '.github/copilot-instructions.md' : '.github/copilot-instructions.md (exists, skipped)');
      return out;
    },
  },
  cline: {
    detect: () => fs.existsSync(path.join(HOME, '.cline')),
    setup() {
      const out = [];
      const note = leanctxNote();
      if (note) { out.push(note); }
      else {
        mergeJson(path.join(HOME, '.cline', 'mcp.json'), { mcpServers: { 'lean-ctx': LEANCTX_MCP_ENTRY } });
        out.push('~/.cline/mcp.json (lean-ctx registered)');
      }
      // No native `engram setup cline` — register the MCP command directly,
      // same shape engram's docs use for any other MCP client.
      out.push(engramSetup(null, path.join(HOME, '.cline', 'mcp.json')));
      const wrote = writeIfAbsent(path.join(CWD, '.clinerules', 'leanstack.md'), RULES_BLOCK);
      out.push(wrote ? '.clinerules/leanstack.md' : '.clinerules/leanstack.md (exists, skipped)');
      return out;
    },
  },
  continue: {
    detect: () => fs.existsSync(path.join(CWD, '.continue')),
    setup() {
      const out = [];
      const note = leanctxNote();
      if (note) { out.push(note); }
      else {
        const wrote = writeIfAbsent(path.join(CWD, '.continue', 'mcpServers', 'leanstack.json'), JSON.stringify(LEANCTX_MCP_ENTRY, null, 2) + '\n');
        out.push(wrote ? '.continue/mcpServers/leanstack.json' : '.continue/mcpServers/leanstack.json (exists, skipped)');
      }
      if (engramInstall.engramInstalled()) {
        const wrote = writeIfAbsent(path.join(CWD, '.continue', 'mcpServers', 'engram.json'), JSON.stringify(ENGRAM_MCP_ENTRY, null, 2) + '\n');
        out.push(wrote ? '.continue/mcpServers/engram.json' : '.continue/mcpServers/engram.json (exists, skipped)');
      } else {
        out.push(engramInstall.startInstall('codex', ENGRAM_LOG).message);
      }
      return out;
    },
  },
};

function main() {
  const requested = process.argv.slice(2).filter(a => !a.startsWith('-'));
  const targets = requested.length ? requested : Object.keys(TOOLS).filter(name => TOOLS[name].detect());

  if (!targets.length) {
    console.log('leanstack setup: no supported tool detected (Cursor/Windsurf/VS Code/Cline/Continue).');
    console.log('Run with an explicit name to force it, e.g.: npx github:getappz/leanstack cursor');
    return;
  }

  for (const name of targets) {
    const tool = TOOLS[name];
    if (!tool) { console.log(`unknown tool: ${name} (known: ${Object.keys(TOOLS).join(', ')})`); continue; }
    console.log(`\n${name}:`);
    for (const line of tool.setup()) console.log('  ' + line);
  }
}

main();
