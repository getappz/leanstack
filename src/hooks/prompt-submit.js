#!/usr/bin/env node
const state = require('./state.js');
const components = require('./components.js');

function emit(text) {
  process.stdout.write(JSON.stringify({
    hookSpecificOutput: { hookEventName: 'UserPromptSubmit', additionalContext: text },
  }));
}

let input = '';
process.stdin.on('data', chunk => { input += chunk; });
process.stdin.on('end', () => {
  let prompt = '';
  try { prompt = ((JSON.parse(input).prompt) || '').trim().toLowerCase(); } catch (_) { return; }

  const s = state.load();

  if (prompt === '/leanstack off' || prompt === '/leanstack stop') {
    s.active = false;
    state.save(s);
    return;
  }
  if (prompt === '/leanstack on') {
    s.active = true;
    state.save(s);
  }

  if (prompt === '/leanstack confirm') {
    if (s.confirmed) { emit('leanstack: already confirmed, nothing pending.'); return; }
    const results = [];
    for (const c of components) {
      if (c.needsConsent && !c.check()) results.push(c.apply());
    }
    s.confirmed = true;
    state.save(s);
    emit(results.length ? 'leanstack install confirmed.\n' + results.join('\n') : 'leanstack: nothing was pending.');
    return;
  }

  if (!s.active) return;

  const bits = [
    'LEANSTACK ACTIVE.',
    'Prefer lean-ctx ctx_* tools over native Read/Grep/Bash/Glob.',
    'Exa is the only web search tool.',
    'Clean git commits, no AI signature.',
  ];
  if (!s.confirmed) {
    const pending = components.some(c => c.needsConsent && !c.check());
    if (pending) bits.push('Reminder: `/leanstack confirm` to finish install.');
  }
  emit(bits.join(' '));
});
