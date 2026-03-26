/**
 * Regression tests for CLI agent-friendliness improvements (round 2).
 *
 * Fixed issues:
 * - handleCommandError() wrote human text to stderr in --json mode instead of
 *   structured JSON to stdout. Agents couldn't parse CLI error responses.
 * - Root `maestro` command showed a one-liner instead of grouped command summary.
 *   Agents wasted context on flat --help dumps.
 * - 53 CLI commands had no usage examples in --help. Agents had to guess
 *   correct invocations without pattern-matchable examples.
 *
 * Test categories:
 * 1. JSON error output contract (exact reproduction + boundary variants)
 * 2. Grouped help display (exact reproduction)
 * 3. Universal examples coverage (source scan for category siblings)
 * 4. Output mode consistency (category siblings -- any path that bypasses --json)
 */

import { describe, test, expect, afterEach } from 'bun:test';
import { createTestHarness, getErrorText, type TestHarness } from '../mocks/test-harness.ts';
import * as fs from 'fs';
import * as path from 'path';

let harness: TestHarness;

afterEach(async () => {
  if (harness) await harness.cleanup();
});

// ---------------------------------------------------------------------------
// 1. JSON error output contract
// ---------------------------------------------------------------------------
describe('CLI JSON error output', () => {
  test('MaestroError produces structured JSON on stdout with exit code 1', async () => {
    // Exact reproduction: before the fix, this would produce human text on stderr.
    // Now it must produce {success:false, command, error, hints} on stdout.
    harness = await createTestHarness();
    await harness.run('init');
    await harness.run('config-set', '--key', 'tasks.backend', '--value', 'fs');
    await harness.run('feature-create', 'test-feature');

    // Trigger a MaestroError: plan-approve without a plan
    const result = await harness.run('plan-approve', '--feature', 'test-feature');
    expect(result.exitCode).toBe(1);

    // Must be valid JSON on stdout
    const parsed = JSON.parse(result.stdout);
    expect(parsed.success).toBe(false);
    expect(parsed.command).toBe('plan-approve');
    expect(typeof parsed.error).toBe('string');
    expect(parsed.error.length).toBeGreaterThan(0);
  });

  test('MaestroError includes hints array when available', async () => {
    harness = await createTestHarness();
    await harness.run('init');
    await harness.run('config-set', '--key', 'tasks.backend', '--value', 'fs');
    await harness.run('feature-create', 'test-feature');
    await harness.run('plan-write', '--feature', 'test-feature', '--content', [
      '## Discovery',
      'We investigated thoroughly and found the system needs work across multiple dimensions that span the entire codebase.',
      '## Non-Goals', '- None',
      '## Ghost Diffs', '- None',
      '### 1. Task A', 'Do something',
    ].join('\n'));
    await harness.run('plan-approve', '--feature', 'test-feature');
    await harness.run('task-sync', '--feature', 'test-feature');

    // Trigger error: feature-complete with undone tasks (includes hints)
    const result = await harness.run('feature-complete', '--feature', 'test-feature');
    expect(result.exitCode).toBe(1);

    const parsed = JSON.parse(result.stdout);
    expect(parsed.success).toBe(false);
    expect(parsed.command).toBe('feature-complete');
    expect(parsed.error).toContain('not done');
    // hints should be present (complete-feature adds contextual hints)
    expect(Array.isArray(parsed.hints)).toBe(true);
  });

  test('plain Error (non-MaestroError) produces JSON with no hints', async () => {
    harness = await createTestHarness();
    await harness.run('init');
    await harness.run('config-set', '--key', 'tasks.backend', '--value', 'fs');
    await harness.run('feature-create', 'test-feature');

    // Trigger plain Error: duplicate feature-create throws Error (not MaestroError)
    const result = await harness.run('feature-create', 'test-feature');
    expect(result.exitCode).toBe(1);

    const parsed = JSON.parse(result.stdout);
    expect(parsed.success).toBe(false);
    expect(parsed.error).toContain('already exists');
    // No hints for plain Error
    expect(parsed.hints).toBeUndefined();
  });

  test('error JSON is parseable even with special characters in message', async () => {
    harness = await createTestHarness();
    await harness.run('init');
    await harness.run('config-set', '--key', 'tasks.backend', '--value', 'fs');
    await harness.run('feature-create', 'test-feature');

    // Trigger error with special chars in context: plan content with quotes
    const result = await harness.run('plan-write', '--feature', 'test-feature',
      '--content', 'Missing "Discovery" section with "quotes" and \\backslashes');
    expect(result.exitCode).toBe(1);

    // Must still be valid JSON despite special chars in error message
    const parsed = JSON.parse(result.stdout);
    expect(parsed.success).toBe(false);
    expect(typeof parsed.error).toBe('string');
  });

  test('success output still works normally in JSON mode', async () => {
    harness = await createTestHarness();
    await harness.run('init');
    await harness.run('config-set', '--key', 'tasks.backend', '--value', 'fs');

    const result = await harness.run('feature-create', 'normal-feature');
    expect(result.exitCode).toBe(0);

    const parsed = JSON.parse(result.stdout);
    // Success output does not have the error fields
    expect(parsed.name).toBe('normal-feature');
    expect(parsed.success).toBeUndefined(); // CLI output() doesn't add success:true (that's MCP only)
  });

  test('getErrorText helper works for both JSON and text error formats', () => {
    // JSON format (--json mode)
    const jsonResult = {
      exitCode: 1,
      stdout: JSON.stringify({ success: false, error: 'bad input', hints: ['fix it'] }),
      stderr: '',
    };
    expect(getErrorText(jsonResult)).toBe('bad input');

    // Text format (no --json)
    const textResult = {
      exitCode: 1,
      stdout: '',
      stderr: '[error] command: bad input\n[hint] fix it',
    };
    expect(getErrorText(textResult)).toContain('bad input');

    // Edge case: non-JSON stdout falls back to stderr
    const mixedResult = {
      exitCode: 1,
      stdout: 'not json',
      stderr: '[error] boom',
    };
    expect(getErrorText(mixedResult)).toContain('boom');
  });
});

// ---------------------------------------------------------------------------
// 2. Grouped help display
// ---------------------------------------------------------------------------
describe('grouped root help display', () => {
  const CLI_PATH = path.join(import.meta.dir, '../../surfaces/cli/index.ts');

  test('root maestro command shows command groups', () => {
    const result = Bun.spawnSync(['bun', CLI_PATH], {
      stdout: 'pipe',
      stderr: 'pipe',
    });
    const output = result.stdout.toString();

    expect(output).toContain('Command groups:');
    expect(output).toContain('feature-*');
    expect(output).toContain('plan-*');
    expect(output).toContain('task-*');
    expect(output).toContain('memory-*');
    expect(output).toContain('doctrine-*');
    expect(output).toContain('config-*');
    expect(output).toContain('maestro <command> --help');
  });

  test('root help mentions --json support', () => {
    const result = Bun.spawnSync(['bun', CLI_PATH], {
      stdout: 'pipe',
      stderr: 'pipe',
    });
    const output = result.stdout.toString();

    expect(output).toContain('--json');
  });
});

// ---------------------------------------------------------------------------
// 3. Universal examples coverage (source scan)
// ---------------------------------------------------------------------------
describe('CLI examples coverage', () => {
  const HANDLERS_DIR = path.join(import.meta.dir, '../../surfaces/cli/handlers');

  function getAllHandlerFiles(dir: string): string[] {
    const results: string[] = [];
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const fullPath = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        results.push(...getAllHandlerFiles(fullPath));
      } else if (entry.name.endsWith('.ts') && !entry.name.startsWith('_')) {
        results.push(fullPath);
      }
    }
    return results;
  }

  test('every handler file with defineCommand has Examples in description', () => {
    const handlerFiles = getAllHandlerFiles(HANDLERS_DIR);
    const missing: string[] = [];

    for (const file of handlerFiles) {
      const content = fs.readFileSync(file, 'utf-8');
      if (!content.includes('defineCommand')) continue;
      if (!content.includes('Examples:')) {
        missing.push(path.relative(HANDLERS_DIR, file));
      }
    }

    expect(missing).toEqual([]);
  });

  test('factory-generated commands also have examples', () => {
    // The _task-factory.ts creates 5 commands (info, spec-read, spec-write,
    // report-read, report-write). Verify the factory file has examples.
    const factoryPath = path.join(HANDLERS_DIR, '_task-factory.ts');
    const content = fs.readFileSync(factoryPath, 'utf-8');

    // Should have Examples for each factory function
    const examplesCount = (content.match(/Examples:/g) || []).length;
    expect(examplesCount).toBeGreaterThanOrEqual(3); // info, docRead, docWrite
  });

  test('examples use realistic command names matching the handler', () => {
    // Spot-check: examples should reference the actual command name
    const spotChecks = [
      { file: 'task/claim.ts', expectedCmd: 'maestro task-claim' },
      { file: 'memory/write.ts', expectedCmd: 'maestro memory-write' },
      { file: 'plan/write.ts', expectedCmd: 'maestro plan-write' },
      { file: 'doctrine/write.ts', expectedCmd: 'maestro doctrine-write' },
      { file: 'config/get.ts', expectedCmd: 'maestro config-get' },
      { file: 'handoff/send.ts', expectedCmd: 'maestro handoff-send' },
      { file: 'toolbox/add.ts', expectedCmd: 'maestro toolbox-add' },
    ];

    for (const { file, expectedCmd } of spotChecks) {
      const content = fs.readFileSync(path.join(HANDLERS_DIR, file), 'utf-8');
      expect(content).toContain(expectedCmd);
    }
  });
});

// ---------------------------------------------------------------------------
// 4. Output mode consistency -- category siblings
// ---------------------------------------------------------------------------
describe('output mode consistency across error paths', () => {
  test('every CLI handler uses handleCommandError for errors', () => {
    // Category sibling check: if a handler catches errors but doesn't use
    // handleCommandError, the JSON error output won't work for that command.
    const HANDLERS_DIR = path.join(import.meta.dir, '../../surfaces/cli/handlers');

    function getAllHandlerFiles(dir: string): string[] {
      const results: string[] = [];
      for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        const fullPath = path.join(dir, entry.name);
        if (entry.isDirectory()) {
          results.push(...getAllHandlerFiles(fullPath));
        } else if (entry.name.endsWith('.ts')) {
          results.push(fullPath);
        }
      }
      return results;
    }

    const handlerFiles = getAllHandlerFiles(HANDLERS_DIR);
    const violations: string[] = [];

    for (const file of handlerFiles) {
      const content = fs.readFileSync(file, 'utf-8');
      if (!content.includes('defineCommand')) continue;

      // If the handler has a catch block, it should use handleCommandError
      if (content.includes('catch') && !content.includes('handleCommandError')) {
        violations.push(path.relative(HANDLERS_DIR, file));
      }
    }

    expect(violations).toEqual([]);
  });

  test('handleCommandError is imported from domain/errors in all handlers', () => {
    const HANDLERS_DIR = path.join(import.meta.dir, '../../surfaces/cli/handlers');

    function getAllHandlerFiles(dir: string): string[] {
      const results: string[] = [];
      for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        const fullPath = path.join(dir, entry.name);
        if (entry.isDirectory()) {
          results.push(...getAllHandlerFiles(fullPath));
        } else if (entry.name.endsWith('.ts')) {
          results.push(fullPath);
        }
      }
      return results;
    }

    const handlerFiles = getAllHandlerFiles(HANDLERS_DIR);
    const missingImport: string[] = [];

    for (const file of handlerFiles) {
      const content = fs.readFileSync(file, 'utf-8');
      if (!content.includes('handleCommandError(')) continue;

      // Must import it from errors.ts
      if (!content.includes("from") || !content.includes("errors")) {
        missingImport.push(path.relative(HANDLERS_DIR, file));
      }
    }

    expect(missingImport).toEqual([]);
  });

  test('multiple different error-triggering commands all produce valid JSON', async () => {
    // Exercise several distinct commands that produce errors to verify
    // the JSON error output is consistent across the codebase.
    harness = await createTestHarness();
    await harness.run('init');
    await harness.run('config-set', '--key', 'tasks.backend', '--value', 'fs');
    await harness.run('feature-create', 'test-feature');

    const errorCommands = [
      // MaestroError path: plan-approve without plan
      ['plan-approve', '--feature', 'test-feature'],
      // MaestroError path: task-sync without approved plan
      ['task-sync', '--feature', 'test-feature'],
      // MaestroError path: plan-write without Discovery section
      ['plan-write', '--feature', 'test-feature', '--content', '# Bad plan'],
      // Error path: memory-read nonexistent
      ['memory-read', '--feature', 'test-feature', '--name', 'nonexistent'],
    ];

    for (const args of errorCommands) {
      const result = await harness.run(...args);
      expect(result.exitCode).toBe(1);

      // Every error must produce parseable JSON on stdout
      let parsed: Record<string, unknown>;
      try {
        parsed = JSON.parse(result.stdout);
      } catch {
        throw new Error(`Command ${args[0]} produced non-JSON error output: ${result.stdout.substring(0, 200)}`);
      }

      expect(parsed.success).toBe(false);
      expect(typeof parsed.command).toBe('string');
      expect(typeof parsed.error).toBe('string');
      expect(parsed.error.length).toBeGreaterThan(0);
    }
  });
});
