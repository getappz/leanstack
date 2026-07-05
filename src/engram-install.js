#!/usr/bin/env node
// engram (github.com/Gentleman-Programming/engram) has no safe universal
// one-liner like lean-ctx's install.sh/npm package: the maintainer's own docs
// say prebuilt Windows binaries get AV-flagged as false positives and
// explicitly recommend `go install` (compiles locally, never flagged) or
// Homebrew on macOS/Linux instead. Auto-install only through one of those
// two safe paths; otherwise print the documented manual options rather than
// silently pulling the AV-flagged binary.
const { execSync, spawn } = require('child_process');
const fs = require('fs');

function has(cmd) {
  try {
    execSync(process.platform === 'win32' ? `where ${cmd}` : `which ${cmd}`, { stdio: 'pipe' });
    return true;
  } catch (_) { return false; }
}

function engramInstalled() {
  try { execSync('engram version', { stdio: 'pipe' }); return true; } catch (_) { return false; }
}

// Kicks off a detached background install + host wiring, same pattern as
// lean-ctx's npm install. Returns a status string; never blocks.
function startInstall(host, logPath) {
  let cmd;
  if (has('go')) {
    cmd = `go install github.com/Gentleman-Programming/engram/cmd/engram@latest && engram setup ${host}`;
  } else if (process.platform !== 'win32' && has('brew')) {
    cmd = `brew install gentleman-programming/tap/engram && engram setup ${host}`;
  } else {
    return {
      started: false,
      message: process.platform === 'win32'
        ? 'engram: no safe auto-install path (no Go toolchain, and prebuilt Windows binaries are AV-flagged per the project\'s own docs). Install Go then re-run, or see github.com/Gentleman-Programming/engram/releases and accept the AV warning yourself.'
        : 'engram: no Go or Homebrew found. Install one, or see github.com/Gentleman-Programming/engram/blob/main/docs/INSTALLATION.md',
    };
  }

  fs.mkdirSync(require('path').dirname(logPath), { recursive: true });
  const fd = fs.openSync(logPath, 'a');
  const child = process.platform === 'win32'
    ? spawn('cmd', ['/c', cmd], { detached: true, stdio: ['ignore', fd, fd] })
    : spawn('sh', ['-c', cmd], { detached: true, stdio: ['ignore', fd, fd] });
  child.unref();
  return { started: true, message: `engram install started in background via ${has('go') ? 'go install' : 'brew'} (log: ${logPath})` };
}

module.exports = { engramInstalled, startInstall };
