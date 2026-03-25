import * as fs from 'node:fs';
import * as path from 'node:path';
import { readStdin, resolveProjectDir, logHookError, getSessionsDir, EVENTS_FILE } from './_helpers.ts';
import { ensureDir } from '../../core/fs-io.ts';

const HOOK_NAME = 'posttooluse';

async function main(): Promise<void> {
  const input = await readStdin();
  const projectDir = resolveProjectDir();
  if (!projectDir) return;

  const toolName = (input.tool_name as string) || 'unknown';
  const toolInput = (input.tool_input as Record<string, unknown>) || {};

  const sessionsDir = getSessionsDir(projectDir);
  ensureDir(sessionsDir);

  const eventsPath = path.join(sessionsDir, EVENTS_FILE);
  const entry: Record<string, unknown> = { ts: new Date().toISOString(), tool: toolName };
  if (toolInput.feature) entry.feature = toolInput.feature;
  if (toolInput.task) entry.task = toolInput.task;
  if (toolInput.status) entry.status = toolInput.status;
  fs.appendFileSync(eventsPath, JSON.stringify(entry) + '\n');

  // Pure side effect -- no stdout output
}

try {
  await main();
} catch (error) {
  logHookError(resolveProjectDir(), HOOK_NAME, error);
  // No output on error
}
