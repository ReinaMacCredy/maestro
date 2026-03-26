/**
 * Integration tests for toolbox-driven service wiring.
 * Verifies initServices() uses toolbox for port resolution.
 */

import { describe, it, expect, beforeEach, afterEach } from 'bun:test';
import * as fs from 'fs';
import * as path from 'path';
import { mkdtemp, rm } from 'fs/promises';
import { tmpdir } from 'os';

import { ToolboxRegistry } from '../../infra/toolbox/registry.ts';
import { clearDetectCache } from '../../infra/toolbox/loader.ts';
import { DEFAULT_SETTINGS, mergeSettings } from '../../domain/ports/settings.ts';
import type { MaestroSettings } from '../../domain/ports/settings.ts';
import type { ToolManifest } from '../../infra/toolbox/types.ts';
import { initServices } from '../../services.ts';
import { FsTaskAdapter } from '../../infra/adapters/tasks/adapter.ts';

// ============================================================================
// Helpers
// ============================================================================

function makeManifest(overrides: Partial<ToolManifest> & { name: string }): ToolManifest {
  return {
    binary: null,
    detect: null,
    provides: null,
    priority: 0,
    adapter: 'test.ts',
    ...overrides,
  };
}

function settingsWith(overrides: Partial<MaestroSettings>): MaestroSettings {
  return mergeSettings(DEFAULT_SETTINGS, overrides);
}

function writeProjectSettings(tmpDir: string, settings: MaestroSettings): void {
  fs.writeFileSync(
    path.join(tmpDir, '.maestro', 'settings.json'),
    JSON.stringify(settings, null, 2),
  );
}

// ============================================================================
// Tests
// ============================================================================

describe('toolbox integration with initServices', () => {
  let tmpDir: string;

  beforeEach(async () => {
    clearDetectCache();
    tmpDir = await mkdtemp(path.join(tmpdir(), 'toolbox-int-'));
    // Create .maestro dir for initServices
    fs.mkdirSync(path.join(tmpDir, '.maestro'), { recursive: true });
    // Create minimal git repo
    const { execSync } = require('child_process');
    execSync('git init', { cwd: tmpDir, stdio: 'pipe' });
  });

  afterEach(async () => {
    await rm(tmpDir, { recursive: true, force: true });
  });

  it('uses FsTaskAdapter when settings.tasks.backend is fs', () => {
    const settings = settingsWith({ tasks: { backend: 'fs', claimExpiresMinutes: 60 } });
    writeProjectSettings(tmpDir, settings);
    // Build toolbox with br "available" but settings say fs
    const manifests = [
      makeManifest({ name: 'fs-tasks', provides: 'tasks', priority: 0 }),
      makeManifest({ name: 'br', provides: 'tasks', priority: 100, detect: 'echo ok' }),
    ];
    const toolbox = new ToolboxRegistry(manifests, settings);
    const services = initServices(tmpDir, toolbox);

    expect(services.taskPort).toBeInstanceOf(FsTaskAdapter);
    expect(services.taskBackend).toBe('fs');
  });

  it('uses fs-tasks fallback when br not installed and backend is auto', () => {
    const settings = settingsWith({ tasks: { backend: 'auto', claimExpiresMinutes: 120 } });
    writeProjectSettings(tmpDir, settings);
    const manifests = [
      makeManifest({ name: 'fs-tasks', provides: 'tasks', priority: 0 }),
      makeManifest({
        name: 'br', provides: 'tasks', priority: 100,
        binary: 'nonexistent-xyz-999', detect: 'nonexistent-xyz-999 --version',
      }),
    ];
    const toolbox = new ToolboxRegistry(manifests, settings);
    const services = initServices(tmpDir, toolbox);

    expect(services.taskPort).toBeInstanceOf(FsTaskAdapter);
    expect(services.taskBackend).toBe('fs');
  });

  it('graph port is undefined when bv is denied', () => {
    const settings = settingsWith({
      toolbox: { allow: [], deny: ['bv'], config: {} },
    });
    const manifests = [
      makeManifest({ name: 'fs-tasks', provides: 'tasks', priority: 0 }),
      makeManifest({ name: 'bv', provides: 'graph', priority: 100, detect: 'echo ok' }),
    ];
    const toolbox = new ToolboxRegistry(manifests, settings);
    const services = initServices(tmpDir, toolbox);

    expect(services.graphPort).toBeUndefined();
  });

  it('exposes toolbox and settingsPort on services', () => {
    const manifests = [
      makeManifest({ name: 'fs-tasks', provides: 'tasks', priority: 0 }),
    ];
    const toolbox = new ToolboxRegistry(manifests, DEFAULT_SETTINGS);
    const services = initServices(tmpDir, toolbox);

    expect(services.toolbox).toBe(toolbox);
    expect(services.settingsPort).toBeDefined();
    expect(services.settingsPort.get()).toBeDefined();
  });

  it('built-in ports always exist regardless of toolbox', () => {
    const manifests = [
      makeManifest({ name: 'fs-tasks', provides: 'tasks', priority: 0 }),
    ];
    const toolbox = new ToolboxRegistry(manifests, DEFAULT_SETTINGS);
    const services = initServices(tmpDir, toolbox);

    expect(services.featureAdapter).toBeDefined();
    expect(services.planAdapter).toBeDefined();
    expect(services.memoryAdapter).toBeDefined();
    expect(services.settingsPort).toBeDefined();
    expect(services.verificationPort).toBeDefined();
    expect(services.doctrinePort).toBeDefined();
  });
});
