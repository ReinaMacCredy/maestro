import { z } from 'zod';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { ServicesThunk } from '../services-thunk.ts';
import { respond, withErrorHandling } from '../respond.ts';
import { ANNOTATIONS_READONLY, ANNOTATIONS_MUTATING } from '../annotations.ts';
import { readJson, writeJsonAtomic, ensureDir } from '../../../infra/utils/fs-io.ts';
import { getNestedValue, setNestedValue } from '../../../infra/utils/object-utils.ts';
import * as path from 'path';

const REDACT_PATTERN = /apiKey|token|secret|password/i;

function redactSecrets(obj: Record<string, unknown>): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(obj)) {
    if (REDACT_PATTERN.test(key)) {
      result[key] = '[REDACTED]';
    } else if (value !== null && typeof value === 'object' && !Array.isArray(value)) {
      result[key] = redactSecrets(value as Record<string, unknown>);
    } else {
      result[key] = value;
    }
  }
  return result;
}

export function registerConfigTools(server: McpServer, thunk: ServicesThunk): void {
  server.registerTool(
    'maestro_config_get',
    {
      description: 'Read maestro project settings. Use to check current configuration values like DCP thresholds, task backend, or toolbox settings. Supports dot notation (e.g. "dcp.enabled"). Omit key for full settings.',
      inputSchema: {
        key: z.string().optional().describe('Specific config key (supports dot notation, e.g. "dcp.enabled", "toolbox.deny"). Omit for full settings.'),
      },
      annotations: ANNOTATIONS_READONLY,
    },
    withErrorHandling(async (input) => {
      const services = thunk.get();
      const settings = services.settingsPort.get();
      const redacted = redactSecrets(settings as unknown as Record<string, unknown>);

      if (input.key) {
        const value = getNestedValue(redacted, input.key);
        return respond({ key: input.key, value: value ?? null });
      }
      return respond({ settings: redacted });
    }),
  );

  server.registerTool(
    'maestro_config_set',
    {
      description: 'Update a maestro project setting. Use to change DCP thresholds, switch task backends, or adjust toolbox configuration. Dot notation keys (e.g. "tasks.backend", "dcp.enabled"). Writes to project settings.json.',
      inputSchema: {
        key: z.string().describe('Settings key with dot notation (e.g. "tasks.backend", "toolbox.deny")'),
        value: z.string().describe('Value to set (JSON for objects/arrays, plain string otherwise)'),
      },
      annotations: ANNOTATIONS_MUTATING,
    },
    withErrorHandling(async (input) => {
      const services = thunk.get();
      let parsed: unknown;
      try { parsed = JSON.parse(input.value); } catch { parsed = input.value; }

      const settingsPath = path.join(services.directory, '.maestro', 'settings.json');
      const existing = readJson<Record<string, unknown>>(settingsPath) ?? {};
      setNestedValue(existing, input.key, parsed);

      ensureDir(path.dirname(settingsPath));
      writeJsonAtomic(settingsPath, existing);

      // Invalidate both the settings adapter cache AND the container so
      // subsequent MCP calls see the new config (e.g. tasks.backend change).
      (services.settingsPort as any).invalidate?.();
      thunk.invalidate();

      return respond({ key: input.key, value: parsed });
    }),
  );
}
