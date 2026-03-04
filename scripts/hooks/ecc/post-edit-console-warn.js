#!/usr/bin/env node
/**
 * PostToolUse Hook: warn if edited content introduces console.log.
 */

const MAX_STDIN = 1024 * 1024;
let data = '';
process.stdin.setEncoding('utf8');

process.stdin.on('data', chunk => {
  if (data.length < MAX_STDIN) {
    const remaining = MAX_STDIN - data.length;
    data += chunk.substring(0, remaining);
  }
});

process.stdin.on('end', () => {
  const raw = (process.env.MAESTRO_ENABLE_ECC_QUALITY_GATES || '').trim().toLowerCase();
  const disabled = ['0', 'false', 'no', 'off', 'disable', 'disabled'].includes(raw);
  if (disabled) {
    process.exit(0);
  }

  try {
    const input = JSON.parse(data);
    const filePath = input.tool_input?.file_path || '';
    const newString = input.tool_input?.new_string || '';

    if (/\.(ts|tsx|js|jsx)$/.test(filePath) && /console\.log\s*\(/.test(newString)) {
      console.error('[Hook] WARNING: console.log detected in edited content: ' + filePath);
    }
  } catch {
    // pass-through
  }

  process.exit(0);
});
