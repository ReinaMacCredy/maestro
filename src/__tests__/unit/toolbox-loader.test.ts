import { describe, it, expect, beforeEach, afterEach } from 'bun:test';
import * as fs from 'fs';
import * as path from 'path';
import { mkdtemp, rm } from 'fs/promises';
import { tmpdir } from 'os';

import {
  loadManifest,
  scanToolboxDir,
  scanBuiltInManifests,
  detectTool,
  clearDetectCache,
} from '../../infra/toolbox/loader.ts';
import type { ToolManifest } from '../../infra/toolbox/types.ts';

// ============================================================================
// loadManifest
// ============================================================================

describe('loadManifest', () => {
  let tmpDir: string;

  beforeEach(async () => {
    tmpDir = await mkdtemp(path.join(tmpdir(), 'toolbox-'));
  });

  afterEach(async () => {
    await rm(tmpDir, { recursive: true, force: true });
  });

  it('loads a valid manifest', () => {
    const filePath = path.join(tmpDir, 'manifest.json');
    fs.writeFileSync(filePath, JSON.stringify({
      name: 'test-tool',
      binary: 'test',
      detect: 'test --version',
      provides: 'tasks',
      priority: 50,
      adapter: 'tools/test/adapter.ts',
    }));
    const result = loadManifest(filePath);
    expect(result).not.toBeNull();
    expect(result!.name).toBe('test-tool');
    expect(result!.priority).toBe(50);
  });

  it('returns null for missing file', () => {
    expect(loadManifest('/nonexistent/manifest.json')).toBeNull();
  });

  it('returns null for invalid JSON', () => {
    const filePath = path.join(tmpDir, 'bad.json');
    fs.writeFileSync(filePath, 'not json');
    expect(loadManifest(filePath)).toBeNull();
  });

  it('returns null when name is missing', () => {
    const filePath = path.join(tmpDir, 'manifest.json');
    fs.writeFileSync(filePath, JSON.stringify({ priority: 0 }));
    expect(loadManifest(filePath)).toBeNull();
  });

  it('returns null when priority is missing', () => {
    const filePath = path.join(tmpDir, 'manifest.json');
    fs.writeFileSync(filePath, JSON.stringify({ name: 'foo' }));
    expect(loadManifest(filePath)).toBeNull();
  });
});

// ============================================================================
// scanToolboxDir
// ============================================================================

describe('scanToolboxDir', () => {
  let tmpDir: string;

  beforeEach(async () => {
    tmpDir = await mkdtemp(path.join(tmpdir(), 'toolbox-scan-'));
  });

  afterEach(async () => {
    await rm(tmpDir, { recursive: true, force: true });
  });

  it('scans built-in and external subdirectories', () => {
    const builtInDir = path.join(tmpDir, 'tools', 'built-in', 'alpha');
    const externalDir = path.join(tmpDir, 'tools', 'external', 'beta');
    fs.mkdirSync(builtInDir, { recursive: true });
    fs.mkdirSync(externalDir, { recursive: true });
    fs.writeFileSync(
      path.join(builtInDir, 'manifest.json'),
      JSON.stringify({ name: 'alpha', priority: 0, provides: 'tasks', binary: null, detect: null, adapter: 'a.ts' }),
    );
    fs.writeFileSync(
      path.join(externalDir, 'manifest.json'),
      JSON.stringify({ name: 'beta', priority: 100, provides: 'graph', binary: 'beta', detect: 'beta -v', adapter: 'b.ts' }),
    );
    const results = scanToolboxDir(tmpDir);
    expect(results).toHaveLength(2);
    expect(results.map(m => m.name).sort()).toEqual(['alpha', 'beta']);
  });

  it('skips directories without manifest.json', () => {
    const dir = path.join(tmpDir, 'tools', 'external', 'empty');
    fs.mkdirSync(dir, { recursive: true });
    const results = scanToolboxDir(tmpDir);
    expect(results).toHaveLength(0);
  });

  it('returns empty when tools dir does not exist', () => {
    const results = scanToolboxDir(path.join(tmpDir, 'nonexistent'));
    expect(results).toHaveLength(0);
  });
});

// ============================================================================
// scanBuiltInManifests
// ============================================================================

describe('scanBuiltInManifests', () => {
  it('finds the 10 bundled manifests', () => {
    const manifests = scanBuiltInManifests();
    expect(manifests.length).toBe(10);
    const names = manifests.map(m => m.name).sort();
    expect(names).toEqual(['agent-mail', 'agent-memory', 'br', 'bv', 'cass', 'fs-handoff', 'fs-search', 'fs-tasks', 'mcp-graph', 'mcp-search']);
  });

  it('fs-tasks is built-in (no binary, no detect)', () => {
    const manifests = scanBuiltInManifests();
    const fsTasks = manifests.find(m => m.name === 'fs-tasks');
    expect(fsTasks).toBeDefined();
    expect(fsTasks!.binary).toBeNull();
    expect(fsTasks!.detect).toBeNull();
    expect(fsTasks!.provides).toBe('tasks');
    expect(fsTasks!.priority).toBe(0);
  });

  it('br has higher priority than fs-tasks for tasks port', () => {
    const manifests = scanBuiltInManifests();
    const br = manifests.find(m => m.name === 'br')!;
    const fsTasks = manifests.find(m => m.name === 'fs-tasks')!;
    expect(br.provides).toBe('tasks');
    expect(br.priority).toBeGreaterThan(fsTasks.priority);
  });

  it('agent-mail has inject dependencies', () => {
    const manifests = scanBuiltInManifests();
    const am = manifests.find(m => m.name === 'agent-mail')!;
    expect(am.inject).toContain('taskPort');
    expect(am.inject).toContain('memoryPort');
  });
});

// ============================================================================
// detectTool
// ============================================================================

describe('detectTool', () => {
  beforeEach(() => {
    clearDetectCache();
  });

  const noFilter = { allow: [], deny: [] };

  it('built-in tools (null detect + null binary) are always installed', () => {
    const manifest: ToolManifest = {
      name: 'fs-tasks', binary: null, detect: null,
      provides: 'tasks', priority: 0, adapter: 'a.ts',
    };
    const status = detectTool(manifest, noFilter);
    expect(status.installed).toBe(true);
    expect(status.settingsState).toBe('default');
  });

  it('detects a real binary (echo)', () => {
    const manifest: ToolManifest = {
      name: 'echo-test', binary: 'echo', detect: 'echo hello',
      provides: null, priority: 0, adapter: 'a.ts',
    };
    const status = detectTool(manifest, noFilter);
    expect(status.installed).toBe(true);
    expect(status.version).toBe('hello');
  });

  it('marks missing binary as not installed', () => {
    const manifest: ToolManifest = {
      name: 'nonexistent', binary: 'nonexistent-tool-xyz-999',
      detect: 'nonexistent-tool-xyz-999 --version',
      provides: null, priority: 0, adapter: 'a.ts',
    };
    const status = detectTool(manifest, noFilter);
    expect(status.installed).toBe(false);
    expect(status.detectError).toBeDefined();
  });

  it('reports denied settingsState', () => {
    const manifest: ToolManifest = {
      name: 'bv', binary: 'bv', detect: 'echo ok',
      provides: 'graph', priority: 100, adapter: 'a.ts',
    };
    const status = detectTool(manifest, { allow: [], deny: ['bv'] });
    expect(status.settingsState).toBe('denied');
  });

  it('reports allowed settingsState in allowlist mode', () => {
    const manifest: ToolManifest = {
      name: 'br', binary: 'br', detect: 'echo ok',
      provides: 'tasks', priority: 100, adapter: 'a.ts',
    };
    const allowed = detectTool(manifest, { allow: ['br'], deny: [] });
    expect(allowed.settingsState).toBe('allowed');

    clearDetectCache();
    const notAllowed = detectTool(
      { ...manifest, name: 'bv', detect: 'echo ok2' },
      { allow: ['br'], deny: [] },
    );
    expect(notAllowed.settingsState).toBe('denied');
  });

  it('caches detection results', () => {
    const manifest: ToolManifest = {
      name: 'cache-test', binary: null, detect: 'echo cached',
      provides: null, priority: 0, adapter: 'a.ts',
    };
    const first = detectTool(manifest, noFilter);
    const second = detectTool(manifest, noFilter);
    expect(first.installed).toBe(second.installed);
    expect(first.version).toBe(second.version);
  });
});
