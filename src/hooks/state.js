#!/usr/bin/env node
// Single JSON state blob instead of a scatter of flag files — one place to
// look, one place to write, no per-flag symlink dance needed: everything
// under ~/.claude/leanstack/ is written and read by the same user, so there's
// no cross-user TOCTOU surface to defend against here.
const fs = require('fs');
const path = require('path');
const os = require('os');

const CLAUDE_DIR = process.env.CLAUDE_CONFIG_DIR || path.join(os.homedir(), '.claude');
const STATE_DIR = path.join(CLAUDE_DIR, 'leanstack');
const STATE_PATH = path.join(STATE_DIR, 'state.json');

const DEFAULT_STATE = {
  active: true,
  rulesInstalled: false,
  confirmed: false,
};

function load() {
  try {
    return { ...DEFAULT_STATE, ...JSON.parse(fs.readFileSync(STATE_PATH, 'utf8')) };
  } catch (_) {
    return { ...DEFAULT_STATE };
  }
}

function save(state) {
  fs.mkdirSync(STATE_DIR, { recursive: true });
  fs.writeFileSync(STATE_PATH, JSON.stringify(state, null, 2) + '\n');
}

module.exports = { load, save, CLAUDE_DIR, STATE_DIR };
