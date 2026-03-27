import { describe, it, expect, beforeEach } from 'bun:test';

import { ToolboxRegistry, buildToolbox } from '../../infra/toolbox/registry.ts';
import { clearDetectCache, ADAPTER_FACTORIES, getAdapterFactory } from '../../infra/toolbox/loader.ts';
import { DEFAULT_SETTINGS } from '../../domain/ports/settings.ts';
import type { MaestroSettings } from '../../domain/ports/settings.ts';
import type { ToolManifest } from '../../infra/toolbox/types.ts';

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
  return { ...DEFAULT_SETTINGS, ...overrides };
}

// ============================================================================
// ToolboxRegistry
// ============================================================================

describe('ToolboxRegistry', () => {
  beforeEach(() => {
    clearDetectCache();
  });

  it('resolves highest-priority provider', () => {
    const manifests = [
      makeManifest({ name: 'fs-tasks', provides: 'tasks', priority: 0 }),
      makeManifest({ name: 'br', provides: 'tasks', priority: 100, detect: 'echo ok' }),
    ];
    const registry = new ToolboxRegistry(manifests, DEFAULT_SETTINGS);
    const provider = registry.resolveProvider('tasks');
    expect(provider?.name).toBe('br');
  });

  it('falls back to lower-priority when higher is not installed', () => {
    const manifests = [
      makeManifest({ name: 'fs-tasks', provides: 'tasks', priority: 0 }),
      makeManifest({
        name: 'br', provides: 'tasks', priority: 100,
        binary: 'nonexistent-xyz-999', detect: 'nonexistent-xyz-999 --version',
      }),
    ];
    const registry = new ToolboxRegistry(manifests, DEFAULT_SETTINGS);
    const provider = registry.resolveProvider('tasks');
    expect(provider?.name).toBe('fs-tasks');
  });

  it('returns null when no provider exists for port', () => {
    const manifests = [
      makeManifest({ name: 'fs-tasks', provides: 'tasks', priority: 0 }),
    ];
    const registry = new ToolboxRegistry(manifests, DEFAULT_SETTINGS);
    expect(registry.resolveProvider('graph')).toBeNull();
  });

  it('excludes denied tools from resolution', () => {
    const manifests = [
      makeManifest({ name: 'fs-tasks', provides: 'tasks', priority: 0 }),
      makeManifest({ name: 'br', provides: 'tasks', priority: 100, detect: 'echo ok' }),
    ];
    const settings = settingsWith({
      toolbox: { allow: [], deny: ['br'], config: {} },
    });
    const registry = new ToolboxRegistry(manifests, settings);
    const provider = registry.resolveProvider('tasks');
    expect(provider?.name).toBe('fs-tasks');
  });

  it('allowlist mode: only allowed tools resolve', () => {
    const manifests = [
      makeManifest({ name: 'fs-tasks', provides: 'tasks', priority: 0 }),
      makeManifest({ name: 'br', provides: 'tasks', priority: 100, detect: 'echo ok' }),
      makeManifest({ name: 'bv', provides: 'graph', priority: 100, detect: 'echo ok' }),
    ];
    // Only allow br -- bv should be denied
    const settings = settingsWith({
      toolbox: { allow: ['br', 'fs-tasks'], deny: [], config: {} },
    });
    const registry = new ToolboxRegistry(manifests, settings);
    expect(registry.resolveProvider('tasks')?.name).toBe('br');
    expect(registry.resolveProvider('graph')).toBeNull();
  });

  it('isAvailable checks install + not denied', () => {
    const manifests = [
      makeManifest({ name: 'fs-tasks', provides: 'tasks', priority: 0 }),
      makeManifest({ name: 'br', provides: 'tasks', priority: 100, detect: 'echo ok' }),
      makeManifest({
        name: 'bv', provides: 'graph', priority: 100,
        binary: 'nonexistent-xyz-999', detect: 'nonexistent-xyz-999 --version',
      }),
    ];
    const registry = new ToolboxRegistry(manifests, DEFAULT_SETTINGS);
    expect(registry.isAvailable('fs-tasks')).toBe(true);
    expect(registry.isAvailable('br')).toBe(true);
    expect(registry.isAvailable('bv')).toBe(false); // not installed
    expect(registry.isAvailable('unknown')).toBe(false);
  });

  it('isAvailable returns false for denied tool', () => {
    const manifests = [
      makeManifest({ name: 'br', provides: 'tasks', priority: 100, detect: 'echo ok' }),
    ];
    const settings = settingsWith({
      toolbox: { allow: [], deny: ['br'], config: {} },
    });
    const registry = new ToolboxRegistry(manifests, settings);
    expect(registry.isAvailable('br')).toBe(false);
  });

  it('getManifest returns manifest or null', () => {
    const manifests = [
      makeManifest({ name: 'br', provides: 'tasks', priority: 100, detect: 'echo ok' }),
    ];
    const registry = new ToolboxRegistry(manifests, DEFAULT_SETTINGS);
    expect(registry.getManifest('br')?.name).toBe('br');
    expect(registry.getManifest('unknown')).toBeNull();
  });

  it('getStatus returns all tool statuses', () => {
    const manifests = [
      makeManifest({ name: 'fs-tasks', provides: 'tasks', priority: 0 }),
      makeManifest({ name: 'br', provides: 'tasks', priority: 100, detect: 'echo ok' }),
    ];
    const registry = new ToolboxRegistry(manifests, DEFAULT_SETTINGS);
    const statuses = registry.getStatus();
    expect(statuses).toHaveLength(2);
    expect(statuses.every(s => s.manifest && typeof s.installed === 'boolean')).toBe(true);
  });

  it('getTransport returns correct transport for tools', () => {
    const manifests = [
      makeManifest({ name: 'fs-tasks', provides: 'tasks', priority: 0 }),
      makeManifest({ name: 'br', provides: 'tasks', priority: 100, binary: 'br', detect: 'echo ok' }),
      makeManifest({ name: 'am', provides: 'handoff', priority: 100, transport: 'http' as const }),
    ];
    const registry = new ToolboxRegistry(manifests, DEFAULT_SETTINGS);
    expect(registry.getTransport('fs-tasks')).toBe('builtin');
    expect(registry.getTransport('br')).toBe('cli');
    expect(registry.getTransport('am')).toBe('http');
    expect(registry.getTransport('unknown')).toBeNull();
  });

  it('getStatus includes transport field', () => {
    const manifests = [
      makeManifest({ name: 'br', provides: 'tasks', priority: 100, binary: 'br', detect: 'echo ok', transport: 'cli' as const }),
    ];
    const registry = new ToolboxRegistry(manifests, DEFAULT_SETTINGS);
    const statuses = registry.getStatus();
    expect(statuses[0].transport).toBe('cli');
  });
});

// ============================================================================
// buildToolbox
// ============================================================================

describe('buildToolbox', () => {
  beforeEach(() => {
    clearDetectCache();
  });

  it('builds from bundled manifests', () => {
    const toolbox = buildToolbox(DEFAULT_SETTINGS);
    const statuses = toolbox.getStatus();
    expect(statuses.length).toBe(10);
    // fs-tasks is always available (built-in)
    expect(toolbox.isAvailable('fs-tasks')).toBe(true);
    // tasks port always resolves (at least fs-tasks)
    expect(toolbox.resolveProvider('tasks')).not.toBeNull();
    // handoff port always resolves (fs-handoff built-in)
    expect(toolbox.isAvailable('fs-handoff')).toBe(true);
    expect(toolbox.resolveProvider('handoff')).not.toBeNull();
  });
});

// ============================================================================
// ADAPTER_FACTORIES + getAdapterFactory
// ============================================================================

describe('ADAPTER_FACTORIES', () => {
  it('has entries for all known tools', () => {
    const names = Object.keys(ADAPTER_FACTORIES).sort();
    expect(names).toEqual(['agent-mail', 'agent-memory', 'br', 'bv', 'cass', 'fs-handoff', 'fs-search', 'fs-tasks', 'mcp-graph', 'mcp-search']);
  });

  it('getAdapterFactory returns a function for known tools', () => {
    const factory = getAdapterFactory('fs-tasks');
    expect(typeof factory).toBe('function');
  });

  it('getAdapterFactory returns null for unknown tools', () => {
    const factory = getAdapterFactory('nonexistent');
    expect(factory).toBeNull();
  });
});
