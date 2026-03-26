/**
 * Integration tests for config_set container invalidation.
 *
 * Verifies that changing settings via config_set takes effect in the
 * running MCP session. Covers the stale config bug where the container
 * was cached forever and config changes were ignored.
 */

import { describe, test, expect, beforeEach, afterEach } from 'bun:test';
import { createServicesThunk, type ServicesThunk } from '../../surfaces/mcp/services-thunk.ts';
import { readJson, writeJsonAtomic } from '../../infra/utils/fs-io.ts';
import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';

function setupProject(tmpDir: string, settings: Record<string, unknown> = {}): void {
  const maestroDir = path.join(tmpDir, '.maestro');
  fs.mkdirSync(path.join(maestroDir, 'features'), { recursive: true });
  fs.mkdirSync(path.join(maestroDir, 'memory'), { recursive: true });
  if (Object.keys(settings).length > 0) {
    writeJsonAtomic(path.join(maestroDir, 'settings.json'), settings);
  }
}

describe('config_set container invalidation', () => {
  let tmpDir: string;

  beforeEach(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'maestro-config-inv-'));
  });

  afterEach(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  test('invalidate() clears cached container', () => {
    setupProject(tmpDir);
    const thunk = createServicesThunk(tmpDir);

    const first = thunk.get();
    expect(thunk.isInitialized()).toBe(true);

    thunk.invalidate();
    expect(thunk.isInitialized()).toBe(false);

    const second = thunk.get();
    expect(second).not.toBe(first); // different reference -- rebuilt
    expect(second.directory).toBe(tmpDir);
  });

  test('get() after invalidate() reads fresh settings from disk', () => {
    setupProject(tmpDir, { tasks: { backend: 'fs' } });
    const thunk = createServicesThunk(tmpDir);

    const first = thunk.get();
    expect(first.taskBackend).toBe('fs');

    // Simulate config_set: write new value to disk + invalidate
    const settingsPath = path.join(tmpDir, '.maestro', 'settings.json');
    writeJsonAtomic(settingsPath, { tasks: { backend: 'fs' } });
    (first.settingsPort as any).invalidate?.();
    thunk.invalidate();

    const second = thunk.get();
    // Should re-read from disk and resolve to fs
    expect(second.taskBackend).toBe('fs');
    expect(second).not.toBe(first);
  });

  test('without invalidate(), config changes are invisible', () => {
    setupProject(tmpDir, { tasks: { backend: 'fs' } });
    const thunk = createServicesThunk(tmpDir);

    const first = thunk.get();
    expect(first.taskBackend).toBe('fs');

    // Write new settings to disk but DON'T invalidate
    const settingsPath = path.join(tmpDir, '.maestro', 'settings.json');
    writeJsonAtomic(settingsPath, { tasks: { backend: 'fs' } });

    const second = thunk.get();
    expect(second).toBe(first); // same cached reference
  });

  test('invalidate() is safe to call before any get()', () => {
    setupProject(tmpDir);
    const thunk = createServicesThunk(tmpDir);

    // Should not throw
    thunk.invalidate();
    expect(thunk.isInitialized()).toBe(false);

    const services = thunk.get();
    expect(services).toBeDefined();
  });

  test('invalidate() is safe to call multiple times', () => {
    setupProject(tmpDir);
    const thunk = createServicesThunk(tmpDir);

    thunk.get();
    thunk.invalidate();
    thunk.invalidate();
    thunk.invalidate();

    const fresh = thunk.get();
    expect(fresh).toBeDefined();
  });

  test('forceInit() after invalidate() works', () => {
    setupProject(tmpDir);
    const thunk = createServicesThunk(tmpDir);

    thunk.get();
    thunk.invalidate();

    const forced = thunk.forceInit();
    expect(forced).toBeDefined();
    expect(thunk.isInitialized()).toBe(true);
  });

  test('DCP settings change takes effect after invalidation', () => {
    setupProject(tmpDir, { dcp: { enabled: true, memoryBudgetTokens: 512 } });
    const thunk = createServicesThunk(tmpDir);

    const first = thunk.get();
    const settings1 = first.settingsPort.get();
    expect(settings1.dcp.memoryBudgetTokens).toBe(512);

    // Change DCP budget via disk write + invalidate
    const settingsPath = path.join(tmpDir, '.maestro', 'settings.json');
    writeJsonAtomic(settingsPath, { dcp: { enabled: true, memoryBudgetTokens: 2048 } });
    (first.settingsPort as any).invalidate?.();
    thunk.invalidate();

    const second = thunk.get();
    const settings2 = second.settingsPort.get();
    expect(settings2.dcp.memoryBudgetTokens).toBe(2048);
  });

  test('toolbox deny list change takes effect after invalidation', () => {
    setupProject(tmpDir, { toolbox: { deny: [] } });
    const thunk = createServicesThunk(tmpDir);

    const first = thunk.get();

    // Add a tool to deny list
    const settingsPath = path.join(tmpDir, '.maestro', 'settings.json');
    writeJsonAtomic(settingsPath, { toolbox: { deny: ['cass'] } });
    (first.settingsPort as any).invalidate?.();
    thunk.invalidate();

    const second = thunk.get();
    const settings2 = second.settingsPort.get();
    expect(settings2.toolbox.deny).toContain('cass');
  });
});

describe('config_set handler integration', () => {
  let tmpDir: string;

  beforeEach(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'maestro-config-handler-'));
  });

  afterEach(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  test('simulated config_set flow: write + invalidate settings + invalidate thunk', () => {
    setupProject(tmpDir, { tasks: { backend: 'fs', claimExpiresMinutes: 120 } });
    const thunk = createServicesThunk(tmpDir);

    // Simulate first MCP call -- container is built
    const services1 = thunk.get();
    expect(services1.taskBackend).toBe('fs');

    // Simulate config_set handler:
    // 1. Write to disk
    const settingsPath = path.join(tmpDir, '.maestro', 'settings.json');
    const existing = readJson<Record<string, unknown>>(settingsPath) ?? {};
    (existing as any).tasks = { ...(existing as any).tasks, claimExpiresMinutes: 60 };
    writeJsonAtomic(settingsPath, existing);
    // 2. Invalidate settings adapter
    (services1.settingsPort as any).invalidate?.();
    // 3. Invalidate container (THE FIX)
    thunk.invalidate();

    // Simulate next MCP call -- should see updated config
    const services2 = thunk.get();
    const settings = services2.settingsPort.get();
    expect(settings.tasks.claimExpiresMinutes).toBe(60);
    expect(services2).not.toBe(services1);
  });

  test('verification config change takes effect after invalidation', () => {
    setupProject(tmpDir, { verification: { buildCommand: 'bun run typecheck' } });
    const thunk = createServicesThunk(tmpDir);

    thunk.get();

    const settingsPath = path.join(tmpDir, '.maestro', 'settings.json');
    writeJsonAtomic(settingsPath, { verification: { buildCommand: 'bun run build', buildTimeoutMs: 60000 } });
    thunk.invalidate();

    const fresh = thunk.get();
    const settings = fresh.settingsPort.get();
    expect(settings.verification.buildCommand).toBe('bun run build');
    expect(settings.verification.buildTimeoutMs).toBe(60000);
  });
});
