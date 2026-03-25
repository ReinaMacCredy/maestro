import * as fs from 'node:fs';
import * as path from 'node:path';
import { readStdin, writeOutput, resolveProjectDir, logHookError, getSessionsDir, EVENTS_FILE } from './_helpers.ts';
import { writeJsonAtomic, ensureDir, writeText } from '../../core/fs-io.ts';
import { initServices } from '../../services.ts';
import { checkStatus } from '../../workflow/status.ts';
import { getHandoffsPath } from '../../core/paths.ts';

const HOOK_NAME = 'precompact';

async function main(): Promise<void> {
  await readStdin();

  const projectDir = resolveProjectDir();
  if (!projectDir) {
    writeOutput({});
    return;
  }

  const services = initServices(projectDir);
  const activeFeature = services.featureAdapter.getActive();

  const sessionsDir = getSessionsDir(projectDir);
  ensureDir(sessionsDir);

  const snapshotPath = path.join(sessionsDir, 'compact-snapshot.json');

  if (!activeFeature) {
    // Minimal snapshot when no feature is active
    const snapshot = {
      timestamp: new Date().toISOString(),
      feature: null,
      tasks: { total: 0, pending: 0, inProgress: 0, done: 0 },
      runnable: [],
      nextAction: 'No active feature. Create one with maestro feature-create.',
      recentEvents: [],
      memoryFiles: [],
    };
    writeJsonAtomic(snapshotPath, snapshot);
    writeOutput({});
    return;
  }

  const featureName = activeFeature.name;
  const status = await checkStatus(services, featureName);

  // Read recent events (last 50 lines)
  const eventsPath = path.join(sessionsDir, EVENTS_FILE);
  let recentEvents: unknown[] = [];
  let fd: number | undefined;
  try {
    // Open directly -- no existence check needed (ENOENT caught below)
    fd = fs.openSync(eventsPath, 'r');
    const stat = fs.fstatSync(fd);
    const maxTailBytes = 65536;
    const buf = Buffer.allocUnsafe(Math.min(stat.size, maxTailBytes));
    fs.readSync(fd, buf, 0, buf.length, Math.max(0, stat.size - maxTailBytes));
    const lines = buf.toString('utf-8').split('\n').filter(Boolean);
    const lastLines = lines.slice(-50);
    recentEvents = lastLines.map((line) => {
      try {
        return JSON.parse(line);
      } catch {
        return { raw: line };
      }
    });
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code !== 'ENOENT') throw e;
  } finally {
    if (fd !== undefined) fs.closeSync(fd);
  }

  // Read memory file names
  const memoryEntries = services.memoryAdapter.list(featureName);
  const memoryFiles = memoryEntries.map((entry) => entry.name);

  const snapshot = {
    timestamp: new Date().toISOString(),
    feature: {
      name: status.feature.name,
      status: status.feature.status,
    },
    tasks: {
      total: status.tasks.total,
      pending: status.tasks.pending,
      inProgress: status.tasks.inProgress,
      done: status.tasks.done,
    },
    runnable: status.runnable,
    nextAction: status.nextAction,
    recentEvents,
    memoryFiles,
  };

  writeJsonAtomic(snapshotPath, snapshot);

  // -- Auto-generate lightweight handoff for session continuity --
  generateSessionHandoff(projectDir, featureName, status, snapshot.timestamp);

  writeOutput({});
}

/**
 * Generate a lightweight handoff document when a session compacts.
 * Goal is inferred from the most recent in-progress or done task.
 */
function generateSessionHandoff(
  projectDir: string,
  featureName: string,
  status: { tasks: { items: Array<{ id: string; folder: string; name: string; status: string }> }; runnable: string[] },
  timestamp: string,
): void {
  try {
    // Infer goal from recent task activity
    const recentTask = status.tasks.items.find(t => t.status === 'claimed')
      ?? status.tasks.items.find(t => t.status === 'done');
    if (!recentTask) return; // No task activity -- skip handoff

    const goal = `Continue work on: ${recentTask.name}`;
    const sessionId = `session-${timestamp.replace(/[:.]/g, '-').slice(0, 19)}`;

    const sections: string[] = [
      `## Session Handoff: ${timestamp.slice(0, 19).replace('T', ' ')}`,
      '',
      `**Goal:** ${goal}`,
      `**Feature:** ${featureName}`,
      `**Last task:** ${recentTask.id} (${recentTask.status})`,
      '',
    ];

    if (status.runnable.length > 0) {
      sections.push('### Runnable Tasks');
      for (const r of status.runnable) sections.push(`- ${r}`);
      sections.push('');
    }

    sections.push('### Handoff Context');
    sections.push(`1. Call \`maestro_status\` to get current state.`);
    sections.push(`2. Review task \`${recentTask.id}\` for continuation.`);
    sections.push('');

    const handoffsDir = getHandoffsPath(projectDir, featureName);
    ensureDir(handoffsDir);
    writeText(path.join(handoffsDir, `${sessionId}.md`), sections.join('\n'));
  } catch {
    // Best-effort -- don't fail the hook
  }
}

try {
  await main();
} catch (error) {
  logHookError(resolveProjectDir(), HOOK_NAME, error);
  writeOutput({});
}
