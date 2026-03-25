/**
 * maestro toolbox-test -- validate a tool's manifest and adapter.
 */

import * as fs from 'fs';
import * as path from 'path';
import { defineCommand } from 'citty';
import { output } from '../../../../core/output.ts';
import { handleCommandError } from '../../../../core/errors.ts';
import { loadManifest, inferTransport } from '../../../../toolbox/loader.ts';
import { execFileSync } from 'node:child_process';

interface CheckResult {
  check: string;
  status: 'pass' | 'fail' | 'skip';
  message: string;
}

function findToolDir(name: string): string | null {
  const builtIn = path.join(import.meta.dir, '../../../toolbox/tools/built-in', name);
  if (fs.existsSync(builtIn)) return builtIn;
  const external = path.join(import.meta.dir, '../../../toolbox/tools/external', name);
  if (fs.existsSync(external)) return external;
  return null;
}

export default defineCommand({
  meta: { name: 'toolbox-test', description: 'Validate a tool manifest and adapter' },
  args: {
    name: {
      type: 'string',
      description: 'Tool name to validate',
      required: true,
    },
  },
  async run({ args }) {
    try {
      const checks: CheckResult[] = [];
      const toolDir = findToolDir(args.name);

      // 1. Tool directory exists
      if (!toolDir) {
        checks.push({ check: 'directory', status: 'fail', message: `Tool '${args.name}' not found` });
        output({ name: args.name, checks, passed: false }, formatChecks);
        return;
      }
      checks.push({ check: 'directory', status: 'pass', message: toolDir });

      // 2. Manifest is valid
      const manifestPath = path.join(toolDir, 'manifest.json');
      const manifest = loadManifest(manifestPath);
      if (!manifest) {
        checks.push({ check: 'manifest', status: 'fail', message: 'manifest.json missing or invalid (needs name + priority)' });
        output({ name: args.name, checks, passed: false }, formatChecks);
        return;
      }
      const transport = inferTransport(manifest);
      checks.push({ check: 'manifest', status: 'pass', message: `name=${manifest.name} transport=${transport} provides=${manifest.provides ?? 'none'}` });

      // 3. Binary detection
      if (manifest.detect) {
        try {
          const version = execFileSync('sh', ['-c', manifest.detect], {
            stdio: ['pipe', 'pipe', 'pipe'],
            timeout: 5000,
          }).toString().trim().split('\n')[0];
          checks.push({ check: 'detect', status: 'pass', message: version || 'detected' });
        } catch {
          checks.push({ check: 'detect', status: 'fail', message: `Detection failed: ${manifest.detect}` });
        }
      } else {
        checks.push({ check: 'detect', status: 'skip', message: 'No detect command (built-in or HTTP tool)' });
      }

      // 4. Adapter exists and exports createAdapter
      const adapterPath = path.join(toolDir, 'adapter.ts');
      if (fs.existsSync(adapterPath)) {
        try {
          const mod = await import(adapterPath);
          if (typeof mod.createAdapter === 'function') {
            checks.push({ check: 'adapter', status: 'pass', message: 'createAdapter() exported' });
          } else {
            checks.push({ check: 'adapter', status: 'fail', message: 'adapter.ts does not export createAdapter function' });
          }
        } catch (e) {
          checks.push({ check: 'adapter', status: 'fail', message: `adapter.ts import error: ${(e as Error).message}` });
        }
      } else {
        checks.push({ check: 'adapter', status: 'skip', message: 'No adapter.ts (manifest-only tool)' });
      }

      const passed = checks.every(c => c.status !== 'fail');
      output({ name: args.name, checks, passed }, formatChecks);
    } catch (err) {
      handleCommandError('toolbox-test', err);
    }
  },
});

function formatChecks(result: { name: string; checks: CheckResult[]; passed: boolean }): string {
  const lines = [`[toolbox] Validating tool '${result.name}':\n`];
  for (const c of result.checks) {
    const icon = c.status === 'pass' ? '[ok]' : c.status === 'fail' ? '[!!]' : '[--]';
    lines.push(`  ${icon} ${c.check}: ${c.message}`);
  }
  lines.push('');
  lines.push(result.passed ? '[ok] All checks passed' : '[!!] Validation failed');
  return lines.join('\n');
}
