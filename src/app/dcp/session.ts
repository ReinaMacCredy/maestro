/**
 * Session-level DCP state tracking.
 * Tracks context injection metrics across tool calls within a session.
 * Persisted to .maestro/sessions/dcp-session.json.
 */

import * as path from 'node:path';
import { readJson, writeJsonAtomic, ensureDir } from '../../core/fs-io.ts';

export interface DcpSessionState {
  injectionCount: number;
  totalTokensInjected: number;
  componentHits: Record<string, number>;
  memoriesSelected: number;
  memoriesDropped: number;
  startedAt: string;
  lastInjectionAt?: string;
}

export function createSessionState(): DcpSessionState {
  return {
    injectionCount: 0,
    totalTokensInjected: 0,
    componentHits: {},
    memoriesSelected: 0,
    memoriesDropped: 0,
    startedAt: new Date().toISOString(),
  };
}

export function recordInjection(
  state: DcpSessionState,
  metrics: { totalTokens: number; memoriesIncluded: number; memoriesDropped: number; componentsIncluded?: string[] },
): DcpSessionState {
  state.injectionCount++;
  state.totalTokensInjected += metrics.totalTokens;
  state.memoriesSelected += metrics.memoriesIncluded;
  state.memoriesDropped += metrics.memoriesDropped;
  state.lastInjectionAt = new Date().toISOString();

  for (const comp of metrics.componentsIncluded ?? []) {
    state.componentHits[comp] = (state.componentHits[comp] ?? 0) + 1;
  }

  return state;
}

export function getSessionSummary(state: DcpSessionState): string {
  const lines = [
    `DCP Session: ${state.injectionCount} injections, ${state.totalTokensInjected} tokens total`,
    `  Memories: ${state.memoriesSelected} selected, ${state.memoriesDropped} dropped`,
  ];
  const topComponents = Object.entries(state.componentHits)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 5);
  if (topComponents.length > 0) {
    lines.push(`  Top components: ${topComponents.map(([n, c]) => `${n}(${c})`).join(', ')}`);
  }
  return lines.join('\n');
}

const SESSION_FILE = 'dcp-session.json';

export function loadSessionState(sessionsDir: string): DcpSessionState {
  return readJson<DcpSessionState>(path.join(sessionsDir, SESSION_FILE)) ?? createSessionState();
}

export function saveSessionState(sessionsDir: string, state: DcpSessionState): void {
  ensureDir(sessionsDir);
  writeJsonAtomic(path.join(sessionsDir, SESSION_FILE), state);
}
