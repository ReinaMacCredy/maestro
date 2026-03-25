/**
 * Manifest loader -- reads manifest.json files and detects tool availability.
 */

import * as fs from 'fs';
import * as path from 'path';
import { execFileSync } from 'node:child_process';
import { isToolAllowed } from '../../domain/ports/settings.ts';
import type { ToolManifest, ToolStatus } from './types.ts';
import type { TransportType } from './sdk/types.ts';

// ============================================================================
// Manifest Loading
// ============================================================================

/**
 * Read and validate a single manifest.json file.
 * Returns null if the file doesn't exist or is malformed.
 */
export function loadManifest(filePath: string): ToolManifest | null {
  try {
    const raw = fs.readFileSync(filePath, 'utf-8');
    const parsed = JSON.parse(raw);
    if (!parsed.name || typeof parsed.priority !== 'number') return null;
    return parsed as ToolManifest;
  } catch {
    return null;
  }
}

/**
 * Scan a toolbox directory for manifest.json files.
 * Expected structure: tools/{built-in,external}/<tool-name>/manifest.json
 */
export function scanToolboxDir(toolboxRoot: string): ToolManifest[] {
  const manifests: ToolManifest[] = [];
  const toolsDir = path.join(toolboxRoot, 'tools');

  for (const category of ['built-in', 'external']) {
    const categoryDir = path.join(toolsDir, category);
    if (!fs.existsSync(categoryDir)) continue;

    const entries = fs.readdirSync(categoryDir, { withFileTypes: true });
    for (const entry of entries) {
      if (!entry.isDirectory()) continue;
      const manifestPath = path.join(categoryDir, entry.name, 'manifest.json');
      const manifest = loadManifest(manifestPath);
      if (manifest) manifests.push(manifest);
    }
  }

  return manifests;
}

/**
 * Scan built-in manifests shipped with the package (src/toolbox/tools/).
 */
export function scanBuiltInManifests(): ToolManifest[] {
  const toolboxRoot = path.join(import.meta.dir);
  return scanToolboxDir(toolboxRoot);
}

// ============================================================================
// Detection
// ============================================================================

const detectCache = new Map<string, { installed: boolean; version?: string; error?: string }>();

/**
 * Check if a tool is installed by running its detect command.
 */
export function detectTool(
  manifest: ToolManifest,
  allowDeny: { allow: string[]; deny: string[] },
): ToolStatus {
  const settingsState: ToolStatus['settingsState'] =
    allowDeny.deny.includes(manifest.name)
      ? 'denied'
      : allowDeny.allow.length > 0
        ? allowDeny.allow.includes(manifest.name) ? 'allowed' : 'denied'
        : 'default';

  // No detection needed -- always available
  if (manifest.detect === null && manifest.binary === null) {
    return { manifest, installed: true, settingsState, transport: inferTransport(manifest) };
  }

  const cacheKey = manifest.detect ?? manifest.binary ?? manifest.name;
  const cached = detectCache.get(cacheKey);
  if (cached) {
    return { manifest, installed: cached.installed, version: cached.version, settingsState, detectError: cached.error, transport: inferTransport(manifest) };
  }

  const detectCmd = manifest.detect ?? `command -v ${manifest.binary}`;
  try {
    const output = execFileSync('sh', ['-c', detectCmd], {
      stdio: ['pipe', 'pipe', 'pipe'],
      timeout: 5000,
    });
    const version = output.toString().trim().split('\n')[0] || undefined;
    detectCache.set(cacheKey, { installed: true, version });
    return { manifest, installed: true, version, settingsState, transport: inferTransport(manifest) };
  } catch (e) {
    const error = e instanceof Error ? e.message : 'detection failed';
    detectCache.set(cacheKey, { installed: false, error });
    return { manifest, installed: false, settingsState, detectError: error, transport: inferTransport(manifest) };
  }
}

/**
 * Clear the detection cache (for testing).
 */
export function clearDetectCache(): void {
  detectCache.clear();
}

// ============================================================================
// Transport Inference
// ============================================================================

/**
 * Infer transport type from manifest fields.
 * Explicit `transport` field takes precedence; otherwise inferred from binary/detect.
 */
export function inferTransport(manifest: ToolManifest): TransportType {
  if (manifest.transport) return manifest.transport;
  if (manifest.binary !== null) return 'cli';
  if (manifest.command) return 'mcp-stdio';
  if (manifest.url) return 'mcp-http';
  if (manifest.baseUrl) return 'http';
  return 'builtin';
}

// ============================================================================
// Adapter Registry
// ============================================================================

import type { AdapterFactory, AdapterContext } from './types.ts';
import { createAdapter as fsTasksFactory } from './tools/built-in/fs-tasks/adapter.ts';
import { createAdapter as brFactory } from './tools/external/br/adapter.ts';
import { createAdapter as bvFactory } from './tools/external/bv/adapter.ts';
import { createAdapter as cassFactory } from './tools/external/cass/adapter.ts';
import { createAdapter as agentMailFactory } from './tools/external/agent-mail/adapter.ts';
import { createAdapter as mcpSearchFactory } from './tools/external/mcp-search/adapter.ts';
import { createAdapter as mcpGraphFactory } from './tools/external/mcp-graph/adapter.ts';
import { createAdapter as fsHandoffFactory } from './tools/built-in/fs-handoff/adapter.ts';
import { createAdapter as fsSearchFactory } from './tools/built-in/fs-search/adapter.ts';

/**
 * Synchronous registry: tool name -> adapter factory function.
 * Used by services.ts for port resolution without async cascade.
 */
export const ADAPTER_FACTORIES: Record<string, AdapterFactory> = {
  'fs-tasks': fsTasksFactory,
  'fs-handoff': fsHandoffFactory,
  'fs-search': fsSearchFactory,
  'br': brFactory,
  'bv': bvFactory,
  'cass': cassFactory,
  'agent-mail': agentMailFactory,
  'mcp-search': mcpSearchFactory,
  'mcp-graph': mcpGraphFactory,
};

/**
 * Get an adapter factory by tool name (synchronous).
 */
export function getAdapterFactory(toolName: string): AdapterFactory | null {
  return ADAPTER_FACTORIES[toolName] ?? null;
}

